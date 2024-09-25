use alloy::primitives::address;
use eyre::Result;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

use helios_opstack::server::start_server;

#[tokio::main]
async fn main() -> Result<()> {
    enable_tracing();

    let server_addr = "127.0.0.1:3000".parse()?;
    let gossip_addr = "0.0.0.0:9876".parse()?;

    let signer = address!("Af6E19BE0F9cE7f8afd49a1824851023A8249e8a");
    let chain_id = 8453;

    start_server(server_addr, gossip_addr, chain_id, signer).await?;

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
