use std::path::PathBuf;

use clap::Parser;
use seedrelay::cli::Cli;

#[test]
fn parses_default_server_cli() {
    let cli = Cli::parse_from(["seedrelay"]);

    assert!(cli.bind.is_none());
    assert!(cli.model.is_none());
    assert!(cli.api_key.is_none());
    assert!(!cli.debug);
    assert!(!cli.reset_credentials);
    assert!(!cli.web);
}

#[test]
fn parses_bind_override_flag() {
    let cli = Cli::parse_from(["seedrelay", "--bind", "0.0.0.0:9000"]);

    assert_eq!(cli.bind.unwrap().to_string(), "0.0.0.0:9000");
}

#[test]
fn parses_web_server_flag() {
    let cli = Cli::parse_from(["seedrelay", "--web", "--debug"]);

    assert!(cli.web);
    assert!(cli.debug);
}

#[test]
fn parses_env_path_flag() {
    let cli = Cli::parse_from(["seedrelay", "--env-path", "/tmp/seedrelay.env"]);

    assert_eq!(cli.env_path, Some(PathBuf::from("/tmp/seedrelay.env")));
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

    assert_eq!(cli.model.as_deref(), Some("custom-asr"));
    assert_eq!(cli.api_key.as_deref(), Some("local-secret"));
}
