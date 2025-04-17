use std::env;

use alloy::eips::BlockNumberOrTag;
use alloy::network::primitives::BlockTransactionsKind;
use alloy::primitives::address;
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::rpc::client::ClientBuilder as AlloyClientBuilder;
use alloy::rpc::types::Filter;
use alloy::sol;
use alloy::transports::http::{Client as ReqwestClient, Http};
use futures::future::join_all;
use pretty_assertions::assert_eq;
use rand::Rng;
use url::Url;

use helios::ethereum::{
    config::networks::Network, database::ConfigDB, EthereumClient, EthereumClientBuilder,
};
use helios_verifiable_api_server::server::{
    Network as ApiNetwork, ServerArgs, VerifiableApiServer,
};

async fn setup() -> (
    EthereumClient<ConfigDB>,
    EthereumClient<ConfigDB>,
    VerifiableApiServer,
    Vec<RootProvider<Http<ReqwestClient>>>,
) {
    let execution_rpc = env::var("MAINNET_EXECUTION_RPC").unwrap();
    let consensus_rpc = "https://www.lightclientdata.org";

    let mut rng = rand::thread_rng();

    // Direct provider
    let client = AlloyClientBuilder::default().http(execution_rpc.parse().unwrap());
    let provider = ProviderBuilder::new().on_client(client);

    // Helios provider (RPC)
    let (helios_client, helios_provider) = {
        let port = rng.gen_range(1024..=65535);
        let mut helios_client = EthereumClientBuilder::new()
            .network(Network::Mainnet)
            .execution_rpc(&execution_rpc)
            .consensus_rpc(consensus_rpc)
            .load_external_fallback()
            .strict_checkpoint_age()
            .rpc_port(port)
            .build()
            .unwrap();

        helios_client.start().await.unwrap();

        let url = format!("http://localhost:{}", port).parse().unwrap();
        let helios_provider = ProviderBuilder::new().on_http(url);

        (helios_client, helios_provider)
    };

    // Start Verifiable API server that'd wrap the given RPC
    let api_port = rng.gen_range(1024..=65535);
    let mut api_server = VerifiableApiServer::new(ApiNetwork::Ethereum(ServerArgs {
        server_address: format!("127.0.0.1:{api_port}").parse().unwrap(),
        execution_rpc: Url::parse(&execution_rpc).unwrap(),
    }));
    api_server.start();

    // Helios provider (Verifiable API)
    let (helios_client_api, helios_provider_api) = {
        let port = rng.gen_range(1024..=65535);
        let mut helios_client = EthereumClientBuilder::new()
            .network(Network::Mainnet)
            .execution_verifiable_api(&format!("http://localhost:{api_port}"))
            .consensus_rpc(consensus_rpc)
            .load_external_fallback()
            .strict_checkpoint_age()
            .rpc_port(port)
            .build()
            .unwrap();

        helios_client.start().await.unwrap();

        let url = format!("http://localhost:{}", port).parse().unwrap();
        let helios_provider = ProviderBuilder::new().on_http(url);

        (helios_client, helios_provider)
    };

    join_all(vec![
        helios_client.wait_synced(),
        helios_client_api.wait_synced(),
    ])
    .await;

    (
        helios_client,
        helios_client_api,
        api_server,
        vec![helios_provider_api, helios_provider, provider],
    )
}

fn rpc_exists() -> bool {
    env::var("MAINNET_EXECUTION_RPC").is_ok_and(|rpc| !rpc.is_empty())
}

#[tokio::test]
async fn get_transaction_by_hash() {
    if !rpc_exists() {
        return;
    }

    let (_handle1, _handle2, _handle3, providers) = setup().await;
    let helios_api = providers[0].clone();
    let helios_rpc = providers[1].clone();
    let provider = providers[2].clone();

    let block_api = helios_api
        .get_block_by_number(BlockNumberOrTag::Latest, BlockTransactionsKind::Hashes)
        .await
        .unwrap()
        .unwrap();
    let block_rpc = helios_rpc
        .get_block_by_number(BlockNumberOrTag::Latest, BlockTransactionsKind::Hashes)
        .await
        .unwrap()
        .unwrap();

    let tx_hash = block_api.transactions.hashes().next().unwrap();
    let tx = helios_api
        .get_transaction_by_hash(tx_hash)
        .await
        .unwrap()
        .unwrap();
    let expected = provider
        .get_transaction_by_hash(tx_hash)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(tx, expected);

    let tx_hash = block_rpc.transactions.hashes().next().unwrap();
    let tx = helios_rpc
        .get_transaction_by_hash(tx_hash)
        .await
        .unwrap()
        .unwrap();
    let expected = provider
        .get_transaction_by_hash(tx_hash)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(tx, expected);
}

// #[tokio::test]
// async fn get_block_by_number() {
//     let (_handle1, _handle2, _handle3, providers) = setup().await;
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
    if !rpc_exists() {
        return;
    }

    let (_handle1, _handle2, _handle3, providers) = setup().await;
    let helios_api = providers[0].clone();
    let helios_rpc = providers[1].clone();
    let provider = providers[2].clone();

    let block_api = helios_api
        .get_block_by_number(BlockNumberOrTag::Latest, BlockTransactionsKind::Hashes)
        .await
        .unwrap()
        .unwrap();
    let block_rpc = helios_rpc
        .get_block_by_number(BlockNumberOrTag::Latest, BlockTransactionsKind::Hashes)
        .await
        .unwrap()
        .unwrap();

    let tx_hash = block_api.transactions.hashes().next().unwrap();
    let receipt = helios_api
        .get_transaction_receipt(tx_hash)
        .await
        .unwrap()
        .unwrap();
    let expected = provider
        .get_transaction_receipt(tx_hash)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(receipt, expected);

    let tx_hash = block_rpc.transactions.hashes().next().unwrap();
    let receipt = helios_rpc
        .get_transaction_receipt(tx_hash)
        .await
        .unwrap()
        .unwrap();
    let expected = provider
        .get_transaction_receipt(tx_hash)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(receipt, expected);
}

#[tokio::test]
async fn get_block_receipts() {
    if !rpc_exists() {
        return;
    }

    let (_handle1, _handle2, _handle3, providers) = setup().await;
    let helios_api = providers[0].clone();
    let helios_rpc = providers[1].clone();
    let provider = providers[2].clone();

    let helios_api_block_num = helios_api.get_block_number().await.unwrap();
    let helios_rpc_block_num = helios_rpc.get_block_number().await.unwrap();

    let receipts = helios_api
        .get_block_receipts(helios_api_block_num.into())
        .await
        .unwrap()
        .unwrap();
    let expected = provider
        .get_block_receipts(helios_api_block_num.into())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(receipts, expected);

    let receipts = helios_rpc
        .get_block_receipts(helios_rpc_block_num.into())
        .await
        .unwrap()
        .unwrap();
    let expected = provider
        .get_block_receipts(helios_rpc_block_num.into())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(receipts, expected);
}

#[tokio::test]
async fn get_balance() {
    if !rpc_exists() {
        return;
    }

    let (_handle1, _handle2, _handle3, providers) = setup().await;
    let helios_api = providers[0].clone();
    let helios_rpc = providers[1].clone();
    let provider = providers[2].clone();

    let helios_api_block_num = helios_api.get_block_number().await.unwrap();
    let helios_rpc_block_num = helios_rpc.get_block_number().await.unwrap();

    let address = address!("00000000219ab540356cBB839Cbe05303d7705Fa");

    let balance = helios_api
        .get_balance(address)
        .block_id(helios_api_block_num.into())
        .await
        .unwrap();
    let expected = provider
        .get_balance(address)
        .block_id(helios_api_block_num.into())
        .await
        .unwrap();
    assert_eq!(balance, expected);

    let balance = helios_rpc
        .get_balance(address)
        .block_id(helios_rpc_block_num.into())
        .await
        .unwrap();
    let expected = provider
        .get_balance(address)
        .block_id(helios_rpc_block_num.into())
        .await
        .unwrap();
    assert_eq!(balance, expected);
}

#[tokio::test]
async fn call() {
    if !rpc_exists() {
        return;
    }

    let (_handle1, _handle2, _handle3, providers) = setup().await;
    let helios_api = providers[0].clone();
    let helios_rpc = providers[1].clone();
    let provider = providers[2].clone();

    let helios_api_block_num = helios_api.get_block_number().await.unwrap();
    let helios_rpc_block_num = helios_rpc.get_block_number().await.unwrap();

    let usdc = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    let user = address!("99C9fc46f92E8a1c0deC1b1747d010903E884bE1");

    sol! {
        #[sol(rpc)]
        interface ERC20 {
            function balanceOf(address who) external returns (uint256);
        }
    }

    let token_api = ERC20::new(usdc, helios_api);
    let token_rpc = ERC20::new(usdc, helios_rpc);
    let token_provider = ERC20::new(usdc, provider);

    let balance = token_api
        .balanceOf(user)
        .block(helios_api_block_num.into())
        .call()
        .await
        .unwrap()
        ._0;
    let expected = token_provider
        .balanceOf(user)
        .block(helios_api_block_num.into())
        .call()
        .await
        .unwrap()
        ._0;
    assert_eq!(balance, expected);

    let balance = token_rpc
        .balanceOf(user)
        .block(helios_rpc_block_num.into())
        .call()
        .await
        .unwrap()
        ._0;
    let expected = token_provider
        .balanceOf(user)
        .block(helios_rpc_block_num.into())
        .call()
        .await
        .unwrap()
        ._0;
    assert_eq!(balance, expected);
}

#[tokio::test]
async fn get_logs() {
    if !rpc_exists() {
        return;
    }

    let (_handle1, _handle2, _handle3, providers) = setup().await;
    let helios_api = providers[0].clone();
    let helios_rpc = providers[1].clone();
    let provider = providers[2].clone();

    let block_api = helios_api
        .get_block_by_number(BlockNumberOrTag::Latest, BlockTransactionsKind::Hashes)
        .await
        .unwrap()
        .unwrap();
    let block_rpc = helios_rpc
        .get_block_by_number(BlockNumberOrTag::Latest, BlockTransactionsKind::Hashes)
        .await
        .unwrap()
        .unwrap();

    let filter = Filter::new().at_block_hash(block_api.header.hash);
    let logs = helios_api.get_logs(&filter).await.unwrap();
    let expected = provider.get_logs(&filter).await.unwrap();
    assert_eq!(logs, expected);

    let filter = Filter::new().at_block_hash(block_rpc.header.hash);
    let logs = helios_rpc.get_logs(&filter).await.unwrap();
    let expected = provider.get_logs(&filter).await.unwrap();
    assert_eq!(logs, expected);
}
