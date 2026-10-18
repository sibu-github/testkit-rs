use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use std::{collections::HashMap, fmt::Display};

use hyper::body::Bytes;
use hyper::header::CONTENT_TYPE;
use hyper::{Method, StatusCode};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::Serialize;
use serde_json::Value as JsonValue;

use crate::http::HttpRequest;
use crate::http::expectations::{HttpExpectation, Validator};
use crate::http::state::{HttpResponse, State};

pub struct HttpExpectationBuilder {
    pub(crate) state: Arc<Mutex<State>>,
    pub(crate) method: Method,
    pub(crate) path: String,
    pub(crate) expected_query: HashMap<String, String>,
    pub(crate) expected_headers: HeaderMap,
    pub(crate) expected_json_body: Option<JsonValue>,
    pub(crate) validator: Option<Validator>,
}

impl HttpExpectationBuilder {
    pub(crate) fn new(state: Arc<Mutex<State>>, method: Method, path: &str) -> Self {
        Self {
            state,
            method,
            path: path.to_string(),
            expected_query: HashMap::new(),
            expected_headers: HeaderMap::new(),
            expected_json_body: None,
            validator: None,
        }
    }

    pub fn query<K, V>(mut self, key: K, val: V) -> Self
    where
        K: Display,
        V: Display,
    {
        self.expected_query.insert(key.to_string(), val.to_string());
        self
    }

    pub fn header<K, V>(mut self, key: K, val: V) -> Self
    where
        K: TryInto<HeaderName>,
        K::Error: Debug,
        V: TryInto<HeaderValue>,
        V::Error: Debug,
    {
        let key = key.try_into().expect("invalid header name");
        let val = val.try_into().expect("invalid header value");
        self.expected_headers.insert(key, val);
        self
    }

    pub fn json_body(mut self, body: JsonValue) -> Self {
        self.expected_json_body = Some(body);
        self
    }

    pub fn validate_with<F>(mut self, validator: F) -> Self
    where
        F: Fn(&HttpRequest) -> anyhow::Result<()> + Send + Sync + 'static,
    {
        self.validator = Some(Box::new(validator));
        self
    }

    pub fn respond_ok<S>(self, body: S)
    where
        S: Into<Bytes>,
    {
        let response = HttpResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: body.into(),
        };
        self.add_expectation(response);
    }

    pub fn respond_with<S>(self, status: StatusCode, body: S)
    where
        S: Into<Bytes>,
    {
        let response = HttpResponse {
            status,
            headers: HeaderMap::new(),
            body: body.into(),
        };
        self.add_expectation(response);
    }

    pub fn respond_ok_json<V>(self, body: V)
    where
        V: Serialize,
    {
        let body = serde_json::to_vec(&body).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let response = HttpResponse {
            status: StatusCode::OK,
            headers,
            body: body.into(),
        };
        self.add_expectation(response);
    }

    pub fn respond_with_json<V>(self, status: StatusCode, body: V)
    where
        V: Serialize,
    {
        let body = serde_json::to_vec(&body).unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let response = HttpResponse {
            status,
            headers,
            body: body.into(),
        };
        self.add_expectation(response);
    }

    fn add_expectation(self, response: HttpResponse) {
        let expectation = HttpExpectation {
            expected_query: self.expected_query,
            expected_header: self.expected_headers,
            expected_json_body: self.expected_json_body,
            validator: self.validator,
            response,
        };
        let key = (self.method, self.path);
        self.state
            .lock()
            .unwrap()
            .insert_expectation(key, expectation);
    }
}
