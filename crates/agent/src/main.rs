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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_args_with_all_options() {
        let args = Args::try_parse_from([
            "herald-agent",
            "--token", "hld_sub_test123",
            "--forward", "http://localhost:8080/hooks",
            "--herald-url", "wss://custom.herald.dev/tunnel",
        ]).unwrap();

        assert_eq!(args.token, "hld_sub_test123");
        assert_eq!(args.forward, "http://localhost:8080/hooks");
        assert_eq!(args.herald_url, "wss://custom.herald.dev/tunnel");
    }

    #[test]
    fn test_args_with_default_herald_url() {
        let args = Args::try_parse_from([
            "herald-agent",
            "--token", "hld_sub_test123",
            "--forward", "http://localhost:8080/hooks",
        ]).unwrap();

        assert_eq!(args.token, "hld_sub_test123");
        assert_eq!(args.forward, "http://localhost:8080/hooks");
        assert_eq!(args.herald_url, "wss://api.herald.dev/v1/tunnel");
    }

    #[test]
    fn test_args_missing_token_fails() {
        let result = Args::try_parse_from([
            "herald-agent",
            "--forward", "http://localhost:8080/hooks",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn test_args_missing_forward_fails() {
        let result = Args::try_parse_from([
            "herald-agent",
            "--token", "hld_sub_test123",
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_from_args() {
        let args = Args {
            token: "test_token".to_string(),
            forward: "http://localhost:9999".to_string(),
            herald_url: "wss://test.herald.dev".to_string(),
        };

        let config = AgentConfig {
            token: args.token.clone(),
            forward_url: args.forward.clone(),
            herald_url: args.herald_url.clone(),
        };

        assert_eq!(config.token, "test_token");
        assert_eq!(config.forward_url, "http://localhost:9999");
        assert_eq!(config.herald_url, "wss://test.herald.dev");
    }
}
