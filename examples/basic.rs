use std::{path::PathBuf, str::FromStr};

use env_logger::Env;
use ethers::{types::Address, utils};
use dirs::home_dir;
use eyre::Result;
use helios::{config::networks::Network, prelude::*};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let untrusted_execution_rpc_url = std::env::var("MAINNET_EXECUTION_RPC")?;
    log::info!("Using untrusted RPC URL [REDACTED]");

    let consensus_rpc_url = std::env::var("MAINNET_CONSENSUS_RPC")?;
    log::info!("Using consensus RPC URL: {}", consensus_rpc_url);

    let mainnet_data_dir_ext = std::env::var("MAINNET_DATA_DIR_EXT")?;
    let data_path = home_dir().unwrap().join(mainnet_data_dir_ext);

    let mut client: Client<FileDB> = ClientBuilder::new()
        .network(Network::MAINNET)
        .consensus_rpc(&consensus_rpc_url)
        .execution_rpc(&untrusted_execution_rpc_url)
        .load_external_fallback()
        .data_dir(PathBuf::from(data_path))
        .build()?;

    log::info!(
        "Built client on network \"{}\" with external checkpoint fallbacks",
        Network::MAINNET
    );

    client.start().await?;

    let head_block_num = client.get_block_number().await?;
    let addr = Address::from_str("0x00000000219ab540356cBB839Cbe05303d7705Fa")?;
    let block = BlockTag::Latest;
    let balance = client.get_balance(&addr, block).await?;

    log::info!("synced up to block: {}", head_block_num);
    log::info!(
        "balance of deposit contract: {}",
        utils::format_ether(balance)
    );

    Ok(())
}
