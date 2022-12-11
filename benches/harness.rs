#![allow(dead_code)]

use std::{str::FromStr, sync::Arc};

use ::client::Client;
use ethers::{
    abi::Address,
    types::{H256, U256},
};
use helios::{client, config::networks, prelude::FileDB, types::BlockTag};

/// Fetches the latest mainnet checkpoint from the fallback service.
///
/// Uses the [CheckpointFallback](config::CheckpointFallback).
/// The `build` method will fetch a list of [CheckpointFallbackService](config::CheckpointFallbackService)s from a community-mainained list by ethPandaOps.
/// This list is NOT guaranteed to be secure, but is provided in good faith.
/// The raw list can be found here: https://github.com/ethpandaops/checkpoint-sync-health-checks/blob/master/_data/endpoints.yaml
pub async fn fetch_mainnet_checkpoint() -> eyre::Result<H256> {
    let cf = config::CheckpointFallback::new().build().await.unwrap();
    cf.fetch_latest_checkpoint(&networks::Network::MAINNET)
        .await
}

/// Constructs a mainnet [Client](client::Client) for benchmark usage.
///
/// Requires a [Runtime](tokio::runtime::Runtime) to be passed in by reference.
/// The client is parameterized with a [FileDB](client::FileDB).
/// It will also use the environment variable `MAINNET_RPC_URL` to connect to a mainnet node.
/// The client will use `https://www.lightclientdata.org` as the consensus RPC.
pub fn construct_mainnet_client(
    rt: &tokio::runtime::Runtime,
) -> eyre::Result<client::Client<client::FileDB>> {
    rt.block_on(inner_construct_mainnet_client())
}

pub async fn inner_construct_mainnet_client() -> eyre::Result<client::Client<client::FileDB>> {
    let benchmark_rpc_url = std::env::var("MAINNET_RPC_URL")?;
    let mut client = client::ClientBuilder::new()
        .network(networks::Network::MAINNET)
        .consensus_rpc("https://www.lightclientdata.org")
        .execution_rpc(&benchmark_rpc_url)
        .load_external_fallback()
        .build()?;
    client.start().await?;
    Ok(client)
}

pub async fn construct_mainnet_client_with_checkpoint(
    checkpoint: &str,
) -> eyre::Result<client::Client<client::FileDB>> {
    let benchmark_rpc_url = std::env::var("MAINNET_RPC_URL")?;
    let mut client = client::ClientBuilder::new()
        .network(networks::Network::MAINNET)
        .consensus_rpc("https://www.lightclientdata.org")
        .execution_rpc(&benchmark_rpc_url)
        .checkpoint(checkpoint)
        .build()?;
    client.start().await?;
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

/// Constructs a goerli client for benchmark usage.
///
/// Requires a [Runtime](tokio::runtime::Runtime) to be passed in by reference.
/// The client is parameterized with a [FileDB](client::FileDB).
/// It will also use the environment variable `GOERLI_RPC_URL` to connect to a mainnet node.
/// The client will use `http://testing.prater.beacon-api.nimbus.team` as the consensus RPC.
pub fn construct_goerli_client(
    rt: &tokio::runtime::Runtime,
) -> eyre::Result<client::Client<client::FileDB>> {
    rt.block_on(async {
        let benchmark_rpc_url = std::env::var("GOERLI_RPC_URL")?;
        let mut client = client::ClientBuilder::new()
            .network(networks::Network::GOERLI)
            .consensus_rpc("http://testing.prater.beacon-api.nimbus.team")
            .execution_rpc(&benchmark_rpc_url)
            .load_external_fallback()
            .build()?;
        client.start().await?;
        Ok(client)
    })
}

/// Gets the balance of the given address on mainnet.
pub fn get_balance(
    rt: &tokio::runtime::Runtime,
    client: Arc<Client<FileDB>>,
    address: &str,
) -> eyre::Result<U256> {
    rt.block_on(async {
        let block = BlockTag::Latest;
        let address = Address::from_str(address)?;
        client.get_balance(&address, block).await
    })
}

// h/t @ https://github.com/smrpn
// rev: https://github.com/smrpn/casbin-rs/commit/7a0a75d8075440ee65acdac3ee9c0de6fcbd5c48
pub fn await_future<F: std::future::Future<Output = T>, T>(future: F) -> T {
    tokio::runtime::Runtime::new().unwrap().block_on(future)
}
