use std::sync::{Arc, Mutex};

use aws_sdk_s3::Client;
use tokio::{net::TcpListener, task::JoinHandle};

use crate::s3::{server::serve, state::State};

/// An in-process Amazon S3 HTTP server for tests.
///
/// `FakeS3` listens on a random localhost port and supports a focused subset of
/// S3 object operations through the official [`aws_sdk_s3::Client`]. Tests can
/// register object bodies returned by `GetObject` requests and paths accepted
/// by `PutObject` requests without contacting AWS.
///
/// Requests for unregistered objects are rejected by the server. The server
/// runs in a background Tokio task and shuts down when the `FakeS3` value is
/// dropped.
pub struct FakeS3 {
    port: u16,
    state: Arc<Mutex<State>>,
    handle: JoinHandle<()>,
}

impl FakeS3 {
    /// Starts a fake S3 server on a random available localhost port.
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
    /// Use this URL to configure an S3 client created outside
    /// [`FakeS3::client`].
    pub fn endpoint(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Creates an official S3 [`Client`] configured to use this fake server.
    ///
    /// The client uses the `us-east-1` region, test credentials, an empty test
    /// environment, and [`FakeS3::endpoint`] as its endpoint. Creating it does
    /// not require real AWS credentials.
    pub async fn client(&self) -> Client {
        let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .empty_test_environment()
            .region(aws_config::Region::new("us-east-1"))
            .test_credentials()
            .endpoint_url(self.endpoint())
            .load()
            .await;
        aws_sdk_s3::Client::new(&sdk_config)
    }

    /// Registers the body returned by a `GetObject` request for `bucket` and
    /// `key`.
    ///
    /// Registering the same bucket and key again replaces the previous body.
    /// The response uses the `application/octet-stream` content type.
    ///
    /// # Panics
    ///
    /// Panics if the fake's internal state mutex is poisoned.
    pub fn get_object<B>(&self, bucket: &str, key: &str, bytes: B)
    where
        B: Into<hyper::body::Bytes>,
    {
        let bytes = bytes.into();
        let path = format!("/{bucket}/{key}");
        let mut state = self.state.lock().unwrap();
        state.get_objects.insert(path, bytes);
    }

    /// Registers an expected `PutObject` request for `bucket` and `key`.
    ///
    /// A matching request receives a successful response. The uploaded body is
    /// consumed but is not retained or validated.
    ///
    /// # Panics
    ///
    /// Panics if the fake's internal state mutex is poisoned.
    pub fn put_object(&self, bucket: &str, key: &str) {
        let path = format!("/{bucket}/{key}");
        let mut state = self.state.lock().unwrap();
        state.put_objects.push(path);
    }
}

impl Drop for FakeS3 {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
