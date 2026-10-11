use bson::{Document, doc};

use crate::mongo::{
    expectations::{CommandExpectation, CommandExpectationBuilder},
    response::Response,
};

pub type CreateIndexExpectationBuilder = CommandExpectationBuilder<CreateIndexBuilder>;

#[derive(Debug, Default)]
pub(crate) struct CreateIndexBuilder {}

impl CommandExpectationBuilder<CreateIndexBuilder> {
    pub fn respond(self) {
        let res = Response::Doc(doc! {});
        let command = self.command_expectation();
        self.add_expectation(command, res);
    }

    fn command_expectation(&self) -> CommandExpectation {
        let create_index_expectation = CreateIndexExpectation {};
        CommandExpectation::CreateIndex(Box::new(create_index_expectation))
    }
}

#[derive(Debug, Default)]
pub(crate) struct CreateIndexExpectation {}

impl CreateIndexExpectation {
    pub fn validate(&self, _command: &Document) -> anyhow::Result<()> {
        Ok(())
    }
}
