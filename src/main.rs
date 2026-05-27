use anyhow::Result;
use clap::Parser;
use seedrelay::cli::Cli;
use seedrelay::config::resolve_server_config;
use seedrelay::credentials::default_env_path;
use seedrelay::server::serve_realtime;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let env_path = cli.env_path.unwrap_or_else(default_env_path);
    let config = resolve_server_config(cli.bind, cli.model, cli.api_key, &env_path)?;

    serve_realtime(config, env_path, cli.reset_credentials, cli.web).await
}
