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
pub struct FakeHttp {
    intercepted_hosts: Vec<String>,
    is_tls: bool,
    port: u16,
    state: Arc<Mutex<State>>,
    handle: JoinHandle<()>,
}

impl FakeHttp {
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

    pub fn intercept(&mut self, host: impl Into<String>) -> &mut Self {
        self.intercepted_hosts.push(host.into());
        self
    }

    pub fn uri(&self) -> String {
        let port = self.port;
        if self.is_tls {
            format!("https://127.0.0.1:{port}")
        } else {
            format!("http://127.0.0.1:{port}")
        }
    }

    pub fn addr(&self) -> SocketAddr {
        let port = self.port;
        format!("127.0.0.1:{port}").parse().unwrap()
    }

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

    pub fn expect_get(&self, path: &str) -> HttpExpectationBuilder {
        HttpExpectationBuilder::new(self.state.clone(), Method::GET, path)
    }

    pub fn expect_post(&self, path: &str) -> HttpExpectationBuilder {
        HttpExpectationBuilder::new(self.state.clone(), Method::POST, path)
    }

    pub fn expect_put(&self, path: &str) -> HttpExpectationBuilder {
        HttpExpectationBuilder::new(self.state.clone(), Method::PUT, path)
    }

    pub fn expect_delete(&self, path: &str) -> HttpExpectationBuilder {
        HttpExpectationBuilder::new(self.state.clone(), Method::DELETE, path)
    }

    pub fn expect_patch(&self, path: &str) -> HttpExpectationBuilder {
        HttpExpectationBuilder::new(self.state.clone(), Method::PATCH, path)
    }
}

impl Drop for FakeHttp {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
