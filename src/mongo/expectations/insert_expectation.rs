use anyhow::{anyhow, ensure};
use bson::Document;

#[derive(Debug, Default)]
pub(crate) struct InsertExpectation {
    pub expected_documents: Option<Vec<Document>>,
    pub expected_ordered: Option<bool>,
}

impl InsertExpectation {
    pub fn validate(&self, command: &Document) -> anyhow::Result<()> {
        if let Some(expected) = self.expected_documents.as_ref() {
            let documents = command.get_array("documents")?;
            ensure!(
                expected.len() == documents.len(),
                "expected document len not matching"
            );
            for (idx, doc) in expected.iter().enumerate() {
                let actual = documents[idx]
                    .as_document()
                    .ok_or(anyhow!("value is not document"))?;
                assert_document_contains(actual, doc)?;
            }
        }
        if let Some(expected) = self.expected_ordered {
            let ordered = command.get_bool("ordered")?;
            ensure!(
                ordered == expected,
                "expected skip: {expected}, received: {ordered}"
            );
        }

        Ok(())
    }
}

fn assert_document_contains(actual: &Document, expected: &Document) -> anyhow::Result<()> {
    for (key, expected_value) in expected {
        let actual_value = actual.get(key).ok_or(anyhow!("{key} not found"))?;
        ensure!(
            actual_value == expected_value,
            "unexpected value for field `{key}`: {actual_value}"
        );
    }

    Ok(())
}
