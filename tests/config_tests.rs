use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use seedrelay::config::{
    resolve_bind, resolve_server_config, DEFAULT_HOST, DEFAULT_MODEL, DEFAULT_PORT,
};

#[test]
fn resolve_bind_uses_default_host_and_port() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(".env");
    std::fs::write(&path, "").expect("write env");

    let bind = resolve_bind(None, &path).expect("resolve bind");

    assert_eq!(
        bind,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), DEFAULT_PORT)
    );
    assert_eq!(DEFAULT_HOST, "127.0.0.1");
}

#[test]
fn resolve_bind_reads_host_and_port_from_env() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(".env");
    std::fs::write(&path, "host=0.0.0.0\nport=9000\n").expect("write env");

    let bind = resolve_bind(None, &path).expect("resolve bind");

    assert_eq!(bind.to_string(), "0.0.0.0:9000");
}

#[test]
fn resolve_bind_prefers_cli_bind_over_env() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(".env");
    std::fs::write(&path, "host=0.0.0.0\nport=9000\n").expect("write env");
    let cli_bind = "127.0.0.1:7000".parse().expect("cli bind");

    let bind = resolve_bind(Some(cli_bind), &path).expect("resolve bind");

    assert_eq!(bind.to_string(), "127.0.0.1:7000");
}

#[test]
fn resolve_bind_rejects_invalid_port() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(".env");
    std::fs::write(&path, "host=127.0.0.1\nport=nope\n").expect("write env");

    let error = resolve_bind(None, &path).expect_err("invalid port");

    assert!(error.to_string().contains("invalid port"));
}

#[test]
fn resolve_server_config_defaults_to_seed_asr_without_api_key() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(".env");
    std::fs::write(&path, "").expect("write env");

    let config = resolve_server_config(None, None, None, &path).expect("resolve config");

    assert_eq!(config.bind.to_string(), "127.0.0.1:8000");
    assert_eq!(config.model, DEFAULT_MODEL);
    assert_eq!(DEFAULT_MODEL, "seed-asr");
    assert_eq!(config.api_key, None);
}

#[test]
fn resolve_server_config_reads_model_and_api_key_from_env() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(".env");
    std::fs::write(&path, "model=custom-asr\napi_key=local-secret\n").expect("write env");

    let config = resolve_server_config(None, None, None, &path).expect("resolve config");

    assert_eq!(config.model, "custom-asr");
    assert_eq!(config.api_key.as_deref(), Some("local-secret"));
}

#[test]
fn resolve_server_config_prefers_cli_model_and_api_key() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(".env");
    std::fs::write(&path, "model=env-asr\napi_key=env-secret\n").expect("write env");

    let config = resolve_server_config(
        None,
        Some("cli-asr".into()),
        Some("cli-secret".into()),
        &path,
    )
    .expect("resolve config");

    assert_eq!(config.model, "cli-asr");
    assert_eq!(config.api_key.as_deref(), Some("cli-secret"));
}

#[test]
fn resolve_server_config_treats_empty_api_key_as_disabled() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(".env");
    std::fs::write(&path, "api_key=   \n").expect("write env");

    let config = resolve_server_config(None, None, None, &path).expect("resolve config");

    assert_eq!(config.api_key, None);
}
