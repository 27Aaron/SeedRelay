use std::time::Duration;

use seedrelay::credentials::{
    default_env_path, ensure_credentials, is_jwt_expired, CachedCredentials,
};

#[test]
fn default_env_path_uses_current_directory_dotenv() {
    let path = default_env_path();

    assert_eq!(
        path.file_name().and_then(|value| value.to_str()),
        Some(".env")
    );
}

#[test]
fn credentials_round_trip_through_dotenv_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(".env");
    std::fs::write(&path, "").expect("create env");
    let credentials = CachedCredentials {
        device_id: "device-1".to_string(),
        install_id: "install-1".to_string(),
        cdid: "cdid-1".to_string(),
        openudid: "openudid-1".to_string(),
        clientudid: "clientudid-1".to_string(),
        token: "token-1".to_string(),
    };

    credentials.save(&path).expect("save credentials");
    let loaded = CachedCredentials::load(&path).expect("load credentials");

    assert_eq!(loaded, credentials);
}

#[test]
fn saving_credentials_preserves_unrelated_dotenv_keys() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(".env");
    std::fs::write(&path, "APP_MODE=dev\ndevice_id=old\n").expect("write env");
    let credentials = CachedCredentials {
        device_id: "device-2".to_string(),
        install_id: "install-2".to_string(),
        cdid: "cdid-2".to_string(),
        openudid: "openudid-2".to_string(),
        clientudid: "clientudid-2".to_string(),
        token: "token-2".to_string(),
    };

    credentials.save(&path).expect("save credentials");
    let contents = std::fs::read_to_string(&path).expect("read env");

    assert!(contents.contains("APP_MODE=dev"));
    assert!(contents.contains("device_id=device-2"));
    assert!(contents.contains("install_id=install-2"));
    assert!(contents.contains("token=token-2"));
    assert!(!contents.contains("device_id=old"));
}

#[test]
fn saving_credentials_requires_existing_dotenv_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(".env");
    let credentials = CachedCredentials {
        device_id: "device-3".to_string(),
        install_id: "install-3".to_string(),
        cdid: "cdid-3".to_string(),
        openudid: "openudid-3".to_string(),
        clientudid: "clientudid-3".to_string(),
        token: "token-3".to_string(),
    };

    let error = credentials
        .save(&path)
        .expect_err("missing env should fail");
    let message = format!("{error:#}");

    assert!(message.contains("Missing .env file"));
    assert!(message.contains("cp .env.example .env"));
}

#[tokio::test]
async fn ensure_credentials_requires_existing_dotenv_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(".env");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(1))
        .build()
        .expect("client");

    let error = ensure_credentials(&client, &path, false)
        .await
        .expect_err("missing env should fail");
    let message = format!("{error:#}");

    assert!(message.contains("Missing .env file"));
    assert!(message.contains("cp .env.example .env"));
}

#[test]
fn malformed_jwt_is_treated_as_not_expired() {
    assert!(!is_jwt_expired("not-a-jwt", 60));
}
