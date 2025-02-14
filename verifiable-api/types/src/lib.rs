use std::collections::HashMap;

use alloy::{
    consensus::Account as TrieAccount,
    eips::BlockId,
    primitives::{Address, Bytes, B256},
    rpc::types::{EIP1186StorageProof, Log},
};
use serde::{Deserialize, Serialize};

use helios_common::network_spec::NetworkSpec;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountResponse {
    pub account: TrieAccount,
    pub code: Bytes,
    pub account_proof: Vec<Bytes>,
    pub storage_proof: Vec<EIP1186StorageProof>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageAtResponse {
    pub storage: EIP1186StorageProof,
    pub account: TrieAccount,
    pub account_proof: Vec<Bytes>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionReceiptResponse<N: NetworkSpec> {
    pub receipt: N::ReceiptResponse,
    pub receipt_proof: Vec<Bytes>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "N: NetworkSpec")]
pub struct LogsResponse<N: NetworkSpec> {
    pub logs: Vec<Log>,
    pub receipt_proofs: HashMap<B256, TransactionReceiptResponse<N>>, // tx_hash -> receipt & proof
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "N: NetworkSpec")]
pub struct FilterLogsResponse<N: NetworkSpec> {
    pub logs: Vec<Log>,
    pub receipt_proofs: HashMap<B256, TransactionReceiptResponse<N>>, // tx_hash -> receipt & proof
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "N: NetworkSpec")]
#[serde(untagged)]
pub enum FilterChangesResponse<N: NetworkSpec> {
    Hashes(Vec<B256>),
    Logs(FilterLogsResponse<N>),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessListRequest<N: NetworkSpec> {
    pub tx: N::TransactionRequest,
    pub block: Option<BlockId>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccessListResponse {
    pub accounts: HashMap<Address, AccountResponse>,
}
