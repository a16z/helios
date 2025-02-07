use std::collections::HashMap;

use alloy::{
    consensus::Account as TrieAccount,
    primitives::{Address, Bytes, B256},
    rpc::types::{EIP1186AccountProofResponse, EIP1186StorageProof, Log},
};
use serde::{Deserialize, Serialize};

use helios_core::{execution::types::Account, network_spec::NetworkSpec};

pub type GetAccountProofResponse = EIP1186AccountProofResponse;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetBalanceResponse {
    pub account: TrieAccount,
    pub account_proof: Vec<Bytes>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTransactionCountResponse {
    pub account: TrieAccount,
    pub account_proof: Vec<Bytes>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetCodeResponse {
    pub code: Bytes,
    pub account: TrieAccount, // ToDo(@eshaan7): Remove `code_hash` from account here
    pub account_proof: Vec<Bytes>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetStorageAtResponse {
    pub storage: EIP1186StorageProof,
    pub account: TrieAccount,
    pub account_proof: Vec<Bytes>,
}

#[allow(type_alias_bounds)]
pub type GetBlockReceiptsResponse<N: NetworkSpec> = Vec<N::ReceiptResponse>;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetTransactionReceiptResponse<N: NetworkSpec> {
    pub receipt: N::ReceiptResponse,
    pub receipt_proof: Vec<Bytes>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "N: NetworkSpec")]
pub struct GetLogsResponse<N: NetworkSpec> {
    pub receipts: Vec<GetTransactionReceiptResponse<N>>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "N: NetworkSpec")]
pub struct GetFilterLogsResponse<N: NetworkSpec> {
    pub logs: Vec<Log>,
    pub receipt_proofs: HashMap<B256, GetTransactionReceiptResponse<N>>, // tx_hash -> receipt & proof
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "N: NetworkSpec")]
#[serde(untagged)]

pub enum GetFilterChangesResponse<N: NetworkSpec> {
    Hashes(Vec<B256>),
    Logs(GetFilterLogsResponse<N>),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallResponse {
    pub accounts: HashMap<Address, Account>,
}
