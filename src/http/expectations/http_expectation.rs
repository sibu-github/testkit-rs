use std::collections::HashMap;

use hyper::HeaderMap;
use serde_json::Value as JsonValue;

use crate::http::{HttpRequest, state::HttpResponse};

pub(crate) type Validator = Box<dyn Fn(&HttpRequest) -> anyhow::Result<()> + Send + Sync>;

pub(crate) struct HttpExpectation {
    pub(crate) expected_query: HashMap<String, String>,
    pub(crate) expected_header: HeaderMap,
    pub(crate) expected_json_body: Option<JsonValue>,
    pub(crate) validator: Option<Validator>,
    pub(crate) response: HttpResponse,
}

impl HttpExpectation {
    pub async fn validate_and_respond(&self, req: &HttpRequest) -> anyhow::Result<HttpResponse> {
        // Validate query parameters.
        for (key, expected) in &self.expected_query {
            let actual = req
                .query
                .get(key)
                .ok_or_else(|| anyhow::anyhow!("Missing query parameter `{key}`"))?;

            anyhow::ensure!(
                actual == expected,
                "Query parameter `{key}` mismatch: expected `{expected}`, got `{actual}`"
            );
        }

        // Validate expected headers.
        for (name, expected) in &self.expected_header {
            let actual = req
                .headers
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("Missing header `{name}`"))?;

            anyhow::ensure!(
                actual == expected,
                "Header `{name}` mismatch: expected `{expected:?}`, got `{actual:?}`"
            );
        }

        // Validate body, if configured.
        if let Some(expected) = &self.expected_json_body {
            let actual = serde_json::from_slice::<JsonValue>(&req.body)?;
            anyhow::ensure!(
                actual == *expected,
                "Request body mismatch: expected {expected}, got {actual}",
            );
        }

        // Run custom validation.
        if let Some(validator) = &self.validator {
            validator(req)?;
        }

        Ok(self.response.clone())
    }
}
