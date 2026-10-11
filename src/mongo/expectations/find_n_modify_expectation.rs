use anyhow::ensure;
use bson::{Bson, Document};

#[derive(Debug, Default)]
pub(crate) struct FindAndModifyExpectation {
    pub expected_filter: Option<Document>,
    pub expected_update: Option<Document>,
    pub expected_array_filters: Option<Vec<Bson>>,
    pub expected_sort: Option<Document>,
    pub expected_fields: Option<Document>,
    pub expected_upsert: Option<bool>,
    pub expected_remove: Option<bool>,
    pub expected_new: Option<bool>,
}

impl FindAndModifyExpectation {
    pub fn validate(&self, command: &Document) -> anyhow::Result<()> {
        if let Some(expected) = self.expected_filter.as_ref() {
            let filter = command.get_document("query")?;
            ensure!(
                expected == filter,
                "expected filter: {expected}, received: {filter}"
            );
        }
        if let Some(expected) = self.expected_update.as_ref() {
            let update = command.get_document("update")?;
            ensure!(
                expected == update,
                "expected update: {expected}, received: {update}"
            );
        }
        if let Some(expected) = self.expected_sort.as_ref() {
            let sort = command.get_document("sort")?;
            ensure!(
                expected == sort,
                "expected sort: {expected}, received: {sort}"
            );
        }
        if let Some(expected) = self.expected_fields.as_ref() {
            let fields = command.get_document("fields")?;
            ensure!(
                expected == fields,
                "expected fields: {expected}, received: {fields}"
            );
        }
        if let Some(expected) = self.expected_array_filters.as_ref() {
            let array_filters = command.get_array("arrayFilters")?;
            ensure!(
                expected == array_filters,
                "expected arrayFilters: {expected:?}, received: {array_filters:?}"
            );
        }
        if let Some(expected) = self.expected_upsert {
            let upsert = command.get_bool("upsert").unwrap_or(false);
            ensure!(
                expected == upsert,
                "expected upsert: {expected}, received: {upsert}"
            );
        }
        if let Some(expected) = self.expected_remove {
            let remove = command.get_bool("remove").unwrap_or(false);
            ensure!(
                expected == remove,
                "expected remove: {expected}, received: {remove}"
            );
        }
        if let Some(expected) = self.expected_new {
            let new = command.get_bool("new").unwrap_or(false);
            ensure!(expected == new, "expected new: {expected}, received: {new}");
        }
        Ok(())
    }
}
