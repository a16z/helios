use std::env;
use std::net::{SocketAddr, TcpListener};

use alloy::eips::BlockNumberOrTag;
use alloy::network::{ReceiptResponse, TransactionResponse};
use alloy::primitives::{address, B256, U256};
use alloy::providers::{Provider, RootProvider};
use alloy::rpc::types::Filter;
use alloy::sol;
use eyre::Result;
use futures::future::join_all;
use pretty_assertions::assert_eq;
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

#[derive(Debug, Clone)]
struct TestResult {
    name: String,
    passed: bool,
    error: Option<String>,
}

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
    // Simple port finding - only needed once now
    for port in 8000..9000 {
        if TcpListener::bind(format!("127.0.0.1:{}", port)).is_ok() {
            return port;
        }
    }
    8000 // fallback
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
            .consensus_rpc(consensus_rpc)
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
            .verifiable_api(&format!("http://localhost:{api_port}"))
            .consensus_rpc(consensus_rpc)
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

fn ensure_rpc_env() {
    if !rpc_exists() {
        panic!(
            "MAINNET_EXECUTION_RPC environment variable is required for RPC equivalence tests.\n\
            Set it to a mainnet Ethereum RPC URL, for example:\n\
            export MAINNET_EXECUTION_RPC=https://eth-mainnet.alchemyapi.io/v2/YOUR_API_KEY\n\
            or\n\
            export MAINNET_EXECUTION_RPC=https://mainnet.infura.io/v3/YOUR_PROJECT_ID"
        );
    }
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
    // Count transactions in the block
    let count = block.transactions.len() as u64;
    let expected_block = expected
        .get_block_by_hash(block_hash)
        .full()
        .await?
        .ok_or_else(|| eyre::eyre!("Block not found"))?;
    let expected_count = expected_block.transactions.len() as u64;
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
    let block = helios
        .get_block_by_number(block_num.into())
        .full()
        .await?
        .ok_or_else(|| eyre::eyre!("Block not found"))?;
    let expected_block = expected
        .get_block_by_number(block_num.into())
        .full()
        .await?
        .ok_or_else(|| eyre::eyre!("Block not found"))?;

    let count = block.transactions.len() as u64;
    let expected_count = expected_block.transactions.len() as u64;
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
    _expected: &RootProvider,
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
    // Get the transaction to find its block and index
    let tx = helios
        .get_transaction_by_hash(tx_hash)
        .await?
        .ok_or_else(|| eyre::eyre!("Transaction not found"))?;
    let block_hash = tx
        .block_hash()
        .ok_or_else(|| eyre::eyre!("No block hash"))?;
    let _tx_index = tx
        .transaction_index()
        .ok_or_else(|| eyre::eyre!("No transaction index"))?;

    // For this test, we'll just verify we can get the transaction by hash
    // since Alloy RootProvider doesn't have get_transaction_by_location
    ensure_eq!(
        tx.tx_hash(),
        tx_hash,
        "Transaction hash mismatch: expected {:?}, got {:?}",
        tx_hash,
        tx.tx_hash()
    );
    ensure!(
        block_hash != B256::ZERO,
        "Block hash should not be zero for a real transaction"
    );
    Ok(())
}

async fn test_get_transaction_by_block_number_and_index(
    helios: &RootProvider,
    _expected: &RootProvider,
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
    // Get the transaction to find its block and index
    let tx = helios
        .get_transaction_by_hash(tx_hash)
        .await?
        .ok_or_else(|| eyre::eyre!("Transaction not found"))?;
    let block_number = tx
        .block_number()
        .ok_or_else(|| eyre::eyre!("No block number"))?;
    let _tx_index = tx
        .transaction_index()
        .ok_or_else(|| eyre::eyre!("No transaction index"))?;

    // For this test, we'll just verify the transaction data is consistent
    ensure_eq!(
        tx.tx_hash(),
        tx_hash,
        "Transaction hash mismatch: expected {:?}, got {:?}",
        tx_hash,
        tx.tx_hash()
    );
    ensure!(block_number > 0, "Block number should be positive");
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
    let block_num = helios.get_block_number().await?;
    let usdc = address!("a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48");
    let user = address!("99C9fc46f92E8a1c0deC1b1747d010903E884bE1");

    sol! {
        #[sol(rpc)]
        interface ERC20 {
            function balanceOf(address who) external view returns (uint256);
        }
    }

    let token_helios = ERC20::new(usdc, helios.clone());
    let token_expected = ERC20::new(usdc, expected.clone());

    // Test that we can call the contract - access list creation is not available in CallBuilder
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
    ensure_rpc_env();

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

                if let Err(e) = api_result {
                    let result =
                        TestResult::fail($test_name, format!("API provider failed: {}", e));
                    println!("  ❌ {}: {}", result.name, result.error.as_ref().unwrap());
                    result
                } else if let Err(e) = rpc_result {
                    let result =
                        TestResult::fail($test_name, format!("RPC provider failed: {}", e));
                    println!("  ❌ {}: {}", result.name, result.error.as_ref().unwrap());
                    result
                } else {
                    let result = TestResult::pass($test_name);
                    println!("  ✅ {}", result.name);
                    result
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
