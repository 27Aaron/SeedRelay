use std::env;

use anyhow::Result;
use clap::Parser;
use seedrelay::cli::Cli;
use seedrelay::config::resolve_server_config;
use seedrelay::credentials::default_env_path;
use seedrelay::env_file::{env_value, parse_env};
use seedrelay::server::serve_realtime;

fn env_flag(key: &str, env: &[(String, String)]) -> bool {
    let val = env::var(key)
        .or_else(|_| env::var(key.to_lowercase()))
        .ok()
        .or_else(|| env_value(env, key).or_else(|| env_value(env, &key.to_lowercase())));
    val.map(|v| matches!(v.to_lowercase().as_str(), "true" | "1" | "yes"))
        .unwrap_or(false)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let env_path = cli.env_path.unwrap_or_else(default_env_path);

    // Load .env file values into process environment (non-destructive: env vars take priority).
    let env_entries = if env_path.exists() {
        let data = std::fs::read_to_string(&env_path).unwrap_or_default();
        let entries = parse_env(&data);
        for (key, value) in &entries {
            if env::var(key).is_err() {
                env::set_var(key, value);
            }
        }
        entries
    } else {
        Vec::new()
    };

    let config = resolve_server_config(cli.bind, cli.model, cli.api_key, &env_path)?;
    let web = cli.web || env_flag("WEB", &env_entries);
    let debug = cli.debug || env_flag("DEBUG", &env_entries);

    serve_realtime(config, env_path, cli.reset_credentials, web, debug).await
}
