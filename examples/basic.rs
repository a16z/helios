use std::{path::PathBuf, str::FromStr};

use alloy::primitives::{utils::format_ether, Address};
use eyre::Result;
use helios::{
    common::types::BlockTag,
    ethereum::{
        config::networks::Network, database::FileDB, EthereumClient, EthereumClientBuilder,
    },
};
use tracing::info;
use tracing_subscriber::{
    filter::{EnvFilter, LevelFilter},
    FmtSubscriber,
};

#[tokio::main]
async fn main() -> Result<()> {
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()
        .expect("invalid env filter");

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(env_filter)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("subscriber set failed");

    let untrusted_rpc_url = "https://eth-mainnet.g.alchemy.com/v2/<YOUR_API_KEY>";
    info!("Using untrusted RPC URL [REDACTED]");

    let consensus_rpc = "https://www.lightclientdata.org";
    info!("Using consensus RPC URL: {}", consensus_rpc);

    let mut client: EthereumClient<FileDB> = EthereumClientBuilder::new()
        .network(Network::Mainnet)
        .consensus_rpc(consensus_rpc)
        .execution_rpc(untrusted_rpc_url)
        .load_external_fallback()
        .data_dir(PathBuf::from("/tmp/helios"))
        .build()?;

    info!(
        "Built client on network \"{}\" with external checkpoint fallbacks",
        Network::Mainnet
    );

    client.start().await?;
    client.wait_synced().await;

    let client_version = client.client_version().await;
    let head_block_num = client.get_block_number().await?;
    let addr = Address::from_str("0x00000000219ab540356cBB839Cbe05303d7705Fa")?;
    let block = BlockTag::Latest;
    let balance = client.get_balance(addr, block).await?;

    info!("client version: {}", client_version);
    info!("synced up to block: {}", head_block_num);
    info!("balance of deposit contract: {}", format_ether(balance));

    Ok(())
}
