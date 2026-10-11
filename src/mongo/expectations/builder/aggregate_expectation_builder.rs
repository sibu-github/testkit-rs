use bson::Document;

use crate::mongo::{
    expectations::{AggregateExpectation, CommandExpectation, CommandExpectationBuilder},
    response::Response,
};

pub type AggregateExpectationBuilder = CommandExpectationBuilder<AggregateBuilder>;

#[derive(Debug, Default)]
pub struct AggregateBuilder {
    pub expected_pipeline: Option<Vec<Document>>,
}

impl CommandExpectationBuilder<AggregateBuilder> {
    pub fn pipeline(mut self, pipeline: Vec<Document>) -> Self {
        self.builder.expected_pipeline = Some(pipeline);
        self
    }

    pub fn respond_with(self, docs: Vec<Document>) {
        let res = Response::Cursor(docs);
        let command = self.command_expectation();
        self.add_expectation(command, res);
    }

    fn command_expectation(&self) -> CommandExpectation {
        let expected_pipeline = self
            .builder
            .expected_pipeline
            .as_ref()
            .map(|pipeline| pipeline.iter().map(Into::into).collect());
        let aggregate_expectation = AggregateExpectation {
            expected_pipeline,
            expected_count_query: None,
        };
        CommandExpectation::Aggregate(Box::new(aggregate_expectation))
    }
}
