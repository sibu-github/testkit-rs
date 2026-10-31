use std::net::SocketAddr;
use std::sync::Arc;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tokio::task::{JoinHandle, JoinSet};

use crate::postgres::server;
use crate::postgres::state::{RecordedQuery, Response, State};

/// An in-process PostgreSQL wire-protocol server for tests.
///
/// `FakePostgres` listens on a random localhost port and supports connections
/// from [`sqlx::PgPool`]. Tests can register responses for SQL statements and
/// inspect the queries received by the server without running PostgreSQL or a
/// container.
///
/// SQL is normalized by collapsing whitespace and removing a trailing
/// semicolon before matching. Rules are evaluated in registration order, and
/// the first matching rule supplies the response. Statements that match no
/// rule receive a successful command response affecting zero rows unless a
/// default response has been configured with [`FakePostgres::set_default`].
///
/// The server and all open connections shut down when this value is dropped.
///
/// # Example
///
/// ```no_run
/// use sqlx::Row;
/// use testkit_rs::postgres::{FakePostgres, PgType, Response};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let postgres = FakePostgres::start().await?;
/// postgres.mock(
///     "SELECT id, name FROM users WHERE id = $1",
///     Response::rows(
///         &[("id", PgType::Int4), ("name", PgType::Text)],
///         vec![vec![1.into(), "Alice".into()]],
///     ),
/// );
///
/// let pool = postgres.pool().await;
/// let row = sqlx::query("SELECT id, name FROM users WHERE id = $1")
///     .bind(1_i32)
///     .fetch_one(&pool)
///     .await?;
///
/// assert_eq!(row.get::<i32, _>("id"), 1);
/// assert_eq!(row.get::<String, _>("name"), "Alice");
/// # Ok(())
/// # }
/// ```
pub struct FakePostgres {
    addr: SocketAddr,
    state: Arc<State>,
    handle: JoinHandle<()>,
}

impl FakePostgres {
    /// Starts a fake PostgreSQL server on a random available localhost port.
    ///
    /// The server runs in a background Tokio task until this value is dropped.
    ///
    /// # Errors
    ///
    /// Returns an error if the TCP listener cannot bind to localhost or its
    /// bound address cannot be read.
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

    /// Returns the local socket address on which the server is listening.
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Returns the local TCP port on which the server is listening.
    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    /// Returns a PostgreSQL connection URL for this server.
    ///
    /// The URL disables TLS and contains placeholder credentials. The fake
    /// accepts the credentials without authentication.
    pub fn url(&self) -> String {
        format!(
            "postgres://postgres:postgres@127.0.0.1:{}/postgres?sslmode=disable",
            self.port()
        )
    }

    /// Creates a [`sqlx::PgPool`] connected to this server.
    ///
    /// The pool permits up to three concurrent connections.
    ///
    /// # Panics
    ///
    /// Panics if the pool cannot connect to the local fake server.
    pub async fn pool(&self) -> PgPool {
        PgPoolOptions::new()
            .max_connections(3)
            .connect(&self.url())
            .await
            .unwrap()
    }

    /// Registers `response` for statements whose normalized SQL equals `sql`.
    ///
    /// Whitespace is collapsed and a trailing semicolon is removed before
    /// comparison. Rules are checked in registration order.
    pub fn mock(&self, sql: &str, response: Response) -> &Self {
        self.state.mock(sql, response);
        self
    }

    /// Registers `response` for statements containing `fragment` after SQL
    /// normalization.
    ///
    /// Rules are checked in registration order.
    pub fn mock_contains(&self, fragment: &str, response: Response) -> &Self {
        self.state.mock_contains(fragment, response);
        self
    }

    /// Registers `response` for normalized SQL matching the regular expression
    /// `pattern`.
    ///
    /// Rules are checked in registration order.
    ///
    /// # Panics
    ///
    /// Panics if `pattern` is not a valid regular expression.
    pub fn mock_regex(&self, pattern: &str, response: Response) -> &Self {
        self.state.mock_regex(pattern, response);
        self
    }

    /// Sets the response returned when no registered rule matches a statement.
    ///
    /// Without a default, unmatched statements receive a successful command
    /// tag derived from their SQL, such as `INSERT 0 0`, `SELECT 0`, or `BEGIN`.
    pub fn set_default(&self, response: Response) -> &Self {
        self.state.set_default(response);
        self
    }

    /// Returns all statements received by the server in execution order.
    ///
    /// Each recorded query contains normalized SQL and the raw, wire-encoded
    /// bind parameters sent by the client.
    pub fn queries(&self) -> Vec<RecordedQuery> {
        self.state.queries()
    }

    /// Clears the recorded query history without removing registered rules or
    /// the default response.
    pub fn clear_queries(&self) {
        self.state.clear_queries();
    }
}

impl Drop for FakePostgres {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
