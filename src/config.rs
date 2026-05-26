use std::fs;
use std::net::{IpAddr, SocketAddr};
use std::path::Path;

use anyhow::{Context, Result};

use crate::env_file::{env_value, parse_env};

pub const DEFAULT_HOST: &str = "127.0.0.1";
pub const DEFAULT_PORT: u16 = 8000;

const HOST_KEY: &str = "host";
const PORT_KEY: &str = "port";

pub fn resolve_bind(cli_bind: Option<SocketAddr>, env_path: &Path) -> Result<SocketAddr> {
    if let Some(bind) = cli_bind {
        return Ok(bind);
    }

    let (host, port) = load_server_config(env_path)?;
    Ok(SocketAddr::new(host, port))
}

fn load_server_config(env_path: &Path) -> Result<(IpAddr, u16)> {
    let mut host = DEFAULT_HOST.parse::<IpAddr>().expect("valid default host");
    let mut port = DEFAULT_PORT;

    if env_path.exists() {
        let data = fs::read_to_string(env_path)
            .with_context(|| format!("failed to read env file {}", env_path.display()))?;
        let env = parse_env(&data);

        if let Some(value) = env_value(&env, HOST_KEY).filter(|value| !value.is_empty()) {
            host = value
                .parse()
                .with_context(|| format!("invalid host `{value}` in {}", env_path.display()))?;
        }
        if let Some(value) = env_value(&env, PORT_KEY).filter(|value| !value.is_empty()) {
            port = value
                .parse()
                .with_context(|| format!("invalid port `{value}` in {}", env_path.display()))?;
        }
    }

    Ok((host, port))
}
