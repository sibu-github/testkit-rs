#![cfg(feature = "http")]

use reqwest::{Client, StatusCode};
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

#[tokio::test]
async fn unmatched_request_returns_501_with_reason() {
    let fake = FakeHttp::start().await;

    let res = fake
        .client()
        .get(format!("{}/missing", fake.uri()))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::NOT_IMPLEMENTED);
    let body = res.text().await.unwrap();
    assert!(
        body.contains("no expectation registered for GET /missing"),
        "{body}"
    );
}

#[tokio::test]
async fn mismatched_request_returns_500_with_reason() {
    let fake = FakeHttp::start().await;
    fake.expect_get("/users").query("id", 1).respond_ok("ok");

    let res = fake
        .client()
        .get(format!("{}/users", fake.uri()))
        .query(&[("id", 2)])
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = res.text().await.unwrap();
    assert!(
        body.contains("Query parameter `id` mismatch: expected `1`, got `2`"),
        "{body}"
    );
}

#[tokio::test]
async fn server_keeps_serving_after_an_unmatched_request() {
    let fake = FakeHttp::start().await;
    fake.expect_get("/health").respond_ok("ok");
    let client = fake.client();

    let unmatched = client
        .get(format!("{}/missing", fake.uri()))
        .send()
        .await
        .unwrap();
    assert_eq!(unmatched.status(), StatusCode::NOT_IMPLEMENTED);

    let res = client
        .get(format!("{}/health", fake.uri()))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.text().await.unwrap(), "ok");
}
