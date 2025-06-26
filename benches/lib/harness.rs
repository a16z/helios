#![allow(dead_code)]
use std::{path::PathBuf, str::FromStr};

use alloy::eips::BlockNumberOrTag;
use alloy::primitives::{Address, B256, U256};
use helios_ethereum::{
    config::{checkpoints, networks},
    database::FileDB,
    EthereumClient, EthereumClientBuilder,
};

/// Fetches the latest mainnet checkpoint from the fallback service.
///
/// Uses the [CheckpointFallback](config::CheckpointFallback).
/// The `build` method will fetch a list of [CheckpointFallbackService](config::CheckpointFallbackService)s from a community-maintained list by ethPandaOps.
/// This list is NOT guaranteed to be secure, but is provided in good faith.
/// The raw list can be found here: https://github.com/ethpandaops/checkpoint-sync-health-checks/blob/master/_data/endpoints.yaml
pub async fn fetch_mainnet_checkpoint() -> eyre::Result<B256> {
    let cf = checkpoints::CheckpointFallback::new()
        .build()
        .await
        .unwrap();
    cf.fetch_latest_checkpoint(&networks::Network::Mainnet)
        .await
}

/// Constructs a mainnet [Client](client::Client) for benchmark usage.
///
/// Requires a [Runtime](tokio::runtime::Runtime) to be passed in by reference.
/// The client is parameterized with a [FileDB](client::FileDB).
/// It will also use the environment variable `MAINNET_EXECUTION_RPC` to connect to a mainnet node.
/// The client will use `https://www.lightclientdata.org` as the consensus RPC.
pub fn construct_mainnet_client(rt: &tokio::runtime::Runtime) -> eyre::Result<EthereumClient> {
    rt.block_on(inner_construct_mainnet_client())
}

pub async fn inner_construct_mainnet_client() -> eyre::Result<EthereumClient> {
    let benchmark_rpc_url = std::env::var("MAINNET_EXECUTION_RPC")?;

    let client = EthereumClientBuilder::<FileDB>::new()
        .network(networks::Network::Mainnet)
        .consensus_rpc("https://www.lightclientdata.org")?
        .execution_rpc(&benchmark_rpc_url)?
        .load_external_fallback()
        .data_dir(PathBuf::from("/tmp/helios"))
        .build()?;
    // Wait for the client to be synced.
    client.wait_synced().await;

    Ok(client)
}

pub async fn construct_mainnet_client_with_checkpoint(
    checkpoint: B256,
) -> eyre::Result<EthereumClient> {
    let benchmark_rpc_url = std::env::var("MAINNET_EXECUTION_RPC")?;

    let client = EthereumClientBuilder::<FileDB>::new()
        .network(networks::Network::Mainnet)
        .consensus_rpc("https://www.lightclientdata.org")?
        .execution_rpc(&benchmark_rpc_url)?
        .checkpoint(checkpoint)
        .data_dir(PathBuf::from("/tmp/helios"))
        .build()?;
    // Wait for the client to be synced.
    client.wait_synced().await;

    Ok(client)
}

/// Create a tokio multi-threaded runtime.
///
/// # Panics
///
/// Panics if the runtime cannot be created.
pub fn construct_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// Constructs a sepolia client for benchmark usage.
///
/// Requires a [Runtime](tokio::runtime::Runtime) to be passed in by reference.
/// The client is parameterized with a [FileDB](client::FileDB).
/// It will also use the environment variable `SEPOLIA_EXECUTION_RPC` to connect to a mainnet node.
/// The client will use `http://unstable.sepolia.beacon-api.nimbus.team/` as the consensus RPC.
pub fn construct_sepolia_client(rt: &tokio::runtime::Runtime) -> eyre::Result<EthereumClient> {
    rt.block_on(async {
        let benchmark_rpc_url = std::env::var("SEPOLIA_EXECUTION_RPC")?;
        let client = EthereumClientBuilder::<FileDB>::new()
            .network(networks::Network::Sepolia)
            .consensus_rpc("http://unstable.sepolia.beacon-api.nimbus.team/")?
            .execution_rpc(&benchmark_rpc_url)?
            .data_dir(PathBuf::from("/tmp/helios"))
            .load_external_fallback()
            .build()?;
        // Wait for the client to be synced.
        client.wait_synced().await;

        Ok(client)
    })
}

/// Gets the balance of the given address on mainnet.
pub fn get_balance(
    rt: &tokio::runtime::Runtime,
    client: EthereumClient,
    address: &str,
) -> eyre::Result<U256> {
    rt.block_on(async {
        let block = BlockNumberOrTag::Latest;
        let address = Address::from_str(address)?;
        client.get_balance(address, block.into()).await
    })
}

// h/t @ https://github.com/0xethsign
// rev: https://github.com/0xethsign/casbin-rs/commit/7a0a75d8075440ee65acdac3ee9c0de6fcbd5c48
pub fn await_future<F: std::future::Future<Output = T>, T>(future: F) -> T {
    tokio::runtime::Runtime::new().unwrap().block_on(future)
}
