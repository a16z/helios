use std::env;

use alloy::eips::BlockNumberOrTag;
use alloy::primitives::address;
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::rpc::client::ClientBuilder as AlloyClientBuilder;
use alloy::sol;
use alloy::transports::http::{Client as ReqwestClient, Http};
use alloy::transports::layers::{RetryBackoffLayer, RetryBackoffService};
use pretty_assertions::assert_eq;
use rand::Rng;

use helios::ethereum::{
    config::networks::Network, database::ConfigDB, EthereumClient, EthereumClientBuilder,
};

async fn setup() -> (
    EthereumClient<ConfigDB>,
    RootProvider<Http<ReqwestClient>>,
    RootProvider<RetryBackoffService<Http<ReqwestClient>>>,
) {
    let execution_rpc = env::var("MAINNET_EXECUTION_RPC").unwrap();
    let consensus_rpc = "https://www.lightclientdata.org";

    let port = rand::thread_rng().gen_range(0..=65535);

    let mut helios_client = EthereumClientBuilder::new()
        .network(Network::MAINNET)
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

    let tx = provider
        .get_transaction_by_hash(tx_hash)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(helios_tx, tx);
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
//     let block = provider
//         .get_block_by_number(helios_block.header.number.unwrap().into(), false)
//         .await
//         .unwrap()
//         .unwrap();
//
//     assert_eq!(helios_block, block);
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

    let receipt = provider
        .get_transaction_receipt(tx_hash)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(helios_receipt, receipt);
}

#[tokio::test]
async fn get_block_receipts() {
    let (_handle, helios_provider, provider) = setup().await;

    let block = helios_provider
        .get_block_by_number(BlockNumberOrTag::Latest, false)
        .await
        .unwrap()
        .unwrap();

    let block_num = block.header.number.unwrap().into();

    let helios_receipts = helios_provider
        .get_block_receipts(block_num)
        .await
        .unwrap()
        .unwrap();

    let receipts = provider
        .get_block_receipts(block_num)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(helios_receipts, receipts);
}

#[tokio::test]
async fn get_balance() {
    let (_handle, helios_provider, provider) = setup().await;
    let num = helios_provider.get_block_number().await.unwrap();

    let address = address!("00000000219ab540356cBB839Cbe05303d7705Fa");
    let helios_balance = helios_provider
        .get_balance(address)
        .block_id(num.into())
        .await
        .unwrap();

    let balance = provider
        .get_balance(address)
        .block_id(num.into())
        .await
        .unwrap();

    assert_eq!(helios_balance, balance);
}

#[tokio::test]
async fn call() {
    let (_handle, helios_provider, provider) = setup().await;
    let num = helios_provider.get_block_number().await.unwrap();
    let usdc = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    let user = address!("99C9fc46f92E8a1c0deC1b1747d010903E884bE1");

    sol! {
        #[sol(rpc)]
        interface ERC20 {
            function balanceOf(address who) external returns (uint256);
        }
    }

    let helios_token = ERC20::new(usdc, helios_provider);
    let token = ERC20::new(usdc, provider);

    let helios_balance = helios_token
        .balanceOf(user)
        .block(num.into())
        .call()
        .await
        .unwrap()
        ._0;

    let balance = token
        .balanceOf(user)
        .block(num.into())
        .call()
        .await
        .unwrap()
        ._0;

    assert_eq!(helios_balance, balance);
}
