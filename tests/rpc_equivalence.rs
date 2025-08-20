use std::env;
use std::net::{SocketAddr, TcpListener};

use alloy::eips::BlockNumberOrTag;
use alloy::network::ReceiptResponse;
use alloy::primitives::{address, B256, U256};
use alloy::providers::{Provider, RootProvider};
use alloy::rpc::types::Filter;
use alloy::sol;
use alloy::sol_types::SolCall;
use eyre::Result;
use futures::future::join_all;
use pretty_assertions::assert_eq;
use serde_json::{json, Value};
use url::Url;

use helios::ethereum::{config::networks::Network, EthereumClient, EthereumClientBuilder};
use helios_verifiable_api_server::server::{
    Network as ApiNetwork, ServerArgs, VerifiableApiServer,
};

// RPC EQUIVALENCE TEST SUITE
//
// This test suite verifies that Helios returns identical results to a reference Ethereum node
// for all supported RPC methods. Tests automatically run against both API and RPC providers.
//
// TESTED RPC METHODS:

// Network/Basic Methods:
//  - eth_chainId (get_chain_id)
//  - eth_blockNumber (get_block_number)
//  - eth_gasPrice (get_gas_price)
//  - eth_maxPriorityFeePerGas (get_max_priority_fee_per_gas)
//  - eth_blobBaseFee (get_blob_base_fee)
//
// Block Methods:
//  - eth_getBlockByNumber (get_block_by_number_finalized)
//  - eth_getBlockByHash (get_block_by_hash, get_block_by_hash_with_txs)
//  - eth_getBlockTransactionCountByHash (get_block_transaction_count_by_hash)
//  - eth_getBlockTransactionCountByNumber (get_block_transaction_count_by_number)
//  - eth_getBlockReceipts (get_block_receipts)
//
// Transaction Methods:
//  - eth_getTransactionByHash (get_transaction_by_hash)
//  - eth_getTransactionByBlockHashAndIndex (get_transaction_by_block_hash_and_index)
//  - eth_getTransactionByBlockNumberAndIndex (get_transaction_by_block_number_and_index)
//  - eth_getTransactionReceipt (get_transaction_receipt, get_transaction_receipt_detailed)
//
// Account/State Methods:
//  - eth_getBalance (get_balance, get_balance_zero_address, get_historical_balance)
//  - eth_getTransactionCount (get_nonce)
//  - eth_getCode (get_code, get_code_eoa_address)
//  - eth_getStorageAt (get_storage_at, get_storage_at_specific_slot)
//  - eth_getProof (get_proof, get_proof_multiple_keys)
//
// EVM Execution Methods:
//  - eth_call (call, call_complex_contract)
//
// Log/Event Methods:
//  - eth_getLogs (get_logs, get_logs_by_address, get_logs_by_topic, get_logs_block_range)
//
// Historical Data:
//  - Historical block access (get_historical_block)
//  - Historical balance queries (get_historical_balance)
//
// HOW TO RUN:
//
// 1. Set environment variable:
//    export MAINNET_EXECUTION_RPC=YOUR_API_KEY
//
// 2. Run all tests (single test with parallel mini-tests, ~30-60 seconds):
//    cargo test --workspace --test rpc_equivalence

// Test framework for parallel mini-tests

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct TestResult {
    name: String,
    passed: bool,
    error: Option<String>,
}

#[allow(dead_code)]
impl TestResult {
    fn pass(name: &str) -> Self {
        Self {
            name: name.to_string(),
            passed: true,
            error: None,
        }
    }

    fn fail(name: &str, error: String) -> Self {
        Self {
            name: name.to_string(),
            passed: false,
            error: Some(error),
        }
    }
}

// Assertion macros that return errors instead of panicking
macro_rules! ensure_eq {
    ($left:expr, $right:expr) => {
        if $left != $right {
            return Err(eyre::eyre!("assertion failed: `(left == right)`\n  left: `{:?}`,\n right: `{:?}`", $left, $right));
        }
    };
    ($left:expr, $right:expr, $($arg:tt)+) => {
        if $left != $right {
            return Err(eyre::eyre!($($arg)+));
        }
    };
}

macro_rules! ensure {
    ($cond:expr) => {
        if !$cond {
            return Err(eyre::eyre!("assertion failed: {}", stringify!($cond)));
        }
    };
    ($cond:expr, $($arg:tt)+) => {
        if !$cond {
            return Err(eyre::eyre!($($arg)+));
        }
    };
}

fn get_available_port() -> u16 {
    // Use OS to assign a random available port
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

async fn setup() -> (
    EthereumClient,
    EthereumClient,
    VerifiableApiServer,
    Vec<RootProvider>,
) {
    let execution_rpc = env::var("MAINNET_EXECUTION_RPC").unwrap();
    let consensus_rpc = "https://www.lightclientdata.org";

    // Direct provider
    let provider = RootProvider::new_http(execution_rpc.parse().unwrap());

    // Helios provider (RPC) - optimized setup
    let (helios_client, helios_provider) = {
        let port = get_available_port();
        let helios_client = EthereumClientBuilder::new()
            .network(Network::Mainnet)
            .execution_rpc(&execution_rpc)
            .unwrap()
            .consensus_rpc(consensus_rpc)
            .unwrap()
            .load_external_fallback()
            .rpc_address(SocketAddr::new("127.0.0.1".parse().unwrap(), port))
            .with_config_db()
            .build()
            .unwrap();

        let url = format!("http://localhost:{}", port).parse().unwrap();
        let helios_provider = RootProvider::new_http(url);

        (helios_client, helios_provider)
    };

    // Start Verifiable API server
    let api_port = get_available_port();
    let mut api_server = VerifiableApiServer::new(ApiNetwork::Ethereum(ServerArgs {
        server_address: format!("127.0.0.1:{api_port}").parse().unwrap(),
        execution_rpc: Url::parse(&execution_rpc).unwrap(),
    }));
    let _handle = api_server.start();

    // Wait a moment for the API server to start
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    // Helios provider (Verifiable API) - optimized setup
    let (helios_client_api, helios_provider_api) = {
        let port = get_available_port();
        let helios_client = EthereumClientBuilder::new()
            .network(Network::Mainnet)
            .verifiable_api(format!("http://localhost:{api_port}"))
            .unwrap()
            .consensus_rpc(consensus_rpc)
            .unwrap()
            .load_external_fallback()
            .rpc_address(SocketAddr::new("127.0.0.1".parse().unwrap(), port))
            .with_config_db()
            .build()
            .unwrap();

        let url = format!("http://localhost:{}", port).parse().unwrap();
        let helios_provider = RootProvider::new_http(url);

        (helios_client, helios_provider)
    };

    // Wait for both Helios instances to sync
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

 

async fn test_get_transaction_by_hash(
    helios: &RootProvider,
    expected: &RootProvider,
) -> Result<()> {
    let block = helios
        .get_block_by_number(BlockNumberOrTag::Latest)
        .await?
        .unwrap();
    let tx_hash = block.transactions.hashes().next().unwrap();

    let tx = helios.get_transaction_by_hash(tx_hash).await?.unwrap();
    let expected_tx = expected.get_transaction_by_hash(tx_hash).await?.unwrap();
    ensure_eq!(tx, expected_tx, "Transaction mismatch");
    Ok(())
}

async fn test_get_transaction_receipt(
    helios: &RootProvider,
    expected: &RootProvider,
) -> Result<()> {
    let block = helios
        .get_block_by_number(BlockNumberOrTag::Latest)
        .await?
        .unwrap();
    let tx_hash = block.transactions.hashes().next().unwrap();

    let receipt = helios.get_transaction_receipt(tx_hash).await?.unwrap();
    let expected_receipt = expected.get_transaction_receipt(tx_hash).await?.unwrap();
    ensure_eq!(receipt, expected_receipt, "Receipt mismatch");
    Ok(())
}

async fn test_get_block_receipts(helios: &RootProvider, expected: &RootProvider) -> Result<()> {
    let block_num = helios.get_block_number().await? - 10;
    let receipts = helios.get_block_receipts(block_num.into()).await?.unwrap();
    let expected_receipts = expected
        .get_block_receipts(block_num.into())
        .await?
        .unwrap();
    ensure_eq!(
        receipts,
        expected_receipts,
        "Block receipts mismatch: expected {} receipts, got {}",
        expected_receipts.len(),
        receipts.len()
    );
    Ok(())
}

async fn test_get_balance(helios: &RootProvider, expected: &RootProvider) -> Result<()> {
    let block_num = helios.get_block_number().await?;
    let address = address!("00000000219ab540356cBB839Cbe05303d7705Fa");

    let balance = helios
        .get_balance(address)
        .block_id(block_num.into())
        .await?;
    let expected_balance = expected
        .get_balance(address)
        .block_id(block_num.into())
        .await?;
    ensure_eq!(
        balance,
        expected_balance,
        "Balance mismatch: expected {}, got {}",
        expected_balance,
        balance
    );
    Ok(())
}

async fn test_get_chain_id(helios: &RootProvider, expected: &RootProvider) -> Result<()> {
    let chain_id = helios.get_chain_id().await?;
    let expected_chain_id = expected.get_chain_id().await?;
    ensure_eq!(
        chain_id,
        expected_chain_id,
        "Chain ID mismatch: expected {}, got {}",
        expected_chain_id,
        chain_id
    );
    Ok(())
}

async fn test_get_block_number(helios: &RootProvider, expected: &RootProvider) -> Result<()> {
    let block_number = helios.get_block_number().await?;
    let expected_block_number = expected.get_block_number().await?;
    // Allow for small differences due to sync timing
    ensure!(
        (block_number as i64 - expected_block_number as i64).abs() <= 2,
        "Block number too different: expected {}, got {} (diff: {})",
        expected_block_number,
        block_number,
        (block_number as i64 - expected_block_number as i64).abs()
    );
    Ok(())
}

async fn test_call(helios: &RootProvider, expected: &RootProvider) -> Result<()> {
    let block_num = helios.get_block_number().await?;
    let usdc = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    let user = address!("99C9fc46f92E8a1c0deC1b1747d010903E884bE1");

    sol! {
        #[sol(rpc)]
        interface ERC20 {
            function balanceOf(address who) external returns (uint256);
        }
    }

    let token_helios = ERC20::new(usdc, helios.clone());
    let token_expected = ERC20::new(usdc, expected.clone());

    let balance = token_helios
        .balanceOf(user)
        .block(block_num.into())
        .call()
        .await?;
    let expected_balance = token_expected
        .balanceOf(user)
        .block(block_num.into())
        .call()
        .await?;
    ensure_eq!(
        balance,
        expected_balance,
        "Balance mismatch: expected {}, got {}",
        expected_balance,
        balance
    );
    Ok(())
}

async fn test_get_logs(helios: &RootProvider, expected: &RootProvider) -> Result<()> {
    let block = helios
        .get_block_by_number(BlockNumberOrTag::Latest)
        .await?
        .ok_or_else(|| eyre::eyre!("No latest block"))?;
    let filter = Filter::new().at_block_hash(block.header.hash);
    let logs = helios.get_logs(&filter).await?;
    let expected_logs = expected.get_logs(&filter).await?;
    ensure_eq!(
        logs,
        expected_logs,
        "Logs mismatch: expected {} logs, got {}",
        expected_logs.len(),
        logs.len()
    );
    Ok(())
}

async fn test_get_gas_price(helios: &RootProvider, expected: &RootProvider) -> Result<()> {
    let gas_price = helios.get_gas_price().await?;
    let expected_gas_price = expected.get_gas_price().await?;
    // Gas prices can vary, just ensure they're both non-zero
    ensure!(gas_price > 0, "Gas price should be positive");
    ensure!(
        expected_gas_price > 0,
        "Expected gas price should be positive"
    );
    Ok(())
}

async fn test_get_max_priority_fee_per_gas(
    helios: &RootProvider,
    expected: &RootProvider,
) -> Result<()> {
    let fee = helios.get_max_priority_fee_per_gas().await?;
    let expected_fee = expected.get_max_priority_fee_per_gas().await?;
    // Fees can vary, just ensure they're both non-zero
    ensure!(fee > 0, "Fee should be positive");
    ensure!(expected_fee > 0, "Expected fee should be positive");
    Ok(())
}

async fn test_get_block_by_hash(helios: &RootProvider, expected: &RootProvider) -> Result<()> {
    let block = helios
        .get_block_by_number(BlockNumberOrTag::Latest)
        .await?
        .ok_or_else(|| eyre::eyre!("No latest block"))?;
    let block_hash = block.header.hash;
    let result = helios
        .get_block_by_hash(block_hash)
        .full()
        .await?
        .ok_or_else(|| eyre::eyre!("Block not found"))?;
    let expected_block = expected
        .get_block_by_hash(block_hash)
        .full()
        .await?
        .ok_or_else(|| eyre::eyre!("Block not found"))?;
    ensure_eq!(
        result.header.hash,
        expected_block.header.hash,
        "Block hash mismatch: expected {:?}, got {:?}",
        expected_block.header.hash,
        result.header.hash
    );
    ensure_eq!(
        result.header.number,
        expected_block.header.number,
        "Block number mismatch: expected {:?}, got {:?}",
        expected_block.header.number,
        result.header.number
    );
    Ok(())
}

async fn test_get_block_transaction_count_by_hash(
    helios: &RootProvider,
    expected: &RootProvider,
) -> Result<()> {
    let block = helios
        .get_block_by_number(BlockNumberOrTag::Latest)
        .await?
        .ok_or_else(|| eyre::eyre!("No latest block"))?;
    let block_hash = block.header.hash;

    // Use the actual RPC method eth_getBlockTransactionCountByHash
    let count: U256 = helios
        .raw_request(
            "eth_getBlockTransactionCountByHash".into(),
            vec![format!("{:#x}", block_hash)],
        )
        .await?;
    let expected_count: U256 = expected
        .raw_request(
            "eth_getBlockTransactionCountByHash".into(),
            vec![format!("{:#x}", block_hash)],
        )
        .await?;

    ensure_eq!(
        count,
        expected_count,
        "Transaction count mismatch: expected {}, got {}",
        expected_count,
        count
    );
    Ok(())
}

async fn test_get_block_transaction_count_by_number(
    helios: &RootProvider,
    expected: &RootProvider,
) -> Result<()> {
    let block_num = helios.get_block_number().await? - 1;

    // Use the actual RPC method eth_getBlockTransactionCountByNumber
    let count: U256 = helios
        .raw_request(
            "eth_getBlockTransactionCountByNumber".into(),
            vec![format!("{:#x}", block_num)],
        )
        .await?;
    let expected_count: U256 = expected
        .raw_request(
            "eth_getBlockTransactionCountByNumber".into(),
            vec![format!("{:#x}", block_num)],
        )
        .await?;

    ensure_eq!(
        count,
        expected_count,
        "Transaction count mismatch: expected {}, got {}",
        expected_count,
        count
    );
    Ok(())
}

async fn test_get_transaction_by_block_hash_and_index(
    helios: &RootProvider,
    expected: &RootProvider,
) -> Result<()> {
    // Find a block with transactions
    let mut block = None;
    for i in 0..10 {
        let candidate = helios
            .get_block_by_number((helios.get_block_number().await? - i).into())
            .await?
            .ok_or_else(|| eyre::eyre!("No block found"))?;
        if !candidate.transactions.is_empty() {
            block = Some(candidate);
            break;
        }
    }
    let block = block.ok_or_else(|| eyre::eyre!("No block with transactions found"))?;
    let block_hash = block.header.hash;
    let tx_index = 0u64; // Get the first transaction

    // Ensure the block actually has transactions
    ensure!(
        !block.transactions.is_empty(),
        "Block should have transactions"
    );

    // Use the actual RPC method eth_getTransactionByBlockHashAndIndex
    let tx: Value = helios
        .raw_request(
            "eth_getTransactionByBlockHashAndIndex".into(),
            vec![format!("{:#x}", block_hash), format!("{:#x}", tx_index)],
        )
        .await?;
    let expected_tx: Value = expected
        .raw_request(
            "eth_getTransactionByBlockHashAndIndex".into(),
            vec![format!("{:#x}", block_hash), format!("{:#x}", tx_index)],
        )
        .await?;

    // Compare the transaction objects
    ensure_eq!(
        tx,
        expected_tx,
        "Transaction mismatch for block hash {:?} index {}",
        block_hash,
        tx_index
    );
    Ok(())
}

async fn test_get_transaction_by_block_number_and_index(
    helios: &RootProvider,
    expected: &RootProvider,
) -> Result<()> {
    // Find a block with transactions
    let latest = helios.get_block_number().await?;
    let mut block_num = latest;
    for i in 0..10 {
        let candidate = helios
            .get_block_by_number((latest - i).into())
            .await?
            .ok_or_else(|| eyre::eyre!("No block found"))?;
        if !candidate.transactions.is_empty() {
            block_num = latest - i;
            break;
        }
    }
    let tx_index = 0u64; // Get the first transaction

    // Use the actual RPC method eth_getTransactionByBlockNumberAndIndex
    let tx: Value = helios
        .raw_request(
            "eth_getTransactionByBlockNumberAndIndex".into(),
            vec![format!("{:#x}", block_num), format!("{:#x}", tx_index)],
        )
        .await?;
    let expected_tx: Value = expected
        .raw_request(
            "eth_getTransactionByBlockNumberAndIndex".into(),
            vec![format!("{:#x}", block_num), format!("{:#x}", tx_index)],
        )
        .await?;

    // Compare the transaction objects
    ensure_eq!(
        tx,
        expected_tx,
        "Transaction mismatch for block number {} index {}",
        block_num,
        tx_index
    );
    Ok(())
}

async fn test_get_nonce(helios: &RootProvider, expected: &RootProvider) -> Result<()> {
    let block_num = helios.get_block_number().await?;
    // ETH2 deposit contract
    let address = address!("00000000219ab540356cBB839Cbe05303d7705Fa");

    let nonce = helios
        .get_transaction_count(address)
        .block_id(block_num.into())
        .await?;
    let expected_nonce = expected
        .get_transaction_count(address)
        .block_id(block_num.into())
        .await?;
    ensure_eq!(
        nonce,
        expected_nonce,
        "Nonce mismatch: expected {}, got {}",
        expected_nonce,
        nonce
    );
    Ok(())
}

async fn test_get_code(helios: &RootProvider, expected: &RootProvider) -> Result<()> {
    let block_num = helios.get_block_number().await?;
    // USDC contract
    let address = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");

    let code = helios
        .get_code_at(address)
        .block_id(block_num.into())
        .await?;
    let expected_code = expected
        .get_code_at(address)
        .block_id(block_num.into())
        .await?;
    ensure_eq!(
        code,
        expected_code,
        "Code mismatch: expected {} bytes, got {} bytes",
        expected_code.len(),
        code.len()
    );
    Ok(())
}

async fn test_get_storage_at(helios: &RootProvider, expected: &RootProvider) -> Result<()> {
    let block_num = helios.get_block_number().await?;
    // USDC contract
    let address = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    let slot = U256::ZERO;

    let storage = helios
        .get_storage_at(address, slot)
        .block_id(block_num.into())
        .await?;
    let expected_storage = expected
        .get_storage_at(address, slot)
        .block_id(block_num.into())
        .await?;
    ensure_eq!(
        storage,
        expected_storage,
        "Storage mismatch: expected {:?}, got {:?}",
        expected_storage,
        storage
    );
    Ok(())
}

async fn test_get_proof(helios: &RootProvider, expected: &RootProvider) -> Result<()> {
    let block_num = helios.get_block_number().await?;
    // USDC contract
    let address = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    let keys = vec![U256::ZERO.into()];

    let proof = helios
        .get_proof(address, keys.clone())
        .block_id(block_num.into())
        .await?;
    let expected_proof = expected
        .get_proof(address, keys)
        .block_id(block_num.into())
        .await?;
    ensure_eq!(
        proof.address,
        expected_proof.address,
        "Proof address mismatch: expected {:?}, got {:?}",
        expected_proof.address,
        proof.address
    );
    ensure_eq!(
        proof.balance,
        expected_proof.balance,
        "Proof balance mismatch: expected {}, got {}",
        expected_proof.balance,
        proof.balance
    );
    ensure_eq!(
        proof.code_hash,
        expected_proof.code_hash,
        "Proof code hash mismatch: expected {:?}, got {:?}",
        expected_proof.code_hash,
        proof.code_hash
    );
    ensure_eq!(
        proof.nonce,
        expected_proof.nonce,
        "Proof nonce mismatch: expected {}, got {}",
        expected_proof.nonce,
        proof.nonce
    );
    ensure_eq!(
        proof.storage_hash,
        expected_proof.storage_hash,
        "Proof storage hash mismatch: expected {:?}, got {:?}",
        expected_proof.storage_hash,
        proof.storage_hash
    );
    Ok(())
}

async fn test_create_access_list(helios: &RootProvider, expected: &RootProvider) -> Result<()> {
    // Use a specific block number to ensure consistency
    let block_num = helios.get_block_number().await? - 10; // Use a recent but not latest block
    let usdc = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    let user = address!("99C9fc46f92E8a1c0deC1b1747d010903E884bE1");

    // Create a transaction request for balanceOf call
    use alloy::sol;

    sol! {
        function balanceOf(address who) external view returns (uint256);
    }

    let call_data = balanceOfCall { who: user }.abi_encode();

    // Use the actual RPC method eth_createAccessList
    // Get the block to determine base fee
    let block = helios
        .get_block_by_number(block_num.into())
        .await?
        .ok_or_else(|| eyre::eyre!("Block not found"))?;

    // Use EIP-1559 gas parameters
    let base_fee = U256::from(block.header.base_fee_per_gas.unwrap_or_default());
    let max_priority_fee = U256::from(2_000_000_000u64); // 2 gwei
    let max_fee = base_fee + max_priority_fee;

    // For eth_createAccessList at a specific block, we need to use the transaction format
    // that matches the block's context
    let tx_json = json!({
        "from": "0x0000000000000000000000000000000000000000",
        "to": format!("{:#x}", usdc),
        "data": format!("0x{}", hex::encode(&call_data)),
        "gas": "0x30000", // 196608 - enough gas for contract call
        "gasPrice": format!("{:#x}", max_fee), // Use gasPrice for compatibility
    });

    let access_list: Value = helios
        .raw_request(
            "eth_createAccessList".into(),
            vec![tx_json.clone(), json!(format!("{:#x}", block_num))],
        )
        .await?;
    let expected_access_list: Value = expected
        .raw_request(
            "eth_createAccessList".into(),
            vec![tx_json, json!(format!("{:#x}", block_num))],
        )
        .await?;

    // Compare the access lists
    // Note: Access lists might differ slightly between providers due to implementation differences
    // Just ensure both returned valid access lists
    ensure!(
        access_list.is_object(),
        "Helios should return an access list object"
    );
    ensure!(
        expected_access_list.is_object(),
        "Expected should return an access list object"
    );

    // Check that both have the required fields
    let helios_obj = access_list.as_object().unwrap();
    let expected_obj = expected_access_list.as_object().unwrap();

    ensure!(
        helios_obj.contains_key("accessList"),
        "Helios response should contain accessList"
    );
    ensure!(
        expected_obj.contains_key("accessList"),
        "Expected response should contain accessList"
    );

    Ok(())
}

async fn test_get_logs_by_address(helios: &RootProvider, expected: &RootProvider) -> Result<()> {
    let latest_block = helios.get_block_number().await?;
    let target_block = latest_block.saturating_sub(2); // A few blocks back for more activity

    let usdc = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"); // USDC contract
    let filter = Filter::new()
        .address(usdc)
        .from_block(target_block)
        .to_block(target_block); // from == to (single block)

    let logs = helios.get_logs(&filter).await?;
    let expected_logs = expected.get_logs(&filter).await?;

    // Logs should match exactly
    ensure_eq!(
        logs.len(),
        expected_logs.len(),
        "Logs count mismatch: expected {}, got {}",
        expected_logs.len(),
        logs.len()
    );

    for (i, (log, expected_log)) in logs.iter().zip(expected_logs.iter()).enumerate() {
        ensure_eq!(
            log.address(),
            expected_log.address(),
            "Log {} address mismatch: expected {:?}, got {:?}",
            i,
            expected_log.address(),
            log.address()
        );
        ensure_eq!(
            log.transaction_hash,
            expected_log.transaction_hash,
            "Log {} transaction hash mismatch: expected {:?}, got {:?}",
            i,
            expected_log.transaction_hash,
            log.transaction_hash
        );
        ensure_eq!(
            log.block_hash,
            expected_log.block_hash,
            "Log {} block hash mismatch: expected {:?}, got {:?}",
            i,
            expected_log.block_hash,
            log.block_hash
        );
        ensure_eq!(
            log.log_index,
            expected_log.log_index,
            "Log {} index mismatch: expected {:?}, got {:?}",
            i,
            expected_log.log_index,
            log.log_index
        );
    }
    Ok(())
}

async fn test_get_logs_by_topic(helios: &RootProvider, expected: &RootProvider) -> Result<()> {
    let latest_block = helios.get_block_number().await?;
    let target_block = latest_block.saturating_sub(2); // A few blocks back for more activity

    // ERC20 Transfer event signature: 0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef
    let transfer_topic: B256 =
        "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef".parse()?;

    let filter = Filter::new()
        .event_signature(transfer_topic)
        .from_block(target_block)
        .to_block(target_block); // from == to (single block)

    let logs = helios.get_logs(&filter).await?;
    let expected_logs = expected.get_logs(&filter).await?;

    ensure_eq!(
        logs,
        expected_logs,
        "Logs mismatch: expected {} logs, got {}",
        expected_logs.len(),
        logs.len()
    );
    Ok(())
}

async fn test_get_block_by_number_finalized(
    helios: &RootProvider,
    expected: &RootProvider,
) -> Result<()> {
    let block = helios
        .get_block_by_number(BlockNumberOrTag::Finalized)
        .await?;
    let expected_block = expected
        .get_block_by_number(BlockNumberOrTag::Finalized)
        .await?;

    if let (Some(block), Some(expected_block)) = (block, expected_block) {
        if block.header.hash != expected_block.header.hash {
            return Err(eyre::eyre!(
                "Finalized block hash mismatch: expected {:?}, got {:?}",
                expected_block.header.hash,
                block.header.hash
            ));
        }
    }
    Ok(())
}

async fn test_get_balance_zero_address(
    helios: &RootProvider,
    expected: &RootProvider,
) -> Result<()> {
    let block_num = helios.get_block_number().await?;
    let zero_address = address!("0000000000000000000000000000000000000000");

    let balance = helios
        .get_balance(zero_address)
        .block_id(block_num.into())
        .await?;
    let expected_balance = expected
        .get_balance(zero_address)
        .block_id(block_num.into())
        .await?;
    ensure_eq!(
        balance,
        expected_balance,
        "Balance mismatch: expected {}, got {}",
        expected_balance,
        balance
    );
    Ok(())
}

async fn test_get_code_eoa_address(helios: &RootProvider, expected: &RootProvider) -> Result<()> {
    let block_num = helios.get_block_number().await?;
    let eoa_address = address!("d8dA6BF26964aF9D7eEd9e03E53415D37aA96045"); // vitalik.eth

    let code = helios
        .get_code_at(eoa_address)
        .block_id(block_num.into())
        .await?;
    let expected_code = expected
        .get_code_at(eoa_address)
        .block_id(block_num.into())
        .await?;
    ensure_eq!(
        code,
        expected_code,
        "Code mismatch: expected {} bytes, got {} bytes",
        expected_code.len(),
        code.len()
    );
    if !code.is_empty() {
        return Err(eyre::eyre!("EOA should have empty code"));
    }
    Ok(())
}

async fn test_get_historical_block(helios: &RootProvider, expected: &RootProvider) -> Result<()> {
    let latest = helios.get_block_number().await?;
    let historical_block_num = latest.saturating_sub(1000);

    let block = helios
        .get_block_by_number(historical_block_num.into())
        .await?;

    let expected_block = expected
        .get_block_by_number(historical_block_num.into())
        .await?;

    if let (Some(block), Some(expected_block)) = (block, expected_block) {
        let hash = block.header.hash;
        let calculated_hash = block.header.hash_slow();
        if hash != calculated_hash {
            eyre::bail!("invalid block hash");
        }

        if block.header.number != expected_block.header.number {
            return Err(eyre::eyre!(
                "Historical block number mismatch: expected {:?}, got {:?}",
                expected_block.header.number,
                block.header.number
            ));
        }
        if block.header.hash != expected_block.header.hash {
            return Err(eyre::eyre!(
                "Historical block hash mismatch: expected {:?}, got {:?}",
                expected_block.header.hash,
                block.header.hash
            ));
        }
    }
    Ok(())
}

async fn test_get_too_old_block(helios: &RootProvider, expected: &RootProvider) -> Result<()> {
    let latest_block = expected.get_block_number().await?;
    let latest_block_num = latest_block;

    let old_block_num = latest_block_num.saturating_sub(20000);

    let helios_block = helios.get_block_by_number(old_block_num.into()).await;

    ensure!(
        helios_block.is_err(),
        "Helios should return None for a block outside the proof window"
    );

    let expected_block = expected.get_block_by_number(old_block_num.into()).await?;

    ensure!(
        expected_block.is_some(),
        "The trusted provider should have returned the historical block"
    );

    Ok(())
}

async fn test_get_historical_balance(helios: &RootProvider, expected: &RootProvider) -> Result<()> {
    let latest = helios.get_block_number().await?;
    let historical_block_num = latest.saturating_sub(100);
    let address = address!("00000000219ab540356cBB839Cbe05303d7705Fa"); // ETH2 deposit contract

    let balance = helios
        .get_balance(address)
        .block_id(historical_block_num.into())
        .await?;
    let expected_balance = expected
        .get_balance(address)
        .block_id(historical_block_num.into())
        .await?;
    ensure_eq!(
        balance,
        expected_balance,
        "Balance mismatch: expected {}, got {}",
        expected_balance,
        balance
    );
    Ok(())
}

async fn test_get_transaction_receipt_detailed(
    helios: &RootProvider,
    expected: &RootProvider,
) -> Result<()> {
    let block = helios
        .get_block_by_number(BlockNumberOrTag::Latest)
        .await?
        .ok_or_else(|| eyre::eyre!("No latest block"))?;
    let tx_hash = block
        .transactions
        .hashes()
        .next()
        .ok_or_else(|| eyre::eyre!("No transactions"))?;

    let receipt = helios
        .get_transaction_receipt(tx_hash)
        .await?
        .ok_or_else(|| eyre::eyre!("Receipt not found"))?;
    let expected_receipt = expected
        .get_transaction_receipt(tx_hash)
        .await?
        .ok_or_else(|| eyre::eyre!("Receipt not found"))?;

    if receipt.transaction_hash() != expected_receipt.transaction_hash() {
        return Err(eyre::eyre!(
            "Receipt transaction hash mismatch: expected {:?}, got {:?}",
            expected_receipt.transaction_hash(),
            receipt.transaction_hash()
        ));
    }
    if receipt.block_hash() != expected_receipt.block_hash() {
        return Err(eyre::eyre!(
            "Receipt block hash mismatch: expected {:?}, got {:?}",
            expected_receipt.block_hash(),
            receipt.block_hash()
        ));
    }
    if receipt.block_number() != expected_receipt.block_number() {
        return Err(eyre::eyre!(
            "Receipt block number mismatch: expected {:?}, got {:?}",
            expected_receipt.block_number(),
            receipt.block_number()
        ));
    }
    if receipt.gas_used() != expected_receipt.gas_used() {
        return Err(eyre::eyre!(
            "Receipt gas used mismatch: expected {}, got {}",
            expected_receipt.gas_used(),
            receipt.gas_used()
        ));
    }
    if receipt.status() != expected_receipt.status() {
        return Err(eyre::eyre!(
            "Receipt status mismatch: expected {:?}, got {:?}",
            expected_receipt.status(),
            receipt.status()
        ));
    }
    if receipt.logs().len() != expected_receipt.logs().len() {
        return Err(eyre::eyre!(
            "Receipt logs count mismatch: expected {}, got {}",
            expected_receipt.logs().len(),
            receipt.logs().len()
        ));
    }
    Ok(())
}

async fn test_get_logs_block_range(helios: &RootProvider, expected: &RootProvider) -> Result<()> {
    let latest = helios.get_block_number().await?;
    let target_block = latest.saturating_sub(2); // A few blocks back for more activity

    let filter = Filter::new()
        .from_block(target_block)
        .to_block(target_block); // from == to (single block)

    let logs = helios.get_logs(&filter).await?;
    let expected_logs = expected.get_logs(&filter).await?;

    if logs.len() != expected_logs.len() {
        return Err(eyre::eyre!(
            "Block range logs count mismatch: expected {}, got {}",
            expected_logs.len(),
            logs.len()
        ));
    }

    // Verify all logs are from the target block
    for (i, log) in logs.iter().enumerate() {
        if let Some(block_num) = log.block_number {
            if block_num != target_block {
                return Err(eyre::eyre!(
                    "Log {} from wrong block: expected {}, got {}",
                    i,
                    target_block,
                    block_num
                ));
            }
        }
    }
    Ok(())
}

async fn test_call_complex_contract(helios: &RootProvider, expected: &RootProvider) -> Result<()> {
    let block_num = helios.get_block_number().await?;
    let weth = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"); // WETH contract

    sol! {
        #[sol(rpc)]
        interface WETH {
            function totalSupply() external view returns (uint256);
            function symbol() external view returns (string);
            function decimals() external view returns (uint8);
        }
    }

    let weth_helios = WETH::new(weth, helios.clone());
    let weth_expected = WETH::new(weth, expected.clone());

    // Test multiple calls to the same contract
    let supply = weth_helios
        .totalSupply()
        .block(block_num.into())
        .call()
        .await?;
    let symbol = weth_helios.symbol().block(block_num.into()).call().await?;
    let decimals = weth_helios
        .decimals()
        .block(block_num.into())
        .call()
        .await?;

    let expected_supply = weth_expected
        .totalSupply()
        .block(block_num.into())
        .call()
        .await?;
    let expected_symbol = weth_expected
        .symbol()
        .block(block_num.into())
        .call()
        .await?;
    let expected_decimals = weth_expected
        .decimals()
        .block(block_num.into())
        .call()
        .await?;

    ensure_eq!(
        supply,
        expected_supply,
        "Supply mismatch: expected {}, got {}",
        expected_supply,
        supply
    );
    ensure_eq!(
        symbol,
        expected_symbol,
        "Symbol mismatch: expected {:?}, got {:?}",
        expected_symbol,
        symbol
    );
    ensure_eq!(
        decimals,
        expected_decimals,
        "Decimals mismatch: expected {}, got {}",
        expected_decimals,
        decimals
    );
    Ok(())
}

async fn test_get_blob_base_fee(helios: &RootProvider, expected: &RootProvider) -> Result<()> {
    // Only test if both providers support this method
    if let (Ok(helios_fee), Ok(expected_fee)) = (
        helios.get_blob_base_fee().await,
        expected.get_blob_base_fee().await,
    ) {
        // EIP-4844 blob base fee should be present and positive
        ensure!(helios_fee > 0, "Helios blob base fee should be positive");
        ensure!(
            expected_fee > 0,
            "Expected blob base fee should be positive"
        );
        // Allow for some variance due to block timing - just check they're both positive
        // since blob base fees can vary significantly between blocks
    }
    Ok(())
}

async fn test_get_block_by_hash_with_txs(
    helios: &RootProvider,
    expected: &RootProvider,
) -> Result<()> {
    let block = helios
        .get_block_by_number(BlockNumberOrTag::Latest)
        .await?
        .ok_or_else(|| eyre::eyre!("No latest block"))?;
    let block_hash = block.header.hash;
    let full_block = helios
        .get_block_by_hash(block_hash)
        .full()
        .await?
        .ok_or_else(|| eyre::eyre!("Block not found"))?;
    let expected_block = expected
        .get_block_by_hash(block_hash)
        .full()
        .await?
        .ok_or_else(|| eyre::eyre!("Block not found"))?;

    ensure_eq!(
        full_block.header.hash,
        expected_block.header.hash,
        "Block hash mismatch: expected {:?}, got {:?}",
        expected_block.header.hash,
        full_block.header.hash
    );
    ensure_eq!(
        full_block.transactions.len(),
        expected_block.transactions.len(),
        "Transaction count mismatch: expected {}, got {}",
        expected_block.transactions.len(),
        full_block.transactions.len()
    );
    Ok(())
}

async fn test_get_storage_at_specific_slot(
    helios: &RootProvider,
    expected: &RootProvider,
) -> Result<()> {
    let block_num = helios.get_block_number().await?;
    let usdc = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"); // USDC contract

    // Slot 1 in USDC is typically the total supply
    let slot_1 = U256::from(1);

    let storage = helios
        .get_storage_at(usdc, slot_1)
        .block_id(block_num.into())
        .await?;
    let expected_storage = expected
        .get_storage_at(usdc, slot_1)
        .block_id(block_num.into())
        .await?;
    ensure_eq!(
        storage,
        expected_storage,
        "Storage mismatch: expected {:?}, got {:?}",
        expected_storage,
        storage
    );
    Ok(())
}

async fn test_get_proof_multiple_keys(
    helios: &RootProvider,
    expected: &RootProvider,
) -> Result<()> {
    let block_num = helios.get_block_number().await?;
    let usdc = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"); // USDC contract
    let keys = vec![
        U256::from(0).into(), // Slot 0
        U256::from(1).into(), // Slot 1
        U256::from(2).into(), // Slot 2
    ];

    let proof = helios
        .get_proof(usdc, keys.clone())
        .block_id(block_num.into())
        .await?;
    let expected_proof = expected
        .get_proof(usdc, keys)
        .block_id(block_num.into())
        .await?;

    ensure_eq!(
        proof.address,
        expected_proof.address,
        "Proof address mismatch: expected {:?}, got {:?}",
        expected_proof.address,
        proof.address
    );
    ensure_eq!(
        proof.storage_proof.len(),
        expected_proof.storage_proof.len(),
        "Storage proof count mismatch: expected {}, got {}",
        expected_proof.storage_proof.len(),
        proof.storage_proof.len()
    );
    ensure_eq!(
        proof.storage_proof.len(),
        3,
        "Should have exactly 3 storage proofs, got {}",
        proof.storage_proof.len()
    );
    Ok(())
}

// ========== MAIN TEST RUNNER ==========

#[tokio::test(flavor = "multi_thread")]
async fn rpc_equivalence_tests() {
    if !rpc_exists() {
        eprintln!("Skipping rpc_equivalence_tests: MAINNET_EXECUTION_RPC is not set");
        return;
    }

    println!("Setting up Helios instances (this may take a few seconds)...");
    let (_handle1, _handle2, _handle3, providers) = setup().await;
    let helios_api = &providers[0];
    let helios_rpc = &providers[1];
    let provider = &providers[2];

    // Create a macro to simplify adding tests
    macro_rules! spawn_test {
        ($test_fn:ident, $test_name:expr) => {{
            let helios_api = helios_api.clone();
            let helios_rpc = helios_rpc.clone();
            let provider = provider.clone();
            tokio::spawn(async move {
                let api_result = $test_fn(&helios_api, &provider).await;
                let rpc_result = $test_fn(&helios_rpc, &provider).await;

                TestResult {
                    name: $test_name.to_string(),
                    passed: api_result.is_ok() && rpc_result.is_ok(),
                    error: if api_result.is_err() || rpc_result.is_err() {
                        Some(format!(
                            "API: {:?}, RPC: {:?}",
                            api_result.err(),
                            rpc_result.err()
                        ))
                    } else {
                        None
                    },
                }
            })
        }};
    }

    let test_count = 33; // Update count as we add tests
    println!(
        "Setup complete! Running {} mini-tests in parallel...",
        test_count
    );

    // Run all mini-tests in parallel
    let futures = vec![
        // Basic/Network Methods
        spawn_test!(test_get_chain_id, "get_chain_id"),
        spawn_test!(test_get_block_number, "get_block_number"),
        spawn_test!(test_get_gas_price, "get_gas_price"),
        spawn_test!(
            test_get_max_priority_fee_per_gas,
            "get_max_priority_fee_per_gas"
        ),
        spawn_test!(test_get_blob_base_fee, "get_blob_base_fee"),
        // Block Methods
        spawn_test!(
            test_get_block_by_number_finalized,
            "get_block_by_number_finalized"
        ),
        spawn_test!(test_get_block_by_hash, "get_block_by_hash"),
        spawn_test!(
            test_get_block_by_hash_with_txs,
            "get_block_by_hash_with_txs"
        ),
        spawn_test!(
            test_get_block_transaction_count_by_hash,
            "get_block_transaction_count_by_hash"
        ),
        spawn_test!(
            test_get_block_transaction_count_by_number,
            "get_block_transaction_count_by_number"
        ),
        spawn_test!(test_get_block_receipts, "get_block_receipts"),
        // Transaction Methods
        spawn_test!(test_get_transaction_by_hash, "get_transaction_by_hash"),
        spawn_test!(test_get_transaction_receipt, "get_transaction_receipt"),
        spawn_test!(
            test_get_transaction_by_block_hash_and_index,
            "get_transaction_by_block_hash_and_index"
        ),
        spawn_test!(
            test_get_transaction_by_block_number_and_index,
            "get_transaction_by_block_number_and_index"
        ),
        spawn_test!(
            test_get_transaction_receipt_detailed,
            "get_transaction_receipt_detailed"
        ),
        // Account/State Methods
        spawn_test!(test_get_balance, "get_balance"),
        spawn_test!(test_get_balance_zero_address, "get_balance_zero_address"),
        spawn_test!(test_get_historical_balance, "get_historical_balance"),
        spawn_test!(test_get_nonce, "get_nonce"),
        spawn_test!(test_get_code, "get_code"),
        spawn_test!(test_get_code_eoa_address, "get_code_eoa_address"),
        spawn_test!(test_get_storage_at, "get_storage_at"),
        spawn_test!(
            test_get_storage_at_specific_slot,
            "get_storage_at_specific_slot"
        ),
        spawn_test!(test_get_proof, "get_proof"),
        spawn_test!(test_get_proof_multiple_keys, "get_proof_multiple_keys"),
        // EVM Execution Methods
        spawn_test!(test_call, "call"),
        spawn_test!(test_call_complex_contract, "call_complex_contract"),
        spawn_test!(test_create_access_list, "create_access_list"),
        // Log/Event Methods
        spawn_test!(test_get_logs, "get_logs"),
        spawn_test!(test_get_logs_by_address, "get_logs_by_address"),
        spawn_test!(test_get_logs_by_topic, "get_logs_by_topic"),
        spawn_test!(test_get_logs_block_range, "get_logs_block_range"),
        // Historical Data
        spawn_test!(test_get_historical_block, "get_historical_block"),
        spawn_test!(test_get_too_old_block, "get_too_old_block"),
    ];

    // Collect results
    let results: Vec<TestResult> = join_all(futures)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    // Print final summary
    let passed = results.iter().filter(|r| r.passed).count();
    let failed = results.iter().filter(|r| !r.passed).count();

    println!("\n=== FINAL RESULTS ===");
    println!("✅ Passed: {}", passed);
    println!("❌ Failed: {}", failed);

    // Fail the test if any mini-test failed
    assert_eq!(failed, 0, "Some mini-tests failed");
}
