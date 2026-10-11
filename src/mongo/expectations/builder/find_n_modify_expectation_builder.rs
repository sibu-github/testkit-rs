use std::marker::PhantomData;

use bson::{Bson, Document, doc};
use mongodb::options::ReturnDocument;
use serde::Serialize;

use crate::mongo::{
    expectations::{CommandExpectation, CommandExpectationBuilder, FindAndModifyExpectation},
    response::Response,
};

#[derive(Default)]
pub struct RemoveDoc;

#[derive(Default)]
pub struct UpdateDoc;

pub type FindAndModifyExpectationBuilder =
    CommandExpectationBuilder<FindAndModifyBuilder<UpdateDoc>>;

pub type FindAndRemoveExpectationBuilder =
    CommandExpectationBuilder<FindAndModifyBuilder<RemoveDoc>>;

#[derive(Debug, Default)]
pub struct FindAndModifyBuilder<T> {
    pub expected_filter: Option<Document>,
    pub expected_update: Option<Document>,
    pub expected_array_filters: Option<Vec<Bson>>,
    pub expected_sort: Option<Document>,
    pub expected_fields: Option<Document>,
    pub expected_upsert: Option<bool>,
    pub expected_remove: Option<bool>,
    pub expected_new: Option<bool>,
    _phantom: PhantomData<T>,
}

impl<T: Default> CommandExpectationBuilder<FindAndModifyBuilder<T>> {
    pub fn filter(mut self, filter: Document) -> Self {
        self.builder.expected_filter = Some(filter);
        self
    }

    pub fn respond_with<V: Serialize>(self, value: Option<V>) {
        let value = bson::serialize_to_bson(&value).unwrap();
        let doc = doc! { "value": value };
        let res = Response::Doc(doc);
        let command = self.command_expectation();
        self.add_expectation(command, res);
    }

    fn command_expectation(&self) -> CommandExpectation {
        let find_n_modify_expectation = FindAndModifyExpectation {
            expected_filter: self.builder.expected_filter.clone(),
            expected_update: self.builder.expected_update.clone(),
            expected_array_filters: self.builder.expected_array_filters.clone(),
            expected_sort: self.builder.expected_sort.clone(),
            expected_fields: self.builder.expected_fields.clone(),
            expected_upsert: self.builder.expected_upsert,
            expected_remove: self.builder.expected_remove,
            expected_new: self.builder.expected_new,
        };
        CommandExpectation::FindAndModify(Box::new(find_n_modify_expectation))
    }
}

impl CommandExpectationBuilder<FindAndModifyBuilder<UpdateDoc>> {
    pub fn update(mut self, update: Document) -> Self {
        self.builder.expected_update = Some(update);
        self
    }

    pub fn sort(mut self, sort: Document) -> Self {
        self.builder.expected_sort = Some(sort);
        self
    }

    pub fn projection(mut self, fields: Document) -> Self {
        self.builder.expected_fields = Some(fields);
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

    pub fn return_document(mut self, return_document: ReturnDocument) -> Self {
        let val = matches!(return_document, ReturnDocument::After);
        self.builder.expected_new = Some(val);
        self
    }
}

impl CommandExpectationBuilder<FindAndModifyBuilder<RemoveDoc>> {
    pub fn remove(mut self) -> Self {
        self.builder.expected_remove = Some(true);
        self
    }

    pub fn sort(mut self, sort: Document) -> Self {
        self.builder.expected_sort = Some(sort);
        self
    }

    pub fn projection(mut self, fields: Document) -> Self {
        self.builder.expected_fields = Some(fields);
        self
    }
}
