use std::sync::Mutex;

use regex::Regex;

use crate::postgres::{
    server::normalize,
    types::{Column, PgType, Value},
};

/// A statement the server executed, as seen on the wire.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordedQuery {
    /// Normalized SQL (whitespace collapsed, trailing `;` removed).
    pub sql: String,
    /// Raw bind parameters (binary-encoded by sqlx); `None` is SQL NULL.
    pub params: Vec<Option<Vec<u8>>>,
}

/// What the fake server answers for a matched query.
#[derive(Clone, Debug)]
pub enum Response {
    /// A result set. Sent as `RowDescription` + `DataRow`s + `CommandComplete`.
    Rows {
        columns: Vec<Column>,
        rows: Vec<Vec<Value>>,
    },
    /// No rows, just a command tag such as `"INSERT 0 1"` or `"UPDATE 3"`.
    Command(String),
    /// An `ErrorResponse` with the given SQLSTATE code (e.g. `"23505"`).
    Error { code: String, message: String },
}

impl Response {
    pub fn rows(columns: &[(&str, PgType)], rows: Vec<Vec<Value>>) -> Self {
        Response::Rows {
            columns: columns
                .iter()
                .map(|(name, ty)| Column {
                    name: (*name).to_owned(),
                    ty: *ty,
                })
                .collect(),
            rows,
        }
    }

    pub fn command(tag: impl Into<String>) -> Self {
        Response::Command(tag.into())
    }

    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Response::Error {
            code: code.into(),
            message: message.into(),
        }
    }
}

enum Matcher {
    Exact(String),
    Contains(String),
    Regex(Regex),
}

impl Matcher {
    fn is_match(&self, sql: &str) -> bool {
        match self {
            Matcher::Exact(s) => s == sql,
            Matcher::Contains(s) => sql.contains(s.as_str()),
            Matcher::Regex(r) => r.is_match(sql),
        }
    }
}

struct Rule {
    matcher: Matcher,
    response: Response,
}

#[derive(Default)]
struct Inner {
    rules: Vec<Rule>,
    default: Option<Response>,
    log: Vec<RecordedQuery>,
}

pub(crate) struct State {
    inner: Mutex<Inner>,
}

impl State {
    pub(crate) fn new() -> Self {
        Self {
            inner: Mutex::new(Inner::default()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn add(&self, matcher: Matcher, response: Response) {
        self.lock().rules.push(Rule { matcher, response });
    }

    /// Mock queries whose normalized SQL equals `sql`.
    pub(crate) fn mock(&self, sql: &str, response: Response) {
        self.add(Matcher::Exact(normalize(sql)), response);
    }

    /// Mock queries whose normalized SQL contains `fragment`.
    pub(crate) fn mock_contains(&self, fragment: &str, response: Response) {
        self.add(Matcher::Contains(normalize(fragment)), response)
    }

    /// Mock queries whose normalized SQL matches the regex `pattern`.
    pub(crate) fn mock_regex(&self, pattern: &str, response: Response) {
        let regex = Regex::new(pattern).unwrap();
        self.add(Matcher::Regex(regex), response);
    }

    pub(crate) fn set_default(&self, response: Response) {
        self.lock().default = Some(response);
    }

    /// Every statement executed so far, in order.
    pub(crate) fn queries(&self) -> Vec<RecordedQuery> {
        self.lock().log.clone()
    }

    pub(crate) fn clear_queries(&self) {
        self.lock().log.clear();
    }

    pub(crate) fn has_match(&self, sql: &str) -> bool {
        self.lock().rules.iter().any(|r| r.matcher.is_match(sql))
    }

    /// First matching mock, else the configured default, else a generic success tag.
    pub(crate) fn resolve(&self, sql: &str) -> Response {
        let inner = self.lock();
        inner
            .rules
            .iter()
            .find(|r| r.matcher.is_match(sql))
            .map(|r| r.response.clone())
            .or_else(|| inner.default.clone())
            .unwrap_or_else(|| Response::Command(command_tag(sql, 0)))
    }

    pub(crate) fn record(&self, sql: &str, params: &[Option<Vec<u8>>]) {
        self.lock().log.push(RecordedQuery {
            sql: sql.to_owned(),
            params: params.to_vec(),
        });
    }
}

/// The CommandComplete tag Postgres would send for this statement with `n` rows.
pub(crate) fn command_tag(sql: &str, n: usize) -> String {
    let mut words = sql.split_whitespace().map(|w| w.to_ascii_uppercase());
    let first = words.next().unwrap_or_default();
    match first.as_str() {
        "SELECT" | "VALUES" | "WITH" => format!("SELECT {n}"),
        "INSERT" => format!("INSERT 0 {n}"),
        "UPDATE" | "DELETE" | "MERGE" | "COPY" | "MOVE" | "FETCH" => format!("{first} {n}"),
        "START" => "BEGIN".into(),
        "END" => "COMMIT".into(),
        "ABORT" => "ROLLBACK".into(),
        "CREATE" | "DROP" | "ALTER" => match words.next() {
            Some(second) => format!("{first} {second}"),
            None => first,
        },
        _ => first,
    }
}
