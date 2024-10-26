use std::net::SocketAddr;

use clap::Parser;
use eyre::Result;
use tracing_subscriber::{EnvFilter, FmtSubscriber};
use url::Url;

use helios_opstack::{
    config::{Network, NetworkConfig},
    server::start_server,
};

#[tokio::main]
async fn main() -> Result<()> {
    enable_tracing();
    let cli = Cli::parse();
    let config = NetworkConfig::from(cli.network);

    let chain_id = config.chain.chain_id;
    let unsafe_signer = config.chain.unsafe_signer;
    let server_addr = cli.server_address;
    let gossip_addr = cli.gossip_address;
    let replica_urls = cli.replica_urls.unwrap_or_default();

    start_server(
        server_addr,
        gossip_addr,
        chain_id,
        unsafe_signer,
        replica_urls,
    )
    .await?;

    Ok(())
}

fn enable_tracing() {
    let env_filter = EnvFilter::builder()
        .with_default_directive("helios_opstack=info".parse().unwrap())
        .from_env()
        .expect("invalid env filter");

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(env_filter)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("subscriber set failed");
}

#[derive(Parser)]
struct Cli {
    #[clap(short, long)]
    network: Network,
    #[clap(short, long, default_value = "127.0.0.1:3000")]
    server_address: SocketAddr,
    #[clap(short, long, default_value = "0.0.0.0:9876")]
    gossip_address: SocketAddr,
    #[clap(short, long, value_delimiter = ',')]
    replica_urls: Option<Vec<Url>>,
}
