# testkit-rs

`testkit-rs` provides lightweight, in-process fakes for testing Rust applications with real client libraries. Each fake starts on a random local port, lets a test define responses and expectations, and shuts down when dropped.

> [!IMPORTANT]
> **This crate is intended exclusively for unit tests.** Add it under `[dev-dependencies]` only. The fakes implement limited protocol behavior for controlled test scenarios and are not full service emulators.

- [`FakeMongo`](#fakemongo) — a MongoDB wire-protocol fake for the official `mongodb` client
- [`FakePostgres`](#fakepostgres) — a PostgreSQL wire-protocol fake for `sqlx`
- [`FakeHttp`](#fakehttp) — a local HTTP/HTTPS fake for `reqwest`

No MongoDB or PostgreSQL installation, container, or external HTTP service is required.

## Installation

Add the crate as a development dependency and enable only the fakes your unit tests use:

```toml
[dev-dependencies]
testkit-rs = { version = "0.1", features = ["mongo", "postgres", "http"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

The crate enables no features by default. Enable features individually or combine them as needed:

| Cargo feature | Provides | Works with | Purpose |
| --- | --- | --- | --- |
| `mongo` | `testkit_rs::mongo::FakeMongo` | `mongodb` | Validate MongoDB commands and return configured CRUD, aggregation, and index responses. |
| `postgres` | `testkit_rs::postgres::FakePostgres` | `sqlx` | Match SQL, return rows, command results, or database errors, and record executed queries. |
| `http` | `testkit_rs::http::FakeHttp` | `reqwest` | Match HTTP/HTTPS requests, intercept hosts, and return configured status codes and bodies. |
| `s3` | `testkit_rs::s3::FakeS3` | `aws-sdk-s3` | Return configured objects for `GetObject` and accept expected `PutObject` requests. |
| `ssm` | `testkit_rs::ssm::FakeSsm` | `aws-sdk-ssm` | Return configured Parameter Store values for `GetParameter` and `GetParameters`. |
| `vault` | `testkit_rs::vault::FakeVault` | `vaultrs` | Return configured Vault KV v2 secrets, path listings, and secret metadata. |

## FakeMongo

`FakeMongo` runs a local server that speaks enough of the MongoDB wire protocol for the official `mongodb` client. Tests configure the expected command and response, then exercise application code through a real `mongodb::Client`.

```rust
use bson::doc;
use serde::{Deserialize, Serialize};
use testkit_rs::mongo::FakeMongo;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
struct User {
    id: i32,
    name: String,
}

#[tokio::test]
async fn finds_a_user() {
    let fake = FakeMongo::start().await;
    let expected = User {
        id: 23,
        name: "Alice".into(),
    };

    fake.expect_find_one()
        .database("app")
        .collection("users")
        .filter(doc! { "id": 23 })
        .respond_with(&expected);

    let client = fake.client().await;
    let actual = client
        .database("app")
        .collection::<User>("users")
        .find_one(doc! { "id": 23 })
        .await
        .unwrap()
        .unwrap();

    assert_eq!(actual, expected);
}
```

Use `fake.uri()` to configure application code that accepts a MongoDB connection string, or `fake.db(name).await` for direct access to a database handle.

Available expectation builders cover:

- `find` and `find_one`
- `insert_one` and `insert_many`
- `update_one` and `update_many`
- `delete_one` and `delete_many`
- aggregation and `count_documents`
- `distinct`
- find-and-update, find-and-replace, and find-and-delete
- index creation

Builders can validate command details such as the database, collection, filter, projection, sort, limit, and update document. `validate_with` is available when a test needs custom command validation, and `fail` can simulate a MongoDB error.

## FakePostgres

`FakePostgres` runs a local PostgreSQL-compatible server for `sqlx`. Register responses by SQL, connect a real `PgPool`, and optionally inspect every statement and bind parameter received by the fake.

```rust
use sqlx::Row;
use testkit_rs::postgres::{FakePostgres, PgType, Response};

#[tokio::test]
async fn returns_query_rows() {
    let fake = FakePostgres::start().await.unwrap();

    fake.mock(
        "SELECT id, name FROM users WHERE id = $1",
        Response::rows(
            &[("id", PgType::Int4), ("name", PgType::Text)],
            vec![vec![1.into(), "Alice".into()]],
        ),
    );

    let pool = fake.pool().await;
    let row = sqlx::query("SELECT id, name FROM users WHERE id = $1")
        .bind(1_i32)
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(row.get::<i32, _>("id"), 1);
    assert_eq!(row.get::<String, _>("name"), "Alice");

    let queries = fake.queries();
    assert_eq!(queries[0].sql, "SELECT id, name FROM users WHERE id = $1");
}
```

`FakePostgres` supports three response kinds:

- `Response::rows(...)` returns typed rows.
- `Response::command(...)` returns a command tag such as `"UPDATE 3"`.
- `Response::error(...)` returns a PostgreSQL error with a SQLSTATE code and message.

Matching options are:

- `mock(sql, response)` for normalized exact SQL
- `mock_contains(fragment, response)` for substring matching
- `mock_regex(pattern, response)` for regular-expression matching
- `set_default(response)` for unmatched statements

SQL matching collapses whitespace and removes a trailing semicolon. Rules are checked in registration order, and the first match wins. Without a configured default, unmatched statements receive a successful command response affecting zero rows.

Use `fake.url()` when application code accepts a connection URL, `fake.pool().await` for a ready-to-use pool, and `fake.clear_queries()` to reset recorded query history without removing mocks.

## FakeHttp

`FakeHttp` starts a local HTTP or HTTPS server and creates a `reqwest::Client` configured to reach it. Host interception lets application code keep using its normal external URL while requests are resolved to the fake server.

```rust
use serde::{Deserialize, Serialize};
use testkit_rs::http::FakeHttp;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
struct User {
    id: u64,
    name: String,
}

#[tokio::test]
async fn intercepts_an_https_request() {
    let expected = User {
        id: 1,
        name: "Alice".into(),
    };

    let mut fake = FakeHttp::start_tls().await;
    fake.intercept("api.example.com")
        .expect_get("/api/users")
        .query("id", 1)
        .respond_ok_json(expected.clone());

    let actual = fake
        .client()
        .get("https://api.example.com/api/users")
        .query(&[("id", 1)])
        .send()
        .await
        .unwrap()
        .json::<User>()
        .await
        .unwrap();

    assert_eq!(actual, expected);
}
```

Start the fake with `FakeHttp::start()` for HTTP or `FakeHttp::start_tls()` for HTTPS. For direct requests, use `fake.uri()` instead of host interception.

Expectations are available for `GET`, `POST`, `PUT`, `PATCH`, and `DELETE`. Each expectation can match:

- query parameters with `query`
- headers with `header`
- a JSON body with `json_body`
- custom request rules with `validate_with`

Responses can use `respond_ok`, `respond_with`, `respond_ok_json`, or `respond_with_json` to control the status and body.
