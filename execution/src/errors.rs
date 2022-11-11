use ethers::{types::{Address, H256}, abi::AbiDecode};
use thiserror::Error;
use eyre::Report;
use bytes::Bytes;

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

/// Errors that can occur during evm.rs calls
#[derive(Debug, Error)]
pub enum EvmError {
    #[error("execution reverted: {0:?}")]
    Revert(Option<Bytes>),

    #[error("evm error: {0:?}")]
    Generic (String),

    // Casting existing errors
    #[error(transparent)]
    Eyre(#[from] Report)
}

impl EvmError {
    pub fn decode_revert_reason(data: impl AsRef<[u8]>) -> Option<String> {
        let data = data.as_ref();

        // skip function selector
        if data.len() < 4 {
            return None;
        }
        String::decode(&data[4..]).ok()
    }
}
