use clap::Parser;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use helios_verifiable_api_server::server::{Network, VerifiableApiServer};

#[tokio::main]
async fn main() {
    // Pre setup
    enable_tracing();

    // parse CLI arguments
    let cli = Cli::parse();

    // construct and start the server
    let mut server = VerifiableApiServer::new(cli.network);

    server.start().await.unwrap();
}

fn enable_tracing() {
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()
        .expect("invalid env filter");

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(env_filter)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("subscriber set failed");
}

#[derive(Parser)]
#[clap(version, about)]
/// Helios' Verifiable API server
struct Cli {
    #[command(subcommand)]
    network: Network,
}
