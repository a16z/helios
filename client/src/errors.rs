use eyre::Report;
use thiserror::Error;

use common::errors::BlockNotFoundError;
use execution::errors::EvmError;

/// Errors that can occur during Node calls
#[derive(Debug, Error)]
pub enum NodeError {
    #[error(transparent)]
    ExecutionEvmError(#[from] EvmError),

    #[error("execution error: {0}")]
    ExecutionError(Report),

    #[error("out of sync: {0} seconds behind")]
    OutOfSync(u64),

    #[error("consensus payload error: {0}")]
    ConsensusPayloadError(Report),

    #[error("execution payload error: {0}")]
    ExecutionPayloadError(Report),

    #[error("consensus client creation error: {0}")]
    ConsensusClientCreationError(Report),

    #[error("execution client creation error: {0}")]
    ExecutionClientCreationError(Report),

    #[error("consensus advance error: {0}")]
    ConsensusAdvanceError(Report),

    #[error("consensus sync error: {0}")]
    ConsensusSyncError(Report),

    #[error(transparent)]
    BlockNotFoundError(#[from] BlockNotFoundError),
}

#[cfg(not(target_arch = "wasm32"))]
impl NodeError {
    pub fn to_json_rpsee_error(self) -> jsonrpsee::types::error::ErrorObjectOwned {
        match self {
            NodeError::ExecutionEvmError(evm_err) => match evm_err {
                EvmError::Revert(data) => {
                    let mut msg = "execution reverted".to_string();
                    if let Some(reason) = data.as_ref().and_then(EvmError::decode_revert_reason) {
                        msg = format!("{msg}: {reason}")
                    }
                    jsonrpsee::types::error::ErrorObject::owned(
                        3,
                        msg,
                        data.map(|data| format!("0x{}", hex::encode(data))),
                    )
                }
                _ => {
                    jsonrpsee::types::error::ErrorObject::owned(1, evm_err.to_string(), None::<()>)
                }
            },
            _ => jsonrpsee::types::error::ErrorObject::owned(1, self.to_string(), None::<()>),
        }
    }
}
