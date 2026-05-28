use std::path::PathBuf;

use clap::Parser;

use crate::config::{DEFAULT_HOST, DEFAULT_MODEL, DEFAULT_PORT};

#[derive(Debug, Parser)]
#[command(
    name = "seedrelay",
    about = "SeedRelay: local OpenAI-style Realtime transcription bridge for Seed-ASR"
)]
pub struct Cli {
    #[arg(long, default_value = DEFAULT_HOST)]
    pub host: String,

    #[arg(long, default_value_t = DEFAULT_PORT)]
    pub port: u16,

    #[arg(long, default_value = DEFAULT_MODEL)]
    pub model: String,

    #[arg(long)]
    pub api_key: Option<String>,

    #[arg(long, default_value = ".seedrelay/credentials.json")]
    pub credentials_path: PathBuf,

    #[arg(long)]
    pub webui: bool,

    #[arg(long)]
    pub reset: bool,
}
