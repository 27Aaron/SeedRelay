use clap::Parser;
use seedrelay::cli::Cli;
use seedrelay::config::{DEFAULT_HOST, DEFAULT_MODEL, DEFAULT_PORT};

#[test]
fn parses_default_server_cli() {
    let cli = Cli::parse_from(["seedrelay"]);

    assert_eq!(cli.host, DEFAULT_HOST);
    assert_eq!(cli.port, DEFAULT_PORT);
    assert_eq!(cli.model, DEFAULT_MODEL);
    assert!(cli.api_key.is_none());
    assert!(!cli.webui);
}

#[test]
fn parses_host_and_port_flags() {
    let cli = Cli::parse_from(["seedrelay", "--host", "127.0.0.1", "--port", "9000"]);

    assert_eq!(cli.host, "127.0.0.1");
    assert_eq!(cli.port, 9000);
}

#[test]
fn parses_webui_flag() {
    let cli = Cli::parse_from(["seedrelay", "--webui"]);

    assert!(cli.webui);
}

#[test]
fn parses_model_and_api_key_flags() {
    let cli = Cli::parse_from([
        "seedrelay",
        "--model",
        "custom-asr",
        "--api-key",
        "local-secret",
    ]);

    assert_eq!(cli.model, "custom-asr");
    assert_eq!(cli.api_key.as_deref(), Some("local-secret"));
}
