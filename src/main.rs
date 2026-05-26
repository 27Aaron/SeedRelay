use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use seedrelay::credentials::default_credentials_path;
use seedrelay::server::serve_realtime;

#[derive(Debug, Parser)]
#[command(
    name = "seedrelay",
    about = "SeedRelay: local OpenAI-style Realtime transcription bridge backed by Doubao Frontier ASR"
)]
struct Cli {
    #[arg(long, default_value = "127.0.0.1:8000", value_name = "ADDR")]
    bind: SocketAddr,

    #[arg(long, value_name = "PATH")]
    credentials_path: Option<PathBuf>,

    #[arg(long)]
    reset_credentials: bool,

    #[arg(long)]
    web: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let credentials_path = cli
        .credentials_path
        .unwrap_or_else(default_credentials_path);

    serve_realtime(cli.bind, credentials_path, cli.reset_credentials, cli.web).await
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use super::Cli;
    use clap::Parser;

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
}
