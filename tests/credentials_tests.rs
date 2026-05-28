use std::time::Duration;

use seedrelay::credentials::{
    default_credentials_path, ensure_credentials, is_jwt_expired, CachedCredentials,
};

#[test]
fn default_credentials_path_points_to_seedrelay_dir() {
    let path = default_credentials_path();

    assert!(path.to_string_lossy().contains("seedrelay"));
    assert_eq!(
        path.file_name().and_then(|v| v.to_str()),
        Some("credentials.json")
    );
}

#[test]
fn credentials_round_trip_through_json_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("credentials.json");
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
fn saving_credentials_creates_parent_directory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("nested").join("credentials.json");
    let credentials = CachedCredentials {
        device_id: "device-2".to_string(),
        install_id: "install-2".to_string(),
        cdid: "cdid-2".to_string(),
        openudid: "openudid-2".to_string(),
        clientudid: "clientudid-2".to_string(),
        token: "token-2".to_string(),
    };

    credentials.save(&path).expect("save creates parent dir");
    let loaded = CachedCredentials::load(&path).expect("load credentials");

    assert_eq!(loaded.device_id, "device-2");
}

#[cfg(unix)]
#[test]
fn saving_credentials_writes_private_file_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("credentials.json");
    let credentials = CachedCredentials {
        device_id: "device-private".to_string(),
        install_id: "install-private".to_string(),
        cdid: "cdid-private".to_string(),
        openudid: "openudid-private".to_string(),
        clientudid: "clientudid-private".to_string(),
        token: "token-private".to_string(),
    };

    credentials.save(&path).expect("save credentials");

    let mode = std::fs::metadata(&path)
        .expect("credentials metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn loading_invalid_json_fails() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("credentials.json");
    std::fs::write(&path, "not json").expect("write invalid");

    let error = CachedCredentials::load(&path).expect_err("invalid json should fail");

    assert!(error.to_string().contains("invalid credentials file"));
}

#[test]
fn malformed_jwt_is_treated_as_expired() {
    assert!(is_jwt_expired("not-a-jwt", 60));
}

#[tokio::test]
async fn ensure_credentials_returns_invalid_cache_errors() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("credentials.json");
    std::fs::write(&path, "not json").expect("write invalid");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(1))
        .build()
        .expect("client");

    let error = ensure_credentials(&client, &path, false)
        .await
        .expect_err("invalid cache should fail before registration");

    assert!(error.to_string().contains("invalid credentials file"));
}
