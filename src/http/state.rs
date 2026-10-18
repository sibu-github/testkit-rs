use std::collections::HashMap;
use std::collections::VecDeque;

use hyper::body::Bytes;
use reqwest::Method;
use reqwest::StatusCode;
use reqwest::header::HeaderMap;

use crate::http::expectations::HttpExpectation;

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: Method,
    pub path: String,
    pub query: HashMap<String, String>,
    pub headers: HeaderMap,
    pub body: Bytes,
}

#[derive(Debug, Clone)]
pub(crate) struct HttpResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}

impl From<HttpResponse> for hyper::Response<http_body_util::Full<Bytes>> {
    fn from(r: HttpResponse) -> Self {
        let mut res = hyper::Response::new(http_body_util::Full::new(r.body));
        *res.status_mut() = r.status;
        *res.headers_mut() = r.headers;
        res
    }
}

pub(crate) type RequestKey = (Method, String); // (method, path)

#[derive(Default)]
pub(crate) struct State {
    pub expectations: HashMap<RequestKey, VecDeque<HttpExpectation>>,
}

impl State {
    pub fn get_expetation(&mut self, key: &RequestKey) -> Option<HttpExpectation> {
        self.expectations.get_mut(key).and_then(|e| e.pop_front())
    }

    pub fn insert_expectation(&mut self, key: RequestKey, expectation: HttpExpectation) {
        self.expectations
            .entry(key)
            .or_default()
            .push_back(expectation);
    }
}
