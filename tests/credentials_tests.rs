use seedrelay::credentials::{default_credentials_path, is_jwt_expired, CachedCredentials};

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

#[test]
fn loading_invalid_json_fails() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("credentials.json");
    std::fs::write(&path, "not json").expect("write invalid");

    let error = CachedCredentials::load(&path).expect_err("invalid json should fail");

    assert!(error.to_string().contains("invalid credentials file"));
}

#[test]
fn malformed_jwt_is_treated_as_not_expired() {
    assert!(!is_jwt_expired("not-a-jwt", 60));
}
