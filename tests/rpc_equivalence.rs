use std::env;

use alloy::eips::BlockNumberOrTag;
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::rpc::client::ClientBuilder as AlloyClientBuilder;
use alloy::transports::http::{Client as ReqwestClient, Http};
use alloy::transports::layers::{RetryBackoffLayer, RetryBackoffService};
use pretty_assertions::assert_eq;

use helios::client::{Client, ClientBuilder};
use helios::consensus::database::ConfigDB;
use rand::Rng;

async fn setup() -> (
    Client<ConfigDB>,
    RootProvider<Http<ReqwestClient>>,
    RootProvider<RetryBackoffService<Http<ReqwestClient>>>,
) {
    let execution_rpc = env::var("MAINNET_EXECUTION_RPC").unwrap();
    let consensus_rpc = "https://www.lightclientdata.org";

    let port = rand::thread_rng().gen_range(0..=65535);

    let mut helios_client = ClientBuilder::new()
        .network(config::Network::MAINNET)
        .execution_rpc(&execution_rpc)
        .consensus_rpc(&consensus_rpc)
        .load_external_fallback()
        .strict_checkpoint_age()
        .rpc_port(port)
        .build()
        .unwrap();

    helios_client.start().await.unwrap();
    helios_client.wait_synced().await;

    let client = AlloyClientBuilder::default()
        .layer(RetryBackoffLayer::new(100, 50, 300))
        .http(execution_rpc.parse().unwrap());

    let provider = ProviderBuilder::new().on_client(client);

    let url = format!("http://localhost:{}", port).parse().unwrap();
    let helios_provider = ProviderBuilder::new().on_http(url);

    (helios_client, helios_provider, provider)
}

#[tokio::test]
async fn get_transaction_by_hash() {
    let (_handle, helios_provider, provider) = setup().await;

    let block = helios_provider
        .get_block_by_number(BlockNumberOrTag::Latest, false)
        .await
        .unwrap()
        .unwrap();

    let tx_hash = block.transactions.hashes().next().unwrap();
    let helios_tx = helios_provider
        .get_transaction_by_hash(tx_hash)
        .await
        .unwrap()
        .unwrap();

    let alloy_tx = provider
        .get_transaction_by_hash(tx_hash)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(helios_tx, alloy_tx);
}

// #[tokio::test]
// async fn get_block_by_number() {
//     let (_handle, helios_provider, provider) = setup().await;
// 
//     let helios_block = helios_provider
//         .get_block_by_number(BlockNumberOrTag::Latest, false)
//         .await
//         .unwrap()
//         .unwrap();
// 
//     let alloy_block = provider
//         .get_block_by_number(helios_block.header.number.unwrap().into(), false)
//         .await
//         .unwrap()
//         .unwrap();
// 
//     assert_eq!(helios_block, alloy_block);
// }

#[tokio::test]
async fn get_transaction_receipt() {
    let (_handle, helios_provider, provider) = setup().await;

    let block = helios_provider
        .get_block_by_number(BlockNumberOrTag::Latest, false)
        .await
        .unwrap()
        .unwrap();

    let tx_hash = block.transactions.hashes().next().unwrap();
    let helios_receipt = helios_provider
        .get_transaction_receipt(tx_hash)
        .await
        .unwrap()
        .unwrap();

    let alloy_receipt = provider
        .get_transaction_receipt(tx_hash)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(helios_receipt, alloy_receipt);
}
