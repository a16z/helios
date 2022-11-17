use ethers::types::{Address, H256, U256};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("invalid account proof for address: {0}")]
    InvalidAccountProof(Address),
    #[error("invalid storage proof for address: {0}, slot: {1}")]
    InvalidStorageProof(Address, H256),
    #[error("code hash mismatch for address: {0}, found: {1}, expected: {2}")]
    CodeHashMismatch(Address, String, String),
    #[error("receipt root mismatch for tx: {0}")]
    ReceiptRootMismatch(String),
    #[error("missing transaction for tx: {0}")]
    MissingTransaction(String),
    #[error("could not prove receipt for tx: {0}")]
    NoReceiptForTransaction(String),
    #[error("missing log for transaction: {0}, index: {1}")]
    MissingLog(String, U256),
    #[error("too many logs to prove: {0}, current limit is: {1}")]
    TooManyLogsToProve(usize, usize),
}
