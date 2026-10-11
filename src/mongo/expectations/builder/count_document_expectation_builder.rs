use bson::{Document, doc};

use crate::mongo::{
    expectations::{AggregateExpectation, CommandExpectation, CommandExpectationBuilder},
    response::Response,
};

pub type CountDocumentsExpectationBuilder = CommandExpectationBuilder<CountDocsBuilder>;

#[derive(Debug, Default)]
pub struct CountDocsBuilder {
    pub expected_count_query: Option<Document>,
}

impl CommandExpectationBuilder<CountDocsBuilder> {
    pub fn query(mut self, query: Document) -> Self {
        self.builder.expected_count_query = Some(query);
        self
    }

    pub fn respond_with(self, count: u32) {
        let doc = doc! {"_id": 1, "n": count};
        let res = Response::Cursor(vec![doc]);
        let command = self.command_expectation();
        self.add_expectation(command, res);
    }

    fn command_expectation(&self) -> CommandExpectation {
        let aggregate_expectation = AggregateExpectation {
            expected_pipeline: None,
            expected_count_query: self.builder.expected_count_query.clone(),
        };
        CommandExpectation::Aggregate(Box::new(aggregate_expectation))
    }
}
