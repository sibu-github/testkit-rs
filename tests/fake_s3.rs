#![cfg(feature = "s3")]

use aws_sdk_s3::primitives::ByteStream;
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
