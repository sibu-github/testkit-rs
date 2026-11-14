#![cfg(feature = "s3")]

use aws_sdk_s3::{error::ProvideErrorMetadata, primitives::ByteStream};
use testkit_rs::s3::FakeS3;

#[tokio::test]
async fn get_object_returns_expected_bytes() {
    let bucket = "my-bucket";
    let key = "docs/my-file.bin";
    let expected = b"some object data\0with binary content";
    let fake = FakeS3::start().await;
    fake.get_object(bucket, key, expected.as_slice());

    let output = fake
        .client()
        .await
        .get_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .unwrap();
    assert_eq!(output.content_length(), Some(expected.len() as i64));
    assert_eq!(output.content_type(), Some("application/octet-stream"));

    let bytes = output.body.collect().await.unwrap().into_bytes();
    assert_eq!(bytes.as_ref(), expected);
}

#[tokio::test]
async fn put_object_succeeds_for_registered_path() {
    let bucket = "my-bucket";
    let key = "uploads/my-file.txt";
    let fake = FakeS3::start().await;
    fake.put_object(bucket, key);

    fake.client()
        .await
        .put_object()
        .bucket(bucket)
        .key(key)
        .body(ByteStream::from_static(b"uploaded content"))
        .send()
        .await
        .unwrap();
}

#[tokio::test]
async fn get_object_for_unregistered_key_returns_no_such_key() {
    let fake = FakeS3::start().await;

    let err = fake
        .client()
        .await
        .get_object()
        .bucket("my-bucket")
        .key("missing.txt")
        .send()
        .await
        .unwrap_err()
        .into_service_error();

    assert!(err.is_no_such_key(), "{err:?}");
}

#[tokio::test]
async fn keys_with_special_characters_match_registered_objects() {
    let bucket = "my-bucket";
    let key = "reports/Q1 summary+final (v2) é.txt";
    let fake = FakeS3::start().await;
    fake.get_object(bucket, key, "report");
    fake.put_object(bucket, key);
    let client = fake.client().await;

    let output = client
        .get_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .unwrap();
    let bytes = output.body.collect().await.unwrap().into_bytes();
    assert_eq!(bytes.as_ref(), b"report");

    client
        .put_object()
        .bucket(bucket)
        .key(key)
        .body(ByteStream::from_static(b"updated report"))
        .send()
        .await
        .unwrap();
}

#[tokio::test]
async fn put_object_for_unregistered_path_returns_error_with_reason() {
    let fake = FakeS3::start().await;

    let err = fake
        .client()
        .await
        .put_object()
        .bucket("my-bucket")
        .key("unexpected.txt")
        .body(ByteStream::from_static(b"content"))
        .send()
        .await
        .unwrap_err()
        .into_service_error();

    assert_eq!(err.code(), Some("UnexpectedRequest"));
    let message = err.message().unwrap();
    assert!(message.contains("/my-bucket/unexpected.txt"), "{message}");
}

#[tokio::test]
async fn server_keeps_serving_after_a_missing_object() {
    let bucket = "my-bucket";
    let fake = FakeS3::start().await;
    fake.get_object(bucket, "present.txt", "data");
    let client = fake.client().await;

    client
        .get_object()
        .bucket(bucket)
        .key("missing.txt")
        .send()
        .await
        .unwrap_err();

    let output = client
        .get_object()
        .bucket(bucket)
        .key("present.txt")
        .send()
        .await
        .unwrap();
    let bytes = output.body.collect().await.unwrap().into_bytes();
    assert_eq!(bytes.as_ref(), b"data");
}
