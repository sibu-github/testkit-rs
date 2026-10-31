#![cfg(feature = "ssm")]

use std::collections::HashMap;

use testkit_rs::ssm::FakeSsm;

#[tokio::test]
async fn get_parameter_returns_expected_value() {
    let fake = FakeSsm::start().await;
    fake.set_param_value("DB_PASSWORD", "Pass123");

    let output = fake
        .client()
        .await
        .get_parameter()
        .name("DB_PASSWORD")
        .with_decryption(true)
        .send()
        .await
        .unwrap();

    let parameter = output.parameter().unwrap();
    assert_eq!(parameter.name(), Some("DB_PASSWORD"));
    assert_eq!(parameter.value(), Some("Pass123"));
    assert_eq!(parameter.version(), 1);
}

#[tokio::test]
async fn get_parameters_returns_valid_values_and_invalid_names() {
    let fake = FakeSsm::start().await;
    fake.set_param_values(&[
        ("DB_USERNAME".to_string(), "test-user".to_string()),
        ("DB_PASSWORD".to_string(), "Pass123".to_string()),
        ("UNREQUESTED".to_string(), "ignored".to_string()),
    ]);

    let output = fake
        .client()
        .await
        .get_parameters()
        .set_names(Some(vec![
            "DB_USERNAME".to_string(),
            "DB_PASSWORD".to_string(),
            "MISSING".to_string(),
        ]))
        .with_decryption(true)
        .send()
        .await
        .unwrap();

    let parameters = output
        .parameters()
        .iter()
        .map(|parameter| {
            (
                parameter.name().unwrap().to_string(),
                parameter.value().unwrap().to_string(),
            )
        })
        .collect::<HashMap<_, _>>();

    assert_eq!(parameters.len(), 2);
    assert_eq!(
        parameters.get("DB_USERNAME"),
        Some(&"test-user".to_string())
    );
    assert_eq!(parameters.get("DB_PASSWORD"), Some(&"Pass123".to_string()));
    assert_eq!(output.invalid_parameters(), &["MISSING"]);
}

#[tokio::test]
async fn setting_parameter_again_replaces_its_value() {
    let fake = FakeSsm::start().await;
    fake.set_param_value("API_KEY", "old-value");
    fake.set_param_value("API_KEY", "new-value");

    let output = fake
        .client()
        .await
        .get_parameter()
        .name("API_KEY")
        .send()
        .await
        .unwrap();

    assert_eq!(output.parameter().unwrap().value(), Some("new-value"));
}
