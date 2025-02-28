use std::{fs::read_to_string, path::PathBuf, str::FromStr};

use alloy::{
    network::Network,
    primitives::{B256, U256},
    rpc::types::{EIP1186AccountProofResponse, Log, TransactionReceipt},
};

use helios_common::types::Account;
use helios_ethereum::spec::Ethereum as EthereumSpec;
use helios_verifiable_api_types::{
    AccessListResponse, AccountResponse, FilterLogsResponse, TransactionReceiptResponse,
};

pub fn testdata_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/testdata/")
}

pub fn rpc_chain_id() -> u64 {
    let json_str = read_to_string(testdata_dir().join("rpc/chain_id.txt")).unwrap();
    u64::from_str(&json_str).unwrap()
}

pub fn rpc_proof() -> EIP1186AccountProofResponse {
    let json_str = read_to_string(testdata_dir().join("rpc/proof.json")).unwrap();
    serde_json::from_str(&json_str).unwrap()
}

pub fn rpc_account() -> Account {
    let json_str = read_to_string(testdata_dir().join("rpc/account.json")).unwrap();
    serde_json::from_str(&json_str).unwrap()
}

pub fn rpc_block_miner_account() -> Account {
    let json_str = read_to_string(testdata_dir().join("rpc/block_miner_account.json")).unwrap();
    serde_json::from_str(&json_str).unwrap()
}

pub fn rpc_tx() -> <EthereumSpec as Network>::TransactionResponse {
    let json_str = read_to_string(testdata_dir().join("rpc/transaction.json")).unwrap();
    serde_json::from_str(&json_str).unwrap()
}

pub fn rpc_tx_receipt() -> TransactionReceipt {
    let json_str = read_to_string(testdata_dir().join("rpc/receipt.json")).unwrap();
    serde_json::from_str(&json_str).unwrap()
}

pub fn rpc_block() -> <EthereumSpec as Network>::BlockResponse {
    let json_str = read_to_string(testdata_dir().join("rpc/block.json")).unwrap();
    serde_json::from_str(&json_str).unwrap()
}

pub fn rpc_block_receipts() -> Vec<TransactionReceipt> {
    let json_str = read_to_string(testdata_dir().join("rpc/receipts.json")).unwrap();
    serde_json::from_str(&json_str).unwrap()
}

pub fn rpc_logs() -> Vec<Log> {
    let json_str = read_to_string(testdata_dir().join("rpc/logs.json")).unwrap();
    serde_json::from_str(&json_str).unwrap()
}

pub fn rpc_filter_block_hashes() -> Vec<B256> {
    let json_str = read_to_string(testdata_dir().join("rpc/block_hashes.json")).unwrap();
    serde_json::from_str(&json_str).unwrap()
}

pub fn rpc_filter_tx_hashes() -> Vec<B256> {
    let json_str = read_to_string(testdata_dir().join("rpc/tx_hashes.json")).unwrap();
    serde_json::from_str(&json_str).unwrap()
}

pub fn rpc_filter_id_logs() -> U256 {
    let id = read_to_string(testdata_dir().join("rpc/filter_id_logs.txt")).unwrap();
    U256::from_str(&id).unwrap()
}

pub fn rpc_filter_id_blocks() -> U256 {
    let id = read_to_string(testdata_dir().join("rpc/filter_id_blocks.txt")).unwrap();
    U256::from_str(&id).unwrap()
}

pub fn rpc_filter_id_txs() -> U256 {
    let id = read_to_string(testdata_dir().join("rpc/filter_id_txs.txt")).unwrap();
    U256::from_str(&id).unwrap()
}

pub fn verifiable_api_access_list_response() -> AccessListResponse {
    let resp = read_to_string(testdata_dir().join("verifiable_api/access_list.json")).unwrap();
    serde_json::from_str(&resp).unwrap()
}

pub fn verifiable_api_account_response() -> AccountResponse {
    let resp = read_to_string(testdata_dir().join("verifiable_api/account.json")).unwrap();
    serde_json::from_str(&resp).unwrap()
}

pub fn verifiable_api_block_miner_account_response() -> AccountResponse {
    let resp =
        read_to_string(testdata_dir().join("verifiable_api/block_miner_account.json")).unwrap();
    serde_json::from_str(&resp).unwrap()
}

pub fn verifiable_api_tx_receipt_response() -> TransactionReceiptResponse<EthereumSpec> {
    let resp = read_to_string(testdata_dir().join("verifiable_api/receipt.json")).unwrap();
    serde_json::from_str(&resp).unwrap()
}

pub fn verifiable_api_logs_response() -> FilterLogsResponse<EthereumSpec> {
    let resp = read_to_string(testdata_dir().join("verifiable_api/logs.json")).unwrap();
    serde_json::from_str(&resp).unwrap()
}
