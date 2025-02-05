use std::collections::HashMap;

use alloy::{
    consensus::Account as TrieAccount,
    primitives::{Address, Bytes, B256},
    rpc::types::{EIP1186StorageProof, Log},
};
use serde::{Deserialize, Serialize};

use helios_core::{execution::types::Account, network_spec::NetworkSpec};

#[derive(Serialize, Deserialize)]
pub struct GetBalanceResponse {
    pub account: TrieAccount,
    pub account_proof: Vec<Bytes>,
}

#[derive(Serialize, Deserialize)]
pub struct GetTransactionCountResponse {
    pub account: TrieAccount,
    pub account_proof: Vec<Bytes>,
}

#[derive(Serialize, Deserialize)]
pub struct GetCodeResponse {
    pub code: Bytes,
    pub account: TrieAccount, // ToDo(@eshaan7): Remove `code_hash` from account here
    pub account_proof: Vec<Bytes>,
}

#[derive(Serialize, Deserialize)]
pub struct GetStorageAtResponse {
    pub storage: EIP1186StorageProof,
    pub account: TrieAccount,
    pub account_proof: Vec<Bytes>,
}

#[derive(Serialize, Deserialize)]
pub struct GetTransactionReceiptResponse<N: NetworkSpec> {
    pub receipt: N::ReceiptResponse,
    pub receipt_proof: Vec<Bytes>,
}

#[derive(Serialize, Deserialize)]
#[serde(bound = "N: NetworkSpec")]
pub struct GetLogsResponse<N: NetworkSpec> {
    pub receipt_proofs: HashMap<B256, GetTransactionReceiptResponse<N>>, // tx_hash -> receipt
}

#[derive(Serialize, Deserialize)]
#[serde(bound = "N: NetworkSpec")]
pub struct GetFilterLogsResponse<N: NetworkSpec> {
    pub logs: Vec<Log>,
    pub receipt_proofs: HashMap<B256, GetTransactionReceiptResponse<N>>, // tx_hash -> receipt
}

#[derive(Serialize, Deserialize)]
pub struct CallResponse {
    pub accounts: HashMap<Address, Account>,
}
