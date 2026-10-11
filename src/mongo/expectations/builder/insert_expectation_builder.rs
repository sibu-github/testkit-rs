use std::marker::PhantomData;

use bson::{Document, doc};
use serde::Serialize;

use crate::mongo::expectations::{
    CommandExpectation, CommandExpectationBuilder, InsertExpectation,
};
use crate::mongo::response::Response;

#[derive(Default)]
pub struct InsertMany;

#[derive(Default)]
pub struct InsertOne;

pub type InsertManyExpectationBuilder = CommandExpectationBuilder<InsertBuilder<InsertMany>>;

pub type InsertOneExpectationBuilder = CommandExpectationBuilder<InsertBuilder<InsertOne>>;

#[derive(Default)]
pub struct InsertBuilder<T> {
    pub expected_documents: Option<Vec<Document>>,
    pub expected_ordered: Option<bool>,
    _phantom: PhantomData<T>,
}

impl<T: Default> CommandExpectationBuilder<InsertBuilder<T>> {
    fn prepare_response(self, n: u32) {
        let doc = doc! { "n": n, "ok": 1.0 };
        let response = Response::Doc(doc);
        self.add_response(response);
    }

    fn add_response(self, response: Response) {
        let insert_expectation = InsertExpectation {
            expected_documents: self.builder.expected_documents.clone(),
            expected_ordered: self.builder.expected_ordered,
        };
        let command = CommandExpectation::Insert(Box::new(insert_expectation));
        self.add_expectation(command, response);
    }
}

impl CommandExpectationBuilder<InsertBuilder<InsertOne>> {
    pub fn with_document<T: Serialize>(mut self, doc: &T) -> Self {
        let doc = bson::serialize_to_document(doc).unwrap();
        self.builder.expected_documents = Some(vec![doc]);
        self
    }

    pub fn respond(self) {
        self.prepare_response(1);
    }
}

impl CommandExpectationBuilder<InsertBuilder<InsertMany>> {
    pub fn with_documents<T: Serialize>(mut self, docs: &[T]) -> Self {
        let docs = docs
            .iter()
            .filter_map(|d| bson::serialize_to_document(d).ok())
            .collect();
        self.builder.expected_documents = Some(docs);
        self
    }

    pub fn ordered(mut self, order: bool) -> Self {
        self.builder.expected_ordered = Some(order);
        self
    }

    pub fn respond_with(self, n: u32) {
        self.prepare_response(n);
    }
}
