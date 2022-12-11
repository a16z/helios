#![allow(dead_code)]

use helios::{client, config::networks};

/// Constructs a mainnet client for benchmark usage.
pub fn construct_mainnet_client(
    rt: &tokio::runtime::Runtime,
) -> eyre::Result<client::Client<client::FileDB>> {
    rt.block_on(async {
        let benchmark_rpc_url = std::env::var("MAINNET_RPC_URL")?;
        let mut client = client::ClientBuilder::new()
            .network(networks::Network::MAINNET)
            .consensus_rpc("https://www.lightclientdata.org")
            .execution_rpc(&benchmark_rpc_url)
            .load_external_fallback()
            .build()?;
        client.start().await?;
        Ok(client)
    })
}

/// Constructs a goerli client for benchmark usage.
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
