use std::sync::{Arc, Mutex};

use aws_sdk_s3::Client;
use tokio::{net::TcpListener, task::JoinHandle};

use crate::s3::{server::serve, state::State};

pub struct FakeS3 {
    port: u16,
    state: Arc<Mutex<State>>,
    handle: JoinHandle<()>,
}

impl FakeS3 {
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
        aws_sdk_s3::Client::new(&sdk_config)
    }

    pub fn get_object<B>(&self, bucket: &str, key: &str, bytes: B)
    where
        B: Into<hyper::body::Bytes>,
    {
        let bytes = bytes.into();
        let path = format!("/{bucket}/{key}");
        let mut state = self.state.lock().unwrap();
        state.get_objects.insert(path, bytes);
    }

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

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn test_fake_s3_1() {
        let fake = FakeS3::start().await;
        fake.get_object("my-bucket", "docs/my-file.txt", "Some texts");
        let client = fake.client().await;
        let res = client
            .get_object()
            .bucket("my-bucket")
            .key("docs/my-file.txt")
            .send()
            .await
            .unwrap();
        let bytes = res.body.collect().await.unwrap().into_bytes();
        let out = String::from_utf8_lossy(&bytes).to_string();
        assert_eq!(out, "Some texts");
    }
}
