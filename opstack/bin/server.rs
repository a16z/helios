use std::net::SocketAddr;

#[cfg(not(target_arch = "wasm32"))]
use clap::Parser;
use eyre::Result;
use tracing_subscriber::{EnvFilter, FmtSubscriber};
use url::Url;

#[cfg(not(target_arch = "wasm32"))]
use helios_opstack::{
    config::{Network, NetworkConfig},
    server::start_server,
};

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() -> Result<()> {
    enable_tracing();
    let cli = Cli::parse();
    let config = NetworkConfig::from(cli.network);

    let chain_id = config.chain.chain_id;
    let unsafe_signer = config.chain.unsafe_signer;
    let system_config_contract = config.chain.system_config_contract;
    let server_addr = cli.server_address;
    let gossip_addr = cli.gossip_address;
    let replica_urls = cli.replica_urls.unwrap_or_default();
    let execution_rpc = cli.execution_rpc;

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

#[cfg(target_arch = "wasm32")]
fn main() -> Result<()> {
    eyre::bail!("server not supported in wasm");
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(not(target_arch = "wasm32"))]
#[derive(Parser)]
struct Cli {
    #[arg(short, long)]
    network: Network,
    #[arg(short, long, default_value = "127.0.0.1:3000")]
    server_address: SocketAddr,
    #[arg(short, long, default_value = "0.0.0.0:9876")]
    gossip_address: SocketAddr,
    #[arg(short, long, value_delimiter = ',')]
    replica_urls: Option<Vec<Url>>,
    #[arg(short, long)]
    execution_rpc: Url,
}
