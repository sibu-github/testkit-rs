use anyhow::{anyhow, ensure};
use bson::{Bson, Document};

use crate::mongo::expectations::same_key_order;

#[derive(Debug, Default)]
pub(crate) struct AggregateExpectation {
    pub expected_pipeline: Option<Vec<Bson>>,
    pub expected_count_query: Option<Document>,
}

impl AggregateExpectation {
    pub fn validate(&self, command: &Document) -> anyhow::Result<()> {
        if let Some(expected) = self.expected_pipeline.as_ref() {
            let pipeline = command.get_array("pipeline")?;
            ensure!(
                expected == pipeline,
                "expected pipeline: {expected:?}, received: {pipeline:?}"
            );
            // Pipeline equality ignores key order, which matters inside `$sort`.
            for (expected, actual) in expected.iter().zip(pipeline) {
                if let (Some(expected), Some(actual)) = (sort_of(expected), sort_of(actual)) {
                    ensure!(
                        same_key_order(expected, actual),
                        "expected $sort stage: {expected}, received: {actual}"
                    );
                }
            }
        }
        if let Some(expected) = self.expected_count_query.as_ref() {
            let query = command
                .get_array("pipeline")?
                .first()
                .and_then(|v| v.as_document())
                .and_then(|v| v.get_document("$match").ok())
                .ok_or(anyhow!("unable to find $match"))?;
            ensure!(
                expected == query,
                "expected query: {expected}, received: {query}"
            );
        }

        Ok(())
    }
}

fn sort_of(stage: &Bson) -> Option<&Document> {
    stage.as_document()?.get_document("$sort").ok()
}
