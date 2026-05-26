use anyhow::Result;
use clap::Parser;
use seedrelay::cli::Cli;
use seedrelay::config::resolve_bind;
use seedrelay::credentials::default_env_path;
use seedrelay::server::serve_realtime;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let env_path = cli.env_path.unwrap_or_else(default_env_path);
    let bind = resolve_bind(cli.bind, &env_path)?;

    serve_realtime(bind, env_path, cli.reset_credentials, cli.web).await
}
