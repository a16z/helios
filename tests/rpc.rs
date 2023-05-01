use common::types::BlockTag;
use dotenv::dotenv;
use env_logger::Builder;
use eyre::Result;
use helios::{config::networks::Network, prelude::*};
use log::LevelFilter;
use std::{env, path::PathBuf};

#[tokio::test]
async fn test_tests() {
    let mut builder = Builder::from_default_env();
    builder.filter(None, LevelFilter::Info).init();
    assert!(true);
}

async fn launch_client() -> Result<Client<FileDB>, eyre::Error> {
    dotenv().ok();
    let execution_rpc = match env::var("MAINNET_EXECUTION_RPC") {
        Ok(val) => val,
        Err(_) => {
            log::info!("Skipping feehistory test: MAINNET_EXECUTION_RPC env variable not set");
            return Err(eyre::Error::msg(
                "MAINNET_EXECUTION_RPC env variable not set",
            ));
        }
    };
    let consensus_rpc = match env::var("MAINNET_CONSENSUS_RPC") {
        Ok(val) => val,
        Err(_) => {
            log::info!("Skipping feehistory test: MAINNET_CONSENSUS_RPC env variable not set");
            return Err(eyre::Error::msg(
                "MAINNET_CONSENSUS_RPC env variable not set",
            ));
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
    return Ok(client);
}

#[tokio::test]
async fn test_eth_get_block_by_number() {
    let client = match launch_client().await {
        Ok(c) => c,
        Err(e) => {
            log::warn!("Skipping test_eth_get_block_by_number: {}", e);
            return;
        }
    };
    let block = client
        .get_block_by_number(BlockTag::Number(1), false)
        .await
        .unwrap();
    assert!(block.is_none());
}

#[tokio::test]
async fn test_eth_get_block_by_hash() {
    let client = match launch_client().await {
        Ok(c) => c,
        Err(e) => {
            log::warn!("Skipping test_eth_get_block_by_hash: {}", e);
            return;
        }
    };
    let first_eth_hash: Vec<u8> =
        "0x88e96d4537bea4d9c05d12549907b32561d3bf31f45aae734cdc119f13406cb6".into();
    let block = client
        .get_block_by_hash(&first_eth_hash, false)
        .await
        .unwrap();
    assert!(block.is_none());
}

#[tokio::test]
async fn test_eth_get_block_transaction_count_by_number() {
    let client = match launch_client().await {
        Ok(c) => c,
        Err(e) => {
            log::warn!(
                "Skipping test_eth_get_block_transaction_count_by_number: {}",
                e
            );
            return;
        }
    };
    let block = client
        .get_block_transaction_count_by_number(BlockTag::Number(1))
        .await
        .unwrap();
    assert!(true);
}
