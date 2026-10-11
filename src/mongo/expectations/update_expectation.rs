use anyhow::{anyhow, ensure};
use bson::{Bson, Document};

#[derive(Debug, Default)]
pub(crate) struct UpdateExpectation {
    pub expected_filter: Option<Document>,
    pub expected_update: Option<Bson>,
    pub expected_upsert: Option<bool>,
    pub expected_array_filters: Option<Vec<Bson>>,
    pub expected_multi: bool,
}

impl UpdateExpectation {
    pub fn validate(&self, command: &Document) -> anyhow::Result<()> {
        let updates = command
            .get_array("updates")?
            .first()
            .and_then(|val| val.as_document())
            .ok_or(anyhow!("updates not found"))?;
        let multi = updates.get_bool("multi").unwrap_or(false);
        ensure!(
            self.expected_multi == multi,
            "expected multi: {}, received: {multi}",
            self.expected_multi
        );
        if let Some(expected) = self.expected_filter.as_ref() {
            let filter = updates.get_document("q")?;
            ensure!(
                expected == filter,
                "expected filter: {expected}, received: {filter}"
            );
        }
        if let Some(expected) = self.expected_update.as_ref() {
            let update = updates.get("u").ok_or(anyhow!("field \"u\" not present"))?;
            ensure!(
                expected == update,
                "expected update: {expected}, received: {update}"
            );
        }
        if let Some(expected) = self.expected_upsert {
            let upsert = updates.get_bool("upsert").unwrap_or(false);
            ensure!(
                expected == upsert,
                "expected upsert: {expected}, received: {upsert}"
            );
        }
        if let Some(expected) = self.expected_array_filters.as_ref() {
            let array_filters = updates.get_array("arrayFilters")?;
            ensure!(
                expected == array_filters,
                "expected arrayFilters: {expected:?}, received: {array_filters:?}"
            );
        }

        Ok(())
    }
}
