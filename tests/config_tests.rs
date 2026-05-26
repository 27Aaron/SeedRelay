use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use seedrelay::config::{resolve_bind, DEFAULT_HOST, DEFAULT_PORT};

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
