use alloy::primitives::{Address, Bytes, B256, U256};
use alloy::sol_types::decode_revert_reason;
use eyre::Report;
use thiserror::Error;

use helios_common::types::BlockTag;

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
    #[error("could not prove receipts for block: {0}")]
    NoReceiptsForBlock(BlockTag),
    #[error("missing log for transaction: {0}, index: {1}")]
    MissingLog(B256, U256),
    #[error("too many logs to prove: {0} spanning {1} blocks current limit is: {2} blocks")]
    TooManyLogsToProve(usize, usize, usize),
    #[error("execution rpc is for the incorrect network")]
    IncorrectRpcNetwork(),
    #[error("block not found: {0}")]
    BlockNotFound(BlockTag),
    #[error("receipts root mismatch for block: {0}")]
    BlockReceiptsRootMismatch(BlockTag),
    #[error("filter not found: 0x{0:x}")]
    FilterNotFound(U256),
    #[error("log does not match filter")]
    LogFilterMismatch(),
}

/// Errors that can occur during evm.rs calls
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
