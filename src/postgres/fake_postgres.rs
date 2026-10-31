//! # FakePostgres
//!
//! A tiny in-process fake Postgres server. It listens on a free local TCP port,
//! speaks enough of the Postgres wire protocol for `sqlx::PgPool`, answers queries
//! with mock results you register by SQL text, and replies with a plain success
//! (`CommandComplete`) to anything you didn't mock.
//!
//! ```no_run
//! use testkit_rs::postgres::{FakePostgres, PgType, Response};
//!
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! let pg = FakePostgres::start().await?;
//! pg.mock(
//!     "SELECT id, name FROM users WHERE id = $1",
//!     Response::rows(
//!         &[("id", PgType::Int4), ("name", PgType::Text)],
//!         vec![vec![1.into(), "alice".into()]],
//!     ),
//! );
//! let pool = sqlx::PgPool::connect(&pg.url()).await?;
//! # Ok(()) }
//! ```
//!
//! Matching is on SQL text with whitespace collapsed and a trailing `;` removed.
//! Rules are checked in registration order; the first match wins.

use std::net::SocketAddr;
use std::sync::Arc;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tokio::task::{JoinHandle, JoinSet};

use crate::postgres::server;
use crate::postgres::state::{RecordedQuery, Response, State};

/// A fake Postgres server bound to a free port on 127.0.0.1.
/// The server (and all its connections) shut down when this value is dropped.
pub struct FakePostgres {
    addr: SocketAddr,
    state: Arc<State>,
    handle: JoinHandle<()>,
}

impl FakePostgres {
    /// Binds `127.0.0.1:0` and starts serving in the background.
    pub async fn start() -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let state = Arc::new(State::new());
        let handle = {
            let state = state.clone();
            tokio::spawn(async move {
                // Dropping the JoinSet (when this task is aborted) aborts every connection.
                let mut conns = JoinSet::new();
                loop {
                    tokio::select! {
                        accepted = listener.accept() => {
                            if let Ok((sock, _)) = accepted {
                                let _ = sock.set_nodelay(true);
                                conns.spawn(server::serve(sock, state.clone()));
                            }
                        }
                        Some(_) = conns.join_next(), if !conns.is_empty() => {}
                    }
                }
            })
        };

        Ok(Self {
            addr,
            state,
            handle,
        })
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    /// Connection URL for `PgPool::connect` (no TLS, no password check).
    pub fn url(&self) -> String {
        format!(
            "postgres://postgres:postgres@127.0.0.1:{}/postgres?sslmode=disable",
            self.port()
        )
    }

    pub async fn pool(&self) -> PgPool {
        PgPoolOptions::new()
            .max_connections(3)
            .connect(&self.url())
            .await
            .unwrap()
    }

    /// Mock queries whose normalized SQL equals `sql`.
    pub fn mock(&self, sql: &str, response: Response) -> &Self {
        self.state.mock(sql, response);
        self
    }

    /// Mock queries whose normalized SQL contains `fragment`.
    pub fn mock_contains(&self, fragment: &str, response: Response) -> &Self {
        self.state.mock_contains(fragment, response);
        self
    }

    /// Mock queries whose normalized SQL matches the regex `pattern`.
    pub fn mock_regex(&self, pattern: &str, response: Response) -> &Self {
        self.state.mock_regex(pattern, response);
        self
    }

    /// Response for queries no rule matches. Without this, unmatched queries get a
    /// plain success tag derived from the SQL (`INSERT 0 0`, `SELECT 0`, `BEGIN`, ...).
    pub fn set_default(&self, response: Response) -> &Self {
        self.state.set_default(response);
        self
    }

    /// Every statement executed so far, in order.
    pub fn queries(&self) -> Vec<RecordedQuery> {
        self.state.queries()
    }

    pub fn clear_queries(&self) {
        self.state.clear_queries();
    }
}

impl Drop for FakePostgres {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
