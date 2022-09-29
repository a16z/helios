use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExecutionError {
    #[error("invalid account proof")]
    InvalidAccountProof,
    #[error("invalid storage proof")]
    InvalidStorageProof,
    #[error("code hash mismatch")]
    CodeHashMismatch,
    #[error("receipt root mismatch")]
    ReceiptRootMismatch,
    #[error("missing transaction")]
    MissingTransaction,
}
