use anyhow::{anyhow, ensure};
use bson::Document;

#[derive(Debug, Default)]
pub(crate) struct DeleteExpectation {
    pub expected_filter: Option<Document>,
    pub expected_limit: i32,
}

impl DeleteExpectation {
    pub fn validate(&self, command: &Document) -> anyhow::Result<()> {
        let deletes = command
            .get_array("deletes")?
            .first()
            .and_then(|val| val.as_document())
            .ok_or(anyhow!("deletes not found"))?;
        let limit = deletes.get_i32("limit").unwrap_or(0);
        ensure!(
            self.expected_limit == limit,
            "expected limit: {}, received: {limit}",
            self.expected_limit
        );
        if let Some(expected) = self.expected_filter.as_ref() {
            let filter = deletes.get_document("q")?;
            ensure!(
                expected == filter,
                "expected filter: {expected}, received: {filter}"
            );
        }

        Ok(())
    }
}
