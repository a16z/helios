use alloy::primitives::{Address, Bytes, B256, U256};
use alloy::sol_types::decode_revert_reason;
use eyre::Report;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("invalid account proof for address: {0}")]
    InvalidAccountProof(Address),
    #[error("invalid storage proof for address: {0}, slot: {1}")]
    InvalidStorageProof(Address, B256),
    #[error("code hash mismatch for address: {0}, found: {1}, expected: {2}")]
    CodeHashMismatch(Address, B256, B256),
    #[error("receipt root mismatch for tx: {0}")]
    ReceiptRootMismatch(B256),
    #[error("could not prove receipt for tx: {0}")]
    NoReceiptForTransaction(B256),
    #[error("missing log for transaction: {0}, index: {1}")]
    MissingLog(B256, U256),
    #[error("too many logs to prove: {0}, current limit is: {1}")]
    TooManyLogsToProve(usize, usize),
    #[error("execution rpc is for the incorrect network")]
    IncorrectRpcNetwork(),
}

/// Errors that can occur during evm.rs calls
#[derive(Debug, Error)]
pub enum EvmError {
    #[error("execution reverted: {0:?}")]
    Revert(Option<Bytes>),

    #[error("evm error: {0:?}")]
    Generic(String),

    #[error("rpc error: {0:?}")]
    RpcError(Report),
}

impl EvmError {
    pub fn decode_revert_reason(data: impl AsRef<[u8]>) -> Option<String> {
        decode_revert_reason(data.as_ref())
    }
}
