use std::sync::{Arc, Mutex};

use bson::Document;

use crate::mongo::{
    command_name::CommandName,
    expectations::{CommandExpectation, Expectation, Validator},
    response::Response,
    state::State,
};

pub struct CommandExpectationBuilder<T> {
    state: Arc<Mutex<State>>,
    command_name: CommandName,
    pub(crate) expected_database: Option<String>,
    pub(crate) expected_collection: Option<String>,
    pub(crate) validator: Option<Validator>,
    pub(crate) builder: T,
}

impl<T: Default> CommandExpectationBuilder<T> {
    pub(crate) fn new(state: Arc<Mutex<State>>, command_name: CommandName) -> Self {
        Self {
            state,
            command_name,
            expected_database: None,
            expected_collection: None,
            validator: None,
            builder: Default::default(),
        }
    }

    pub fn database(mut self, db: impl Into<String>) -> Self {
        self.expected_database = Some(db.into());
        self
    }

    pub fn collection(mut self, coll: impl Into<String>) -> Self {
        self.expected_collection = Some(coll.into());
        self
    }

    pub fn validate_with<F>(mut self, validator: F) -> Self
    where
        F: Fn(&Document) -> anyhow::Result<()> + Send + Sync + 'static,
    {
        self.validator = Some(Box::new(validator));
        self
    }

    pub fn fail(self, msg: &str) {
        let res = Response::Error {
            code: 1,
            message: msg.to_string(),
        };
        let command = CommandExpectation::new(self.command_name);
        self.add_expectation(command, res);
    }

    pub(crate) fn add_expectation(self, command: CommandExpectation, response: Response) {
        let command_name = command.command_name();
        let expectation = Expectation {
            expected_database: self.expected_database,
            expected_collection: self.expected_collection,
            command,
            validator: self.validator,
            response,
        };
        self.state
            .lock()
            .unwrap()
            .insert_expectation(command_name, expectation);
    }
}
