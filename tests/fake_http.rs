#![cfg(feature = "http")]

use reqwest::Client;
use serde::{Deserialize, Serialize};
use testkit_rs::http::FakeHttp;

const USER_NAME: &str = "testuser";
const PASSWORD: &str = "testpass";
const BASE_URL: &str = "https://api.example.com/api";

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
struct User {
    id: u64,
    name: String,
    status: String,
}

async fn fetch_user(client: &Client, id: u64) -> anyhow::Result<User> {
    let url = format!("{BASE_URL}/users");
    let user = client
        .get(url)
        .query(&[("id", id)])
        .basic_auth(USER_NAME, Some(PASSWORD))
        .send()
        .await?
        .json::<User>()
        .await?;
    Ok(user)
}

#[tokio::test]
async fn test_get_return_expected_response() {
    let user = User {
        id: 1,
        name: "siba".to_string(),
        status: "active".to_string(),
    };
    let mut fake = FakeHttp::start_tls().await;
    fake.intercept("api.example.com")
        .expect_get("/api/users")
        .query("id", 1)
        .respond_ok_json(user.clone());
    let client = fake.client();
    let res = fetch_user(&client, 1).await.unwrap();
    assert_eq!(res, user);
}
