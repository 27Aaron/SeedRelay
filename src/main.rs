use anyhow::Result;
use clap::Parser;
use seedrelay::cli::Cli;
use seedrelay::config::resolve_server_config;
use seedrelay::credentials::default_credentials_path;
use seedrelay::server::serve_realtime;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = resolve_server_config(&cli.host, cli.port, &cli.model, cli.api_key)?;
    let credentials_path = default_credentials_path();

    serve_realtime(config, &credentials_path, cli.reset, cli.webui).await
}
