use anyhow::{bail, ensure};
use bson::{Bson, Document};

use crate::mongo::{
    command_name::CommandName,
    expectations::{
        AggregateExpectation, CreateIndexExpectation, DeleteExpectation, DistinctExpectation,
        FindAndModifyExpectation, FindExpectation, InsertExpectation, UpdateExpectation,
    },
    response::Response,
};

pub(crate) type Validator = Box<dyn Fn(&Document) -> anyhow::Result<()> + Send + Sync>;

/// Reads an optional integer field that the driver may encode as either
/// `Int32` or `Int64`.
pub(crate) fn get_integer(command: &Document, key: &str) -> anyhow::Result<Option<i64>> {
    match command.get(key) {
        None => Ok(None),
        Some(Bson::Int64(value)) => Ok(Some(*value)),
        Some(Bson::Int32(value)) => Ok(Some(i64::from(*value))),
        Some(other) => bail!("expected `{key}` to be an integer, received: {other}"),
    }
}

/// Ensures an optional integer command field equals `expected`.
pub(crate) fn ensure_integer(command: &Document, key: &str, expected: i64) -> anyhow::Result<()> {
    let actual = get_integer(command, key)?;
    ensure!(
        actual == Some(expected),
        "expected {key}: {expected}, received: {}",
        actual.map_or_else(|| "none".to_owned(), |value| value.to_string())
    );
    Ok(())
}

/// Compares two documents, including the order of their keys.
///
/// BSON document equality ignores key order, but the order is significant in a
/// sort specification: `{ a: 1, b: -1 }` and `{ b: -1, a: 1 }` sort differently.
pub(crate) fn same_key_order(expected: &Document, actual: &Document) -> bool {
    expected.len() == actual.len() && expected.iter().eq(actual.iter())
}

#[derive(Debug)]
pub(crate) enum CommandExpectation {
    Find(Box<FindExpectation>),
    FindAndModify(Box<FindAndModifyExpectation>),
    Insert(Box<InsertExpectation>),
    Update(Box<UpdateExpectation>),
    Delete(Box<DeleteExpectation>),
    Aggregate(Box<AggregateExpectation>),
    Distinct(Box<DistinctExpectation>),
    CreateIndex(Box<CreateIndexExpectation>),
}

impl CommandExpectation {
    pub fn new(command_name: CommandName) -> Self {
        match command_name {
            CommandName::Aggregate => Self::Aggregate(Default::default()),
            CommandName::Find => Self::Find(Default::default()),
            CommandName::FindAndModify => Self::FindAndModify(Default::default()),
            CommandName::Insert => Self::Insert(Default::default()),
            CommandName::Update => Self::Update(Default::default()),
            CommandName::Delete => Self::Delete(Default::default()),
            CommandName::Distinct => Self::Distinct(Default::default()),
            CommandName::CreateIndex => Self::CreateIndex(Default::default()),
            CommandName::Unknown => unreachable!(),
            CommandName::Handshake => unreachable!(),
        }
    }

    pub fn validate(&self, command: &Document) -> anyhow::Result<()> {
        match self {
            Self::Find(fe) => fe.validate(command)?,
            Self::FindAndModify(fe) => fe.validate(command)?,
            Self::Insert(ie) => ie.validate(command)?,
            Self::Update(ue) => ue.validate(command)?,
            Self::Delete(de) => de.validate(command)?,
            Self::Aggregate(ae) => ae.validate(command)?,
            Self::Distinct(de) => de.validate(command)?,
            Self::CreateIndex(ce) => ce.validate(command)?,
        }
        Ok(())
    }

    pub fn command_name(&self) -> CommandName {
        match self {
            Self::Find(_) => CommandName::Find,
            Self::FindAndModify(_) => CommandName::FindAndModify,
            Self::Aggregate(_) => CommandName::Aggregate,
            Self::Insert(_) => CommandName::Insert,
            Self::Update(_) => CommandName::Update,
            Self::Delete(_) => CommandName::Delete,
            Self::Distinct(_) => CommandName::Distinct,
            Self::CreateIndex(_) => CommandName::CreateIndex,
        }
    }
}

pub(crate) struct Expectation {
    pub expected_database: Option<String>,
    pub expected_collection: Option<String>,
    pub command: CommandExpectation,
    pub validator: Option<Validator>,
    pub response: Response,
}

impl Expectation {
    fn namespace(&self, command: &Document) -> anyhow::Result<String> {
        let name = CommandName::try_from(command)?;
        let db = command.get_str("$db").unwrap_or("test");
        let coll = command.get_str(name.to_string()).unwrap_or("test");
        Ok(format!("{db}.{coll}"))
    }

    fn command_name(&self) -> CommandName {
        self.command.command_name()
    }

    pub fn validate_and_respond(&self, command: &Document) -> anyhow::Result<Document> {
        let ns = self.namespace(command)?;
        if let Some(expected) = self.expected_database.as_ref() {
            let db = command.get_str("$db")?;
            ensure!(
                expected == db,
                "expected database: {expected}, received: {db}"
            );
        }
        if let Some(expected) = self.expected_collection.as_ref() {
            let coll = command.get_str(self.command_name().to_string())?;
            ensure!(
                expected == coll,
                "expected collection: {expected}, received: {coll}"
            );
        }
        self.command.validate(command)?;
        if let Some(validator) = self.validator.as_deref() {
            validator(command)?;
        }
        let doc = self.response.to_bson_doc(&ns);
        Ok(doc)
    }
}
