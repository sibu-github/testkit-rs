# testkit-rs
A collection of lightweight fake implementations and testing utilities for Rust applications.


> **Work in progress:** `testkit-rs` is currently under active development. The initial implementation focuses on `FakeMongo`.

## Overview

`testkit-rs` provides lightweight fake implementations of external services that can be used in Rust unit and integration tests.

The goal is to test application code using the **real client libraries** while replacing the external service with a local, deterministic fake.

For example, instead of mocking the MongoDB Rust driver API directly, `FakeMongo` starts a local TCP server and allows the official MongoDB Rust driver to connect to it normally.

```text
┌──────────────────────┐
│   Application Test   │
│                      │
│  mongodb::Client     │
└──────────┬───────────┘
           │
           │ MongoDB wire protocol
           │
           ▼
┌──────────────────────┐
│      FakeMongo       │
│                      │
│  localhost:<port>    │
│                      │
│  Expectations        │
│  Validators          │
│  Responses           │
└──────────────────────┘
```

This means the code under test does not need to know that MongoDB is fake.

## Why FakeMongo?

Traditional mocking approaches often require mocking the application's database abstraction:

```rust
trait UserRepository {
    async fn find_user(&self, id: u64) -> Result<User>;
}
```

While this can work, it means tests are testing the mocked abstraction rather than the actual MongoDB interaction.

With `FakeMongo`, application code can continue using the normal MongoDB driver:

```rust
let fake = testkit_rs::mongo::FakeMongo::start().await;
let client = fake.client().await;

let collection = client
    .database("testdb")
    .collection::<User>("users");

let user = collection
    .find_one(doc! { "id": 1 })
    .await
    .unwrap();
```

The MongoDB driver still serializes the command, opens a connection, sends the MongoDB wire protocol message, and deserializes the response.

Only the MongoDB server itself is replaced.

This makes tests useful for verifying that application code is actually issuing the expected MongoDB operations.

The official Rust driver supports typed collections, `find`, `find_one`, `insert_one`, `insert_many`, and other CRUD operations, which makes this approach particularly useful for integration-style unit tests.

## Features

### FakeMongo

`FakeMongo` provides:

- Local MongoDB-compatible TCP server
- Random available port
- Real MongoDB Rust driver compatibility
- Command expectations
- Query validation
- Typed responses
- Support for multiple concurrent connections
- Automatic handling of MongoDB driver handshake commands
- Deterministic test failures when expectations are not satisfied

The fake does **not** require a MongoDB installation.

## Installation

Add `testkit-rs` as a development dependency:

```toml
[dev-dependencies]
testkit-rs = "0.1"
```

> The crate version will change while the project is under development.

## Quick Start

### Find

```rust
#[tokio::test]
async fn find_user() {
    let fake = FakeMongo::start().await;
    fake.expect_find()
        .database("testdb")
        .collection("users")
        .filter(doc! { "active": true })
        .respond_with(vec![
                doc! { "id": 1, "name": "Alice" },
                doc! { "id": 2, "name": "Bob" },
            ]);
    let client = fake.client().await;
    let users = find_users(&client).await.unwrap();
    assert_eq!(user.len(), 2);
}
```
