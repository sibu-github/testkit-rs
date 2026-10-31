use std::sync::{Arc, Mutex};

use serde_json::Value as JsonValue;
use tokio::{net::TcpListener, task::JoinHandle};
use vaultrs::{
    api::kv2::responses::ReadSecretMetadataResponse,
    client::{VaultClient, VaultClientSettingsBuilder},
};

use crate::vault::{server::serve, state::State};

const TOKEN: &str = "test-token";

/// An in-process HashiCorp Vault KV v2 server for tests.
///
/// `FakeVault` listens on a random localhost port and supports reading secrets,
/// listing secret paths, and reading secret metadata through the `vaultrs` KV v2
/// API. Tests register responses for paths under the mount passed to
/// [`FakeVault::start`] without running or contacting a Vault server.
///
/// Requests for unregistered paths are rejected by the server. Registering a
/// response for the same path again replaces the previous response. The server
/// runs in a background Tokio task and shuts down when the `FakeVault` value is
/// dropped.
pub struct FakeVault {
    port: u16,
    mount: String,
    state: Arc<Mutex<State>>,
    handle: JoinHandle<()>,
}

impl FakeVault {
    /// Starts a fake Vault server for the KV v2 `mount` on a random available
    /// localhost port.
    ///
    /// The server runs in a background Tokio task until this value is dropped.
    /// Paths passed to the response registration methods are relative to
    /// `mount`.
    ///
    /// # Panics
    ///
    /// Panics if the server cannot bind to a local port or inspect its bound
    /// address.
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

    /// Returns the HTTP endpoint URL for this server.
    ///
    /// Use this URL to configure a Vault client created outside
    /// [`FakeVault::client`].
    pub fn endpoint(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Creates a [`VaultClient`] configured to use this fake server.
    ///
    /// The client connects over HTTP and uses the fixed test token
    /// `test-token`. The fake does not require real Vault credentials.
    ///
    /// # Panics
    ///
    /// Panics if the client settings are invalid or the client cannot be
    /// constructed.
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

    /// Registers the keys returned when listing `path` in the configured KV v2
    /// mount.
    ///
    /// Registering the same path again replaces its previous response.
    ///
    /// # Panics
    ///
    /// Panics if the fake's internal state mutex is poisoned.
    pub fn set_list_response(&self, path: &str, data: Vec<String>) {
        let path = format!("/v1/{}/metadata/{path}", self.mount);
        self.state.lock().unwrap().set_list_response(&path, data);
    }

    /// Registers the metadata returned when reading metadata for `path` in the
    /// configured KV v2 mount.
    ///
    /// Registering the same path again replaces its previous response.
    ///
    /// # Panics
    ///
    /// Panics if the fake's internal state mutex is poisoned.
    pub fn set_metadata_response(&self, path: &str, data: ReadSecretMetadataResponse) {
        let path = format!("/v1/{}/metadata/{path}", self.mount);
        self.state
            .lock()
            .unwrap()
            .set_metadata_response(&path, data);
    }

    /// Registers the secret data returned when reading `path` from the
    /// configured KV v2 mount.
    ///
    /// The JSON value is wrapped in a version-one Vault secret response with
    /// synthetic metadata. Registering the same path again replaces its
    /// previous response.
    ///
    /// # Panics
    ///
    /// Panics if the fake's internal state mutex is poisoned.
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
