use std::collections::HashMap;

use alloy::{
    eips::BlockId,
    primitives::{Address, Bytes, B256, U256},
    rpc::types::{Filter, Log},
};
use helios_common::{network_spec::NetworkSpec, types::Account};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub type AccountResponse = Account;

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
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
pub struct ExtendedAccessListRequest<N: NetworkSpec> {
    pub tx: N::TransactionRequest,
    pub validate_tx: bool,
    pub block: Option<BlockId>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtendedAccessListResponse {
    pub accounts: HashMap<Address, Account>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainIdResponse {
    pub chain_id: u64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendRawTxRequest {
    pub bytes: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendRawTxResponse {
    pub hash: B256,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub enum FilterKind {
    Logs,
    NewBlocks,
    NewPendingTransactions,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewFilterRequest {
    pub kind: FilterKind,
    pub filter: Option<Filter>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewFilterResponse {
    pub id: U256,
    pub kind: FilterKind,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallFilterResponse {
    pub ok: bool,
}
