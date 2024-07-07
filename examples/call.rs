#![allow(deprecated)]

use std::path::PathBuf;

use dotenv::dotenv;
use ethers::prelude::*;
use helios::{
    client::{Client, ClientBuilder, FileDB},
    config::networks::Network,
    types::{BlockTag, CallOpts},
};
use tracing::info;
use tracing_subscriber::{
    filter::{EnvFilter, LevelFilter},
    FmtSubscriber,
};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()
        .expect("invalid env filter");

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(env_filter)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("subsriber set failed");

    // Load the rpc url using the `MAINNET_EXECUTION_RPC` environment variable
    dotenv().ok();
    let eth_rpc_url = std::env::var("MAINNET_EXECUTION_RPC")?;
    let consensus_rpc = "https://www.lightclientdata.org";
    info!("Consensus RPC URL: {}", consensus_rpc);

    // Construct the client
    let data_dir = PathBuf::from("/tmp/helios");
    let mut client: Client<FileDB> = ClientBuilder::new()
        .network(Network::MAINNET)
        .data_dir(data_dir)
        .consensus_rpc(consensus_rpc)
        .execution_rpc(&eth_rpc_url)
        .load_external_fallback()
        .build()?;

    info!(
        "[\"{}\"] Client built with external checkpoint fallbacks",
        Network::MAINNET
    );

    // Start the client
    client.start().await?;
    client.wait_synced().await;

    // Call on helios client
    let call_opts = CallOpts {
        from: None,
        to: Some("0x6B175474E89094C44Da98b954EedeAC495271d0F".parse::<Address>()?),
        gas: None,
        gas_price: None,
        value: None,
        data: Some("0x18160ddd".parse::<Bytes>()?),
    };

    let result = client.call(&call_opts, BlockTag::Latest).await?;
    info!("[HELIOS] DAI total supply: {:?}", result);

    Ok(())
}
