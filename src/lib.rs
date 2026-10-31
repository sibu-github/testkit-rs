//! Lightweight, in-process fakes for testing Rust applications with real
//! service clients.
//!
//! `testkit-rs` provides local protocol-level substitutes for external
//! databases and services. Each fake binds to a random loopback port, runs in a
//! background Tokio task, and shuts down when its owning value is dropped. This
//! allows application code to exercise its normal client, serialization, and
//! request-building paths without starting containers or contacting external
//! systems.
//!
//! The fakes implement only the protocol operations needed for focused tests;
//! they are not complete service emulators and must not be used in production.
//!
//! # Feature flags
//!
//! The crate has no default features. Enable only the services used by a test:
//!
//! - `mongo` provides `FakeMongo` for the official MongoDB client.
//! - `postgres` provides `FakePostgres` for `sqlx`.
//! - `http` provides `FakeHttp` for `reqwest`.
//! - `s3` provides `FakeS3` for the AWS SDK for Amazon S3.
//! - `ssm` provides `FakeSsm` for the AWS Systems Manager SDK.
//! - `vault` provides `FakeVault` for the `vaultrs` client.
//!
//! ```toml
//! [dev-dependencies]
//! testkit-rs = { version = "0.1", features = ["mongo", "postgres", "http", "s3", "ssm", "vault"] }
//! ```

#[cfg(feature = "mongo")]
/// A MongoDB wire-protocol fake and operation expectation builders.
pub mod mongo;

#[cfg(feature = "http")]
/// An HTTP/HTTPS fake with request expectations and host interception.
pub mod http;

#[cfg(feature = "ssm")]
/// An AWS Systems Manager Parameter Store fake for retrieving test parameters.
pub mod ssm;

#[cfg(feature = "vault")]
/// A HashiCorp Vault KV v2 fake for secrets, listings, and metadata.
pub mod vault;

#[cfg(feature = "s3")]
/// An Amazon S3 fake for focused object GET and PUT tests.
pub mod s3;

#[cfg(feature = "postgres")]
/// A PostgreSQL wire-protocol fake with configurable SQL responses.
pub mod postgres;
