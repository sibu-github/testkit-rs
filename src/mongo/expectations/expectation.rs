use anyhow::ensure;
use bson::Document;

use crate::mongo::{
    command_name::CommandName,
    expectations::{
        AggregateExpectation, CreateIndexExpectation, DeleteExpectation, DistinctExpectation,
        FindAndModifyExpectation, FindExpectation, InsertExpectation, UpdateExpectation,
    },
    response::Response,
};

pub(crate) type Validator = Box<dyn Fn(&Document) -> anyhow::Result<()> + Send + Sync>;

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
