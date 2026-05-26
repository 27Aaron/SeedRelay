use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "seedrelay",
    about = "SeedRelay: local OpenAI-style Realtime transcription bridge backed by Doubao Frontier ASR"
)]
pub struct Cli {
    #[arg(long, default_value = "127.0.0.1:8000", value_name = "ADDR")]
    pub bind: SocketAddr,

    #[arg(long, value_name = "PATH")]
    pub env_path: Option<PathBuf>,

    #[arg(long)]
    pub reset_credentials: bool,

    #[arg(long)]
    pub web: bool,
}
