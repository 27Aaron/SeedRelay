use seedrelay::credentials::{is_jwt_expired, CachedCredentials};

#[test]
fn default_credentials_path_uses_seedrelay_config_dir() {
    let path = seedrelay::credentials::default_credentials_path();

    assert!(path.to_string_lossy().contains(".config/seedrelay"));
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
fn malformed_jwt_is_treated_as_not_expired() {
    assert!(!is_jwt_expired("not-a-jwt", 60));
}
