use std::marker::PhantomData;

use bson::{Document, doc};

use crate::mongo::expectations::{
    CommandExpectation, CommandExpectationBuilder, DeleteExpectation,
};
use crate::mongo::response::Response;

#[derive(Debug, Default)]
pub struct DeleteMany;

#[derive(Debug, Default)]
pub struct DeleteOne;

pub type DeleteManyExpectationBuilder =
    CommandExpectationBuilder<DeleteExpectationBuilder<DeleteMany>>;

pub type DeleteOneExpectationBuilder =
    CommandExpectationBuilder<DeleteExpectationBuilder<DeleteOne>>;

#[derive(Default)]
pub struct DeleteExpectationBuilder<T> {
    pub expected_filter: Option<Document>,
    _phantom: PhantomData<T>,
}

impl<T: Default> CommandExpectationBuilder<DeleteExpectationBuilder<T>> {
    pub fn filter(mut self, filter: Document) -> Self {
        self.builder.expected_filter = Some(filter);
        self
    }

    fn respond_with(self, limit: i32, n: i32) {
        let res = doc! { "n": n };
        let response = Response::Doc(res);
        self.add_resonse(limit, response);
    }

    fn add_resonse(self, limit: i32, response: Response) {
        let delete_expectation = DeleteExpectation {
            expected_filter: self.builder.expected_filter.clone(),
            expected_limit: limit,
        };
        let command = CommandExpectation::Delete(Box::new(delete_expectation));
        self.add_expectation(command, response);
    }
}

impl CommandExpectationBuilder<DeleteExpectationBuilder<DeleteMany>> {
    pub fn respond(self, n: i32) {
        self.respond_with(0, n);
    }
}

impl CommandExpectationBuilder<DeleteExpectationBuilder<DeleteOne>> {
    pub fn respond(self, n: i32) {
        self.respond_with(1, n);
    }
}
