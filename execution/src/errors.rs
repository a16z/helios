use ethers::types::{Address, H256};
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
}
