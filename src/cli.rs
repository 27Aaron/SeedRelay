use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "seedrelay",
    about = "SeedRelay: local OpenAI-style Realtime transcription bridge for Seed-ASR 2.0"
)]
pub struct Cli {
    #[arg(long, value_name = "ADDR")]
    pub bind: Option<SocketAddr>,

    #[arg(long, value_name = "PATH")]
    pub env_path: Option<PathBuf>,

    #[arg(long, value_name = "MODEL")]
    pub model: Option<String>,

    #[arg(long, value_name = "KEY")]
    pub api_key: Option<String>,

    #[arg(long)]
    pub reset_credentials: bool,

    #[arg(long)]
    pub debug: bool,

    #[arg(long)]
    pub web: bool,
}
