#![cfg(feature = "vault")]

use std::collections::HashMap;

use serde_json::json;
use testkit_rs::vault::FakeVault;
use vaultrs::{
    api::kv2::responses::{ReadSecretMetadataResponse, SecretMetadata},
    error::ClientError,
    kv2,
};

#[tokio::test]
async fn read_for_unregistered_path_returns_404() {
    let mount = "dev";
    let fake = FakeVault::start(mount).await;

    let err = kv2::read::<HashMap<String, String>>(&fake.client(), mount, "global/missing")
        .await
        .unwrap_err();

    assert!(
        matches!(err, ClientError::APIError { code: 404, .. }),
        "{err:?}"
    );
}

#[tokio::test]
async fn list_for_unregistered_path_returns_404() {
    let mount = "dev";
    let fake = FakeVault::start(mount).await;

    let err = kv2::list(&fake.client(), mount, "global")
        .await
        .unwrap_err();

    assert!(
        matches!(err, ClientError::APIError { code: 404, .. }),
        "{err:?}"
    );
}

#[tokio::test]
async fn read_from_another_mount_names_the_configured_mount() {
    let fake = FakeVault::start("dev").await;

    let err = kv2::read::<HashMap<String, String>>(&fake.client(), "prod", "global/key1")
        .await
        .unwrap_err();

    let ClientError::APIError { code, errors } = err else {
        panic!("expected an API error, got {err:?}");
    };
    assert_eq!(code, 404);
    assert!(errors[0].contains("mount `dev`"), "{errors:?}");
}

#[tokio::test]
async fn server_keeps_serving_after_a_missing_secret() {
    let mount = "dev";
    let fake = FakeVault::start(mount).await;
    fake.set_read_response("global/key1", json!({ "username": "test-user" }));
    let client = fake.client();

    kv2::read::<HashMap<String, String>>(&client, mount, "global/missing")
        .await
        .unwrap_err();

    let values = kv2::read::<HashMap<String, String>>(&client, mount, "global/key1")
        .await
        .unwrap();
    assert_eq!(values["username"], "test-user");
}

#[tokio::test]
async fn read_returns_expected_secret() {
    let mount = "dev";
    let path = "global/key1";
    let expected = HashMap::from([
        ("username".to_string(), "test-user".to_string()),
        ("password".to_string(), "test-password".to_string()),
    ]);
    let fake = FakeVault::start(mount).await;
    fake.set_read_response(path, json!(expected));

    let values = kv2::read::<HashMap<String, String>>(&fake.client(), mount, path)
        .await
        .unwrap();

    assert_eq!(values, expected);
}

#[tokio::test]
async fn list_returns_expected_keys() {
    let mount = "dev";
    let path = "global";
    let expected = vec!["key1".to_string(), "nested/".to_string()];
    let fake = FakeVault::start(mount).await;
    fake.set_list_response(path, expected.clone());

    let keys = kv2::list(&fake.client(), mount, path).await.unwrap();

    assert_eq!(keys, expected);
}

#[tokio::test]
async fn read_metadata_returns_expected_metadata() {
    let mount = "dev";
    let path = "global/key1";
    let version = SecretMetadata {
        created_time: "2026-10-18T00:00:00Z".to_string(),
        deletion_time: String::new(),
        destroyed: false,
    };
    let expected = ReadSecretMetadataResponse {
        cas_required: true,
        created_time: "2026-10-18T00:00:00Z".to_string(),
        current_version: 1,
        delete_version_after: "0s".to_string(),
        max_versions: 10,
        oldest_version: 1,
        updated_time: "2026-10-18T00:00:00Z".to_string(),
        custom_metadata: Some(HashMap::from([(
            "environment".to_string(),
            "test".to_string(),
        )])),
        versions: HashMap::from([("1".to_string(), version)]),
    };
    let fake = FakeVault::start(mount).await;
    fake.set_metadata_response(path, expected);

    let metadata = kv2::read_metadata(&fake.client(), mount, path)
        .await
        .unwrap();

    assert!(metadata.cas_required);
    assert_eq!(metadata.current_version, 1);
    assert_eq!(metadata.max_versions, 10);
    assert_eq!(metadata.oldest_version, 1);
    assert_eq!(
        metadata.custom_metadata.unwrap().get("environment"),
        Some(&"test".to_string())
    );
    let version = metadata.versions.get("1").unwrap();
    assert_eq!(version.created_time, "2026-10-18T00:00:00Z");
    assert!(!version.destroyed);
}
