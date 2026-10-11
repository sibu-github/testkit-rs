use anyhow::ensure;
use bson::Document;

#[derive(Debug, Default)]
pub(crate) struct FindExpectation {
    pub expected_filter: Option<Document>,
    pub expected_sort: Option<Document>,
    pub expected_project: Option<Document>,
    pub expected_limit: Option<i64>,
    pub expected_skip: Option<i64>,
}

impl FindExpectation {
    pub fn validate(&self, command: &Document) -> anyhow::Result<()> {
        if let Some(expected) = self.expected_filter.as_ref() {
            let filter = command.get_document("filter")?;
            ensure!(
                expected == filter,
                "expected filter: {expected}, received: {filter}"
            );
        }
        if let Some(expected) = self.expected_sort.as_ref() {
            let sort = command.get_document("sort")?;
            ensure!(
                expected == sort,
                "expected sort: {expected}, received: {sort}"
            );
        }
        if let Some(expected) = self.expected_project.as_ref() {
            let project = command.get_document("projection")?;
            ensure!(
                expected == project,
                "expected project: {expected}, received: {project}"
            );
        }
        if let Some(expected) = self.expected_limit {
            let limit = command.get_i64("limit").unwrap();
            ensure!(
                expected == limit,
                "expected limit: {expected}, received: {limit}"
            );
        }
        if let Some(expected) = self.expected_skip {
            let skip = command.get_i64("skip")?;
            ensure!(
                expected == skip,
                "expected skip: {expected}, received: {skip}"
            );
        }

        Ok(())
    }
}
