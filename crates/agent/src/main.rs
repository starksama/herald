use clap::Parser;
use tracing_subscriber::EnvFilter;

mod config;
mod forward;
mod tunnel;

use config::AgentConfig;

#[derive(Debug, Parser)]
#[command(name = "herald-agent")]
#[command(about = "Herald Agent tunnel client", version)]
struct Args {
    #[arg(long)]
    token: String,
    #[arg(long)]
    forward: String,
    #[arg(long, default_value = "wss://api.herald.dev/v1/tunnel")]
    herald_url: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .json()
        .init();

    let args = Args::parse();
    let config = AgentConfig {
        token: args.token,
        forward_url: args.forward,
        herald_url: args.herald_url,
    };

    tunnel::run_tunnel(config).await
}
