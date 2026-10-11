use anyhow::ensure;
use bson::Document;

#[derive(Debug, Default)]
pub(crate) struct DistinctExpectation {
    pub expected_filter: Option<Document>,
    pub expected_key: Option<String>,
}

impl DistinctExpectation {
    pub fn validate(&self, command: &Document) -> anyhow::Result<()> {
        if let Some(expected) = self.expected_filter.as_ref() {
            let filter = command.get_document("query")?;
            ensure!(
                expected == filter,
                "expected filter: {expected}, received: {filter}"
            );
        }
        if let Some(expected) = self.expected_key.as_ref() {
            let key = command.get_str("key")?;
            ensure!(expected == key, "expected key: {expected}, received: {key}");
        }

        Ok(())
    }
}
