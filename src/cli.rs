use clap::Parser;

#[derive(Debug, Parser)]
#[command(
    name = "seedrelay",
    about = "SeedRelay: local OpenAI-style Realtime transcription bridge for Seed-ASR"
)]
pub struct Cli {
    #[arg(long, default_value = "0.0.0.0")]
    pub host: String,

    #[arg(long, default_value_t = 8000)]
    pub port: u16,

    #[arg(long, default_value = "seed-asr")]
    pub model: String,

    #[arg(long)]
    pub api_key: Option<String>,

    #[arg(long)]
    pub webui: bool,

    #[arg(long)]
    pub reset: bool,
}
