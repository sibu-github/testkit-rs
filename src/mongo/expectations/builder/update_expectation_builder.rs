use std::marker::PhantomData;

use bson::oid::ObjectId;
use bson::{Bson, Document, doc};

use crate::mongo::expectations::{
    CommandExpectation, CommandExpectationBuilder, UpdateExpectation,
};
use crate::mongo::response::Response;

#[derive(Default)]
pub struct UpdateMany;

#[derive(Default)]
pub struct UpdateOne;

pub type UpdateManyExpectationBuilder = CommandExpectationBuilder<UpdateBuilder<UpdateMany>>;
pub type UpdateOneExpectationBuilder = CommandExpectationBuilder<UpdateBuilder<UpdateOne>>;

#[derive(Default)]
pub struct UpdateBuilder<T> {
    pub expected_filter: Option<Document>,
    pub expected_update: Option<Bson>,
    pub expected_upsert: Option<bool>,
    pub expected_array_filters: Option<Vec<Bson>>,
    _phantom: PhantomData<T>,
}

impl<T: Default> CommandExpectationBuilder<UpdateBuilder<T>> {
    pub fn filter(mut self, filter: Document) -> Self {
        self.builder.expected_filter = Some(filter);
        self
    }

    pub fn update<B>(mut self, update: B) -> Self
    where
        B: Into<Bson>,
    {
        self.builder.expected_update = Some(update.into());
        self
    }

    pub fn upsert(mut self, upsert: bool) -> Self {
        self.builder.expected_upsert = Some(upsert);
        self
    }

    pub fn array_filters<I, B>(mut self, array_filters: I) -> Self
    where
        B: Into<Bson>,
        I: IntoIterator<Item = B>,
    {
        let array_filters = array_filters.into_iter().map(Into::into).collect();
        self.builder.expected_array_filters = Some(array_filters);
        self
    }

    fn add_response(self, multi: bool, response: Response) {
        let update_expectation = UpdateExpectation {
            expected_filter: self.builder.expected_filter.clone(),
            expected_update: self.builder.expected_update.clone(),
            expected_upsert: self.builder.expected_upsert,
            expected_array_filters: self.builder.expected_array_filters.clone(),
            expected_multi: multi,
        };
        let command = CommandExpectation::Update(Box::new(update_expectation));
        self.add_expectation(command, response);
    }
}

impl CommandExpectationBuilder<UpdateBuilder<UpdateMany>> {
    pub fn respond_with(self, matched_count: u32, modified_count: u32) {
        let res = doc! { "n": matched_count, "nModified": modified_count };
        let response = Response::Doc(res);
        self.add_response(true, response);
    }

    pub fn respond_upserted(self, n: u32, n_modified: u32) {
        let res = doc! {
            "n": n,
            "nModified": n_modified,
            "upserted": [{
                "index": 0,
                "_id": ObjectId::new()
            }]
        };
        let response = Response::Doc(res);
        self.add_response(true, response);
    }
}

impl CommandExpectationBuilder<UpdateBuilder<UpdateOne>> {
    pub fn respond(self) {
        let res = doc! { "n": 1, "nModified": 1 };
        let response = Response::Doc(res);
        self.add_response(false, response);
    }

    pub fn respond_upserted(self) {
        let res = doc! {
            "n": 1,
            "nModified": 0,
            "upserted": [{
                "index": 0,
                "_id": ObjectId::new()
            }]
        };
        let response = Response::Doc(res);
        self.add_response(false, response);
    }
}
