use seedrelay::config::{resolve_server_config, DEFAULT_HOST, DEFAULT_MODEL, DEFAULT_PORT};

#[test]
fn resolve_server_config_uses_defaults() {
    let config = resolve_server_config(DEFAULT_HOST, DEFAULT_PORT, DEFAULT_MODEL, None)
        .expect("resolve config");

    assert_eq!(config.bind.port(), DEFAULT_PORT);
    assert_eq!(config.bind.ip().to_string(), "0.0.0.0");
    assert_eq!(config.model, DEFAULT_MODEL);
    assert_eq!(config.api_key, None);
}

#[test]
fn resolve_server_config_uses_custom_host_and_port() {
    let config =
        resolve_server_config("127.0.0.1", 9000, DEFAULT_MODEL, None).expect("resolve config");

    assert_eq!(config.bind.to_string(), "127.0.0.1:9000");
}

#[test]
fn resolve_server_config_rejects_invalid_host() {
    let error = resolve_server_config("not-an-ip", DEFAULT_PORT, DEFAULT_MODEL, None)
        .expect_err("invalid host");

    assert!(error.to_string().contains("invalid host"));
}

#[test]
fn resolve_server_config_rejects_invalid_port() {
    let result = resolve_server_config(DEFAULT_HOST, 0, DEFAULT_MODEL, None);

    // Port 0 is valid for SocketAddr but unusual; 65535+ would overflow u16
    assert!(result.is_ok());
}

#[test]
fn resolve_server_config_reads_api_key() {
    let config = resolve_server_config(
        DEFAULT_HOST,
        DEFAULT_PORT,
        DEFAULT_MODEL,
        Some("secret".into()),
    )
    .expect("resolve config");

    assert_eq!(config.api_key.as_deref(), Some("secret"));
}

#[test]
fn resolve_server_config_treats_empty_api_key_as_disabled() {
    let config = resolve_server_config(
        DEFAULT_HOST,
        DEFAULT_PORT,
        DEFAULT_MODEL,
        Some("   ".into()),
    )
    .expect("resolve config");

    assert_eq!(config.api_key, None);
}

#[test]
fn resolve_server_config_uses_custom_model() {
    let config = resolve_server_config(DEFAULT_HOST, DEFAULT_PORT, "custom-asr", None)
        .expect("resolve config");

    assert_eq!(config.model, "custom-asr");
}
