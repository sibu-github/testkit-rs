use bson::{Bson, Document, doc};

use crate::mongo::{
    expectations::{CommandExpectation, CommandExpectationBuilder, DistinctExpectation},
    response::Response,
};

pub type DistinctExpectationBuilder = CommandExpectationBuilder<DistinctBuilder>;

#[derive(Debug, Default)]
pub struct DistinctBuilder {
    pub expected_filter: Option<Document>,
    pub expected_key: Option<String>,
}

impl CommandExpectationBuilder<DistinctBuilder> {
    pub fn filter(mut self, filter: Document) -> Self {
        self.builder.expected_filter = Some(filter);
        self
    }

    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.builder.expected_key = Some(key.into());
        self
    }

    pub fn respond_with<I, T>(self, values: I)
    where
        T: Into<Bson> + Clone,
        I: IntoIterator<Item = T>,
    {
        let values = values.into_iter().map(Into::into).collect::<Vec<_>>();
        let doc = doc! {"values": values};
        let res = Response::Doc(doc);
        let command = self.command_expectation();
        self.add_expectation(command, res);
    }

    fn command_expectation(&self) -> CommandExpectation {
        let distinct_expectation = DistinctExpectation {
            expected_filter: self.builder.expected_filter.clone(),
            expected_key: self.builder.expected_key.clone(),
        };
        CommandExpectation::Distinct(Box::new(distinct_expectation))
    }
}
