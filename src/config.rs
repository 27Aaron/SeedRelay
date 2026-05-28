use std::net::{IpAddr, SocketAddr};

use anyhow::{anyhow, Context, Result};

pub const DEFAULT_HOST: &str = "127.0.0.1";
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
    let api_key = api_key
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    if !host.is_loopback() && api_key.is_none() {
        return Err(anyhow!(
            "public bind requires --api-key; use 127.0.0.1 for unauthenticated local access"
        ));
    }

    let bind = SocketAddr::new(host, port);

    Ok(ServerConfig {
        bind,
        model: model.to_string(),
        api_key,
    })
}
