use std::marker::PhantomData;

use bson::Document;
use serde::Serialize;

use crate::mongo::expectations::CommandExpectationBuilder;
use crate::mongo::{
    expectations::{CommandExpectation, FindExpectation},
    response::Response,
};

#[derive(Default)]
pub struct FindMany;

#[derive(Default)]
pub struct FindOne;

pub type FindExpectationBuilder = CommandExpectationBuilder<FindBuilder<FindMany>>;

pub type FindOneExpectationBuilder = CommandExpectationBuilder<FindBuilder<FindOne>>;

#[derive(Default)]
pub struct FindBuilder<T> {
    pub expected_filter: Option<Document>,
    pub expected_sort: Option<Document>,
    pub expected_project: Option<Document>,
    pub expected_limit: Option<i64>,
    pub expected_skip: Option<i64>,
    _phantom: PhantomData<T>,
}

impl<T: Default> CommandExpectationBuilder<FindBuilder<T>> {
    pub fn filter(mut self, filter: Document) -> Self {
        self.builder.expected_filter = Some(filter);
        self
    }

    pub fn sort(mut self, sort: Document) -> Self {
        self.builder.expected_sort = Some(sort);
        self
    }

    pub fn project(mut self, project: Document) -> Self {
        self.builder.expected_project = Some(project);
        self
    }

    pub fn skip(mut self, skip: i64) -> Self {
        self.builder.expected_skip = Some(skip);
        self
    }

    fn respond(self, docs: Vec<Document>) {
        let response = Response::Cursor(docs);
        self.add_response(response);
    }

    fn add_response(self, response: Response) {
        let find_expectation = FindExpectation {
            expected_filter: self.builder.expected_filter.clone(),
            expected_sort: self.builder.expected_sort.clone(),
            expected_project: self.builder.expected_project.clone(),
            expected_limit: self.builder.expected_limit,
            expected_skip: self.builder.expected_skip,
        };
        let command = CommandExpectation::Find(Box::new(find_expectation));
        self.add_expectation(command, response);
    }
}

impl CommandExpectationBuilder<FindBuilder<FindMany>> {
    pub fn limit(mut self, limit: i64) -> Self {
        self.builder.expected_limit = Some(limit);
        self
    }
    pub fn respond_with<T: Serialize>(self, docs: &[T]) {
        let docs = docs
            .iter()
            .filter_map(|val| bson::serialize_to_document(val).ok())
            .collect();
        self.respond(docs);
    }
}

impl CommandExpectationBuilder<FindBuilder<FindOne>> {
    pub fn respond_with<T: Serialize>(self, doc: &T) {
        let doc = bson::serialize_to_document(doc).unwrap();
        self.respond(vec![doc]);
    }
}
