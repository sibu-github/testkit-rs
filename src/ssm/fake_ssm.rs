use std::sync::{Arc, Mutex};

use aws_sdk_ssm::Client;
use tokio::{net::TcpListener, task::JoinHandle};

use crate::ssm::{server::serve, state::State};

/// An in-process AWS Systems Manager Parameter Store server for tests.
///
/// `FakeSsm` listens on a random localhost port and supports `GetParameter` and
/// `GetParameters` requests from the official [`aws_sdk_ssm::Client`]. Tests can
/// register parameter names and values without contacting AWS.
///
/// For `GetParameters`, registered names are returned as parameters and missing
/// names are reported as invalid parameters. Registering an existing name again
/// replaces its value. The server runs in a background Tokio task and shuts down
/// when the `FakeSsm` value is dropped.
pub struct FakeSsm {
    port: u16,
    state: Arc<Mutex<State>>,
    handle: JoinHandle<()>,
}

impl FakeSsm {
    /// Starts a fake SSM Parameter Store server on a random available localhost
    /// port.
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
        let server_state = Arc::clone(&state);
        let handle = tokio::spawn(async move {
            serve(listener, server_state).await;
        });
        Self {
            port,
            state,
            handle,
        }
    }

    /// Returns the HTTP endpoint URL for this server.
    ///
    /// Use this URL to configure an SSM client created outside
    /// [`FakeSsm::client`].
    pub fn endpoint(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Creates an official SSM [`Client`] configured to use this fake server.
    ///
    /// The client uses the `us-east-1` region, test credentials, an empty test
    /// environment, and [`FakeSsm::endpoint`] as its endpoint. Creating it does
    /// not require real AWS credentials.
    pub async fn client(&self) -> Client {
        let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .empty_test_environment()
            .region(aws_config::Region::new("us-east-1"))
            .test_credentials()
            .endpoint_url(self.endpoint())
            .load()
            .await;
        aws_sdk_ssm::Client::new(&sdk_config)
    }

    /// Registers one parameter with the given `name` and `value`.
    ///
    /// If `name` is already registered, its value is replaced.
    ///
    /// # Panics
    ///
    /// Panics if the fake's internal state mutex is poisoned.
    pub fn set_param_value(&self, name: &str, value: &str) {
        let pair = (name.to_string(), value.to_string());
        self.state.lock().unwrap().add_parameters(&[pair]);
    }

    /// Registers multiple parameter name-value pairs.
    ///
    /// Values for names that are already registered are replaced. Parameters
    /// not included in `values` remain unchanged.
    ///
    /// # Panics
    ///
    /// Panics if the fake's internal state mutex is poisoned.
    pub fn set_param_values(&self, values: &[(String, String)]) {
        self.state.lock().unwrap().add_parameters(values);
    }
}

impl Drop for FakeSsm {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
