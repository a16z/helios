use alloy::eips::BlockId;
use alloy::primitives::{Address, B256, U256};

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
    #[error("could not prove receipts for block: {0}")]
    NoReceiptsForBlock(BlockId),
    #[error("missing log for transaction: {0}, index: {1}")]
    MissingLog(B256, U256),
    #[error("too many logs to prove: {0} spanning {1} blocks current limit is: {2} blocks")]
    TooManyLogsToProve(usize, usize, usize),
    #[error("transaction {0} is not included in a block")]
    TransactionNotIncluded(B256),
    #[error("execution rpc is for the incorrect network")]
    IncorrectRpcNetwork(),
    #[error("block not found: {0}")]
    BlockNotFound(BlockId),
    #[error("receipts root mismatch for block: {0}")]
    BlockReceiptsRootMismatch(BlockId),
    #[error("filter not found: 0x{0:x}")]
    FilterNotFound(U256),
    #[error("log does not match filter")]
    LogFilterMismatch(),
}

impl ExecutionError {
    /// Returns `true` if this error means data served by the execution RPC did not verify
    /// against the proven state, as opposed to the data being unavailable.
    pub fn is_verification_failure(&self) -> bool {
        matches!(
            self,
            Self::InvalidAccountProof(_)
                | Self::InvalidStorageProof(_, _)
                | Self::CodeHashMismatch(_, _, _)
        )
    }
}

/// Returns `true` if `err`, or any error in its source chain, is an [`ExecutionError`] that
/// reports a proof verification failure (see [`ExecutionError::is_verification_failure`]).
pub fn is_verification_failure(err: &eyre::Report) -> bool {
    err.chain().any(|cause| {
        cause
            .downcast_ref::<ExecutionError>()
            .is_some_and(ExecutionError::is_verification_failure)
    })
}

#[cfg(test)]
mod tests {
    use eyre::{eyre, WrapErr};

    use super::*;

    #[test]
    fn detects_verification_failures() {
        let address = Address::repeat_byte(0x11);
        let slot = B256::repeat_byte(0x22);

        assert!(is_verification_failure(&eyre!(
            ExecutionError::InvalidAccountProof(address)
        )));
        assert!(is_verification_failure(
            &ExecutionError::InvalidStorageProof(address, slot).into()
        ));
        assert!(is_verification_failure(
            &ExecutionError::CodeHashMismatch(address, B256::ZERO, slot).into()
        ));

        // Still detected when wrapped with context.
        let wrapped: eyre::Result<()> =
            Err(ExecutionError::InvalidAccountProof(address)).wrap_err("prefetch failed");
        assert!(is_verification_failure(&wrapped.unwrap_err()));
    }

    #[test]
    fn ignores_other_errors() {
        assert!(!is_verification_failure(&eyre!("request timed out")));
        assert!(!is_verification_failure(
            &ExecutionError::BlockNotFound(BlockId::latest()).into()
        ));
    }
}
