use std::sync::{Arc, Mutex};

use serde_json::Value as JsonValue;
use tokio::{net::TcpListener, task::JoinHandle};
use vaultrs::{
    api::kv2::responses::ReadSecretMetadataResponse,
    client::{VaultClient, VaultClientSettingsBuilder},
};

use crate::vault::{server::serve, state::State};

const TOKEN: &str = "test-token";

pub struct FakeVault {
    port: u16,
    mount: String,
    state: Arc<Mutex<State>>,
    handle: JoinHandle<()>,
}

impl FakeVault {
    pub async fn start(mount: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let state = Arc::new(Mutex::new(State::new(mount)));
        let server_state = Arc::clone(&state);
        let handle = tokio::spawn(async move {
            serve(listener, server_state).await;
        });
        Self {
            port,
            mount: mount.to_string(),
            state,
            handle,
        }
    }

    pub fn endpoint(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn client(&self) -> VaultClient {
        VaultClient::new(
            VaultClientSettingsBuilder::default()
                .address(self.endpoint())
                .token(TOKEN)
                .build()
                .unwrap(),
        )
        .unwrap()
    }

    pub fn set_list_response(&self, path: &str, data: Vec<String>) {
        let path = format!("/v1/{}/metadata/{path}", self.mount);
        self.state.lock().unwrap().set_list_response(&path, data);
    }

    pub fn set_metadata_response(&self, path: &str, data: ReadSecretMetadataResponse) {
        let path = format!("/v1/{}/metadata/{path}", self.mount);
        self.state
            .lock()
            .unwrap()
            .set_metadata_response(&path, data);
    }

    pub fn set_read_response(&self, path: &str, data: JsonValue) {
        let path = format!("/v1/{}/data/{path}", self.mount);
        self.state.lock().unwrap().set_read_response(&path, data);
    }
}

impl Drop for FakeVault {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
