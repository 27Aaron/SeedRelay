use std::env;
use std::fs;
use std::net::{IpAddr, SocketAddr};
use std::path::Path;

use anyhow::{Context, Result};

use crate::env_file::{env_value, parse_env};

pub const DEFAULT_HOST: &str = "0.0.0.0";
pub const DEFAULT_PORT: u16 = 8000;
pub const DEFAULT_MODEL: &str = "seed-asr";

const HOST_KEY: &str = "host";
const PORT_KEY: &str = "port";
const MODEL_KEY: &str = "model";
const API_KEY_KEY: &str = "api_key";

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ServerConfig {
    pub bind: SocketAddr,
    pub model: String,
    pub api_key: Option<String>,
}

pub fn resolve_server_config(
    cli_bind: Option<SocketAddr>,
    cli_model: Option<String>,
    cli_api_key: Option<String>,
    env_path: &Path,
) -> Result<ServerConfig> {
    let env = load_env_entries(env_path)?;
    let bind = resolve_config_bind(cli_bind, &env)?;
    let model = resolve_config_value(cli_model, &env, MODEL_KEY).unwrap_or(DEFAULT_MODEL.into());
    let api_key = resolve_config_value(cli_api_key, &env, API_KEY_KEY);

    Ok(ServerConfig {
        bind,
        model,
        api_key,
    })
}

fn resolve_config_bind(
    cli_bind: Option<SocketAddr>,
    env: &[(String, String)],
) -> Result<SocketAddr> {
    if let Some(bind) = cli_bind {
        return Ok(bind);
    }

    let mut host = DEFAULT_HOST.parse::<IpAddr>().expect("valid default host");
    let mut port = DEFAULT_PORT;

    if let Some(value) = resolve_config_value(None, env, HOST_KEY) {
        host = value
            .parse()
            .with_context(|| format!("invalid host `{value}`"))?;
    }
    if let Some(value) = resolve_config_value(None, env, PORT_KEY) {
        port = value
            .parse()
            .with_context(|| format!("invalid port `{value}`"))?;
    }

    Ok(SocketAddr::new(host, port))
}

fn load_env_entries(env_path: &Path) -> Result<Vec<(String, String)>> {
    if env_path.exists() {
        let data = fs::read_to_string(env_path)
            .with_context(|| format!("failed to read env file {}", env_path.display()))?;
        Ok(parse_env(&data))
    } else {
        Ok(Vec::new())
    }
}

fn resolve_config_value(
    cli_value: Option<String>,
    env: &[(String, String)],
    env_key: &str,
) -> Option<String> {
    cli_value
        .or_else(|| env::var(env_key).ok())
        .or_else(|| env_value(env, env_key))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
