use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use clap::Parser;
use seedrelay::cli::Cli;

#[test]
fn parses_default_server_cli() {
    let cli = Cli::parse_from(["seedrelay"]);

    assert_eq!(
        cli.bind,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8000)
    );
    assert!(!cli.reset_credentials);
    assert!(!cli.web);
}

#[test]
fn parses_web_server_flag() {
    let cli = Cli::parse_from(["seedrelay", "--web"]);

    assert!(cli.web);
}

#[test]
fn parses_env_path_flag() {
    let cli = Cli::parse_from(["seedrelay", "--env-path", "/tmp/seedrelay.env"]);

    assert_eq!(cli.env_path, Some(PathBuf::from("/tmp/seedrelay.env")));
}
