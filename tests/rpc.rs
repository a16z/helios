use common::types::BlockTag;
use dotenv::dotenv;
use eyre::Result;
use env_logger::Builder;
use helios::{config::networks::Network, prelude::*};
use std::{env, path::PathBuf};
use log::LevelFilter;

#[tokio::test]
async fn test_tests() {
    let mut builder = Builder::from_default_env();
    builder
        .filter(None, LevelFilter::Info)
        .init();
    assert!(true);
}

#[tokio::test]
async fn test_eth_get_block_by_number() -> Result<()> {
    dotenv().ok();
    let execution_rpc = match env::var("MAINNET_EXECUTION_RPC") {
        Ok(val) => val,
        Err(_) => {
            log::info!("Skipping feehistory test: MAINNET_EXECUTION_RPC env variable not set");
            return Ok(());
        }
    };
    let consensus_rpc = match env::var("MAINNET_CONSENSUS_RPC") {
        Ok(val) => val,
        Err(_) => {
            log::info!("Skipping feehistory test: MAINNET_CONSENSUS_RPC env variable not set");
            return Ok(());
        }
    };
    log::info!("Using consensus RPC URL: {}", consensus_rpc);
    let data_dir = "/tmp/helios";
    let client: Client<FileDB> = ClientBuilder::new()
        .network(Network::MAINNET)
        .consensus_rpc(&consensus_rpc)
        .execution_rpc(&execution_rpc)
        .load_external_fallback()
        .data_dir(PathBuf::from(data_dir))
        .build()?;
    let block = client
        .get_block_by_number(BlockTag::Number(1), false)
        .await
        .unwrap();
    assert!(block.is_none());
    return Ok(());
}
