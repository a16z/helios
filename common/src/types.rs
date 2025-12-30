use alloy::{
    consensus::TrieAccount,
    primitives::{Bytes, B256, U256},
    rpc::types::EIP1186StorageProof,
    sol_types::decode_revert_reason,
};
use eyre::Report;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::broadcast::Receiver;

use crate::network_spec::NetworkSpec;

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub account: TrieAccount,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<Bytes>,
    pub account_proof: Vec<Bytes>,
    pub storage_proof: Vec<EIP1186StorageProof>,
}

impl Account {
    /// Retrieve the value at the given storage slot.
    pub fn get_storage_value(&self, slot: B256) -> Option<U256> {
        self.storage_proof
            .iter()
            .find_map(|EIP1186StorageProof { key, value, .. }| {
                if key.as_b256() == slot {
                    Some(*value)
                } else {
                    None
                }
            })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SubscriptionType {
    NewHeads,
    NewPendingTransactions,
    Logs,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum SubscriptionEvent<N: NetworkSpec> {
    NewHeads(N::BlockResponse),
}

pub type SubEventRx<N> = Receiver<SubscriptionEvent<N>>;

#[derive(Debug, Error)]
pub enum EvmError {
    #[error("execution reverted: {}", display_revert(.0))]
    Revert(Option<Bytes>),

    #[error("evm error: {0:?}")]
    Generic(String),

    #[error("rpc error: {0:?}")]
    RpcError(Report),
}

fn display_revert(output: &Option<Bytes>) -> String {
    match output {
        Some(bytes) => decode_revert_reason(bytes.as_ref()).unwrap_or(hex::encode(bytes)),
        None => "execution halted".to_string(),
    }
}
