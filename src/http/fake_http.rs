use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use hyper::Method;
use tokio::{net::TcpListener, task::JoinHandle};
use tokio_rustls::{
    TlsAcceptor,
    rustls::{
        ServerConfig,
        crypto::aws_lc_rs::default_provider,
        pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer},
    },
};

use crate::http::{
    expectations::HttpExpectationBuilder,
    server::{serve, serve_tls},
    state::State,
};

const TIMEOUT: Duration = Duration::from_secs(2);

/// An in-process HTTP or HTTPS server for tests.
///
/// `FakeHttp` listens on a random localhost port and responds to requests using
/// expectations registered by HTTP method and path. Use [`FakeHttp::client`] to
/// create a [`reqwest::Client`] configured for direct requests or for hosts
/// added with [`FakeHttp::intercept`].
///
/// The generated client has a two-second timeout and accepts the self-signed
/// certificate used by [`FakeHttp::start_tls`]. It is intended only for tests.
/// The server shuts down when the `FakeHttp` value is dropped.
///
/// # Example
///
/// ```no_run
/// use testkit_rs::http::FakeHttp;
///
/// # async fn example() -> Result<(), reqwest::Error> {
/// let fake = FakeHttp::start().await;
/// fake.expect_get("/health").respond_ok("ok");
///
/// let response = fake
///     .client()
///     .get(format!("{}/health", fake.uri()))
///     .send()
///     .await?;
///
/// assert_eq!(response.text().await?, "ok");
/// # Ok(())
/// # }
/// ```
pub struct FakeHttp {
    intercepted_hosts: Vec<String>,
    is_tls: bool,
    port: u16,
    state: Arc<Mutex<State>>,
    handle: JoinHandle<()>,
}

impl FakeHttp {
    /// Starts an HTTP server on a random available localhost port.
    ///
    /// The server runs in a background Tokio task until this value is dropped.
    ///
    /// # Panics
    ///
    /// Panics if the server cannot bind to a local port or inspect its bound
    /// address.
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let state = Arc::new(Mutex::new(State::default()));
        let handle = Self::listen(listener, state.clone(), None);
        Self {
            intercepted_hosts: vec![],
            is_tls: false,
            port,
            state,
            handle,
        }
    }

    /// Starts an HTTPS server on a random available localhost port.
    ///
    /// The server uses a newly generated self-signed certificate valid for
    /// `127.0.0.1` and `localhost`. [`FakeHttp::client`] is configured to accept
    /// this certificate.
    ///
    /// # Panics
    ///
    /// Panics if the server cannot bind to a local port, its bound address
    /// cannot be inspected, or the TLS certificate and server configuration
    /// cannot be generated.
    pub async fn start_tls() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let state = Arc::new(Mutex::new(State::default()));
        let server_config = Self::server_config();
        let acceptor = TlsAcceptor::from(Arc::new(server_config));
        let handle = Self::listen(listener, state.clone(), Some(acceptor));
        Self {
            intercepted_hosts: vec![],
            is_tls: true,
            port,
            state,
            handle,
        }
    }

    fn server_config() -> ServerConfig {
        let subjects = ["127.0.0.1".to_string(), "localhost".to_string()];
        let cert_key = rcgen::generate_simple_self_signed(subjects).unwrap();
        let certs = vec![cert_key.cert.der().clone()];
        let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
            cert_key.signing_key.serialize_der(),
        ));
        ServerConfig::builder_with_provider(Arc::new(default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .unwrap()
    }

    fn listen(
        listener: TcpListener,
        state: Arc<Mutex<State>>,
        tls: Option<TlsAcceptor>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            match tls {
                Some(acceptor) => serve_tls(listener, acceptor, state).await,
                None => serve(listener, state).await,
            }
        })
    }

    /// Configures clients created by [`FakeHttp::client`] to resolve `host` to
    /// this fake server.
    ///
    /// This allows application code to keep using its normal external URL while
    /// routing requests to the fake. Multiple hosts can be registered by calling
    /// this method repeatedly.
    pub fn intercept(&mut self, host: impl Into<String>) -> &mut Self {
        self.intercepted_hosts.push(host.into());
        self
    }

    /// Returns the base URI for direct requests to this server.
    ///
    /// The URI uses `http` or `https` according to how the server was started.
    pub fn uri(&self) -> String {
        let port = self.port;
        if self.is_tls {
            format!("https://127.0.0.1:{port}")
        } else {
            format!("http://127.0.0.1:{port}")
        }
    }

    /// Returns the local socket address on which the server is listening.
    pub fn addr(&self) -> SocketAddr {
        let port = self.port;
        format!("127.0.0.1:{port}").parse().unwrap()
    }

    /// Creates a [`reqwest::Client`] configured to send requests to this server.
    ///
    /// Registered intercepted hosts resolve to the fake's local address. The
    /// client uses a two-second timeout and disables certificate and hostname
    /// validation so it can connect to the self-signed TLS server.
    ///
    /// This client configuration is intentionally insecure and must only be
    /// used in tests.
    ///
    /// # Panics
    ///
    /// Panics if the client cannot be built.
    pub fn client(&self) -> reqwest::Client {
        let addr = self.addr();
        let mut builder = reqwest::ClientBuilder::new()
            .timeout(TIMEOUT)
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true);
        for host in self.intercepted_hosts.iter() {
            builder = builder.resolve(host, addr);
        }
        builder.build().unwrap()
    }

    /// Begins configuring an expected `GET` request to `path`.
    ///
    /// Complete the builder with a response method to register the expectation.
    pub fn expect_get(&self, path: &str) -> HttpExpectationBuilder {
        HttpExpectationBuilder::new(self.state.clone(), Method::GET, path)
    }

    /// Begins configuring an expected `POST` request to `path`.
    ///
    /// Complete the builder with a response method to register the expectation.
    pub fn expect_post(&self, path: &str) -> HttpExpectationBuilder {
        HttpExpectationBuilder::new(self.state.clone(), Method::POST, path)
    }

    /// Begins configuring an expected `PUT` request to `path`.
    ///
    /// Complete the builder with a response method to register the expectation.
    pub fn expect_put(&self, path: &str) -> HttpExpectationBuilder {
        HttpExpectationBuilder::new(self.state.clone(), Method::PUT, path)
    }

    /// Begins configuring an expected `DELETE` request to `path`.
    ///
    /// Complete the builder with a response method to register the expectation.
    pub fn expect_delete(&self, path: &str) -> HttpExpectationBuilder {
        HttpExpectationBuilder::new(self.state.clone(), Method::DELETE, path)
    }

    /// Begins configuring an expected `PATCH` request to `path`.
    ///
    /// Complete the builder with a response method to register the expectation.
    pub fn expect_patch(&self, path: &str) -> HttpExpectationBuilder {
        HttpExpectationBuilder::new(self.state.clone(), Method::PATCH, path)
    }
}

impl Drop for FakeHttp {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
