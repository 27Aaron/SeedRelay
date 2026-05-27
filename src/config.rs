use std::net::{IpAddr, SocketAddr};

use anyhow::{Context, Result};

pub const DEFAULT_HOST: &str = "0.0.0.0";
pub const DEFAULT_PORT: u16 = 8000;
pub const DEFAULT_MODEL: &str = "seed-asr";

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ServerConfig {
    pub bind: SocketAddr,
    pub model: String,
    pub api_key: Option<String>,
}

pub fn resolve_server_config(
    host: &str,
    port: u16,
    model: &str,
    api_key: Option<String>,
) -> Result<ServerConfig> {
    let host: IpAddr = host
        .parse()
        .with_context(|| format!("invalid host `{host}`"))?;
    let bind = SocketAddr::new(host, port);

    let api_key = api_key
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());

    Ok(ServerConfig {
        bind,
        model: model.to_string(),
        api_key,
    })
}
