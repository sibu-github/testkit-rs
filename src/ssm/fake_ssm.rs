use std::sync::{Arc, Mutex};

use aws_sdk_ssm::Client;
use tokio::{net::TcpListener, task::JoinHandle};

use crate::ssm::{server::serve, state::State};

pub struct FakeSsm {
    port: u16,
    state: Arc<Mutex<State>>,
    handle: JoinHandle<()>,
}

impl FakeSsm {
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

    pub fn endpoint(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

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

    pub fn set_param_value(&self, name: &str, value: &str) {
        let pair = (name.to_string(), value.to_string());
        self.state.lock().unwrap().add_parameters(&[pair]);
    }

    pub fn set_param_values(&self, values: &[(String, String)]) {
        self.state.lock().unwrap().add_parameters(values);
    }
}

impl Drop for FakeSsm {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
