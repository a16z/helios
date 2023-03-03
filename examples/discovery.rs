use std::path::PathBuf;

use env_logger::Env;
use eyre::Result;
use helios::{config::networks::Network, prelude::*};

use consensus::p2p::P2pNetworkInterface;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let untrusted_rpc_url = "https://eth-mainnet.g.alchemy.com/v2/<YOUR_API_KEY>";
    log::info!("Using untrusted RPC URL [REDACTED]");

    let consensus_rpc = "https://www.lightclientdata.org";
    log::info!("Using consensus RPC URL: {}", consensus_rpc);

    let mut client: Client<FileDB, P2pNetworkInterface> = ClientBuilder::new()
        .network(Network::MAINNET)
        .consensus_rpc(consensus_rpc)
        .execution_rpc(untrusted_rpc_url)
        .load_external_fallback()
        .data_dir(PathBuf::from("/tmp/helios"))
        .build()
        .await?;

    client.start().await?;

    Ok(())
}
