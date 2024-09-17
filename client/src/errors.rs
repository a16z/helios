use thiserror_no_std::Error;

use common::errors::BlockNotFoundError;
use execution::errors::EvmError;

/// Errors that can occur during Node calls
#[derive(Debug, Error)]
pub enum NodeError {
    #[error(transparent)]
    ExecutionEvmError(#[from] EvmError),

    #[error("execution error: {0}")]
    ExecutionError(anyhow::Error),

    #[error("out of sync: {0} seconds behind")]
    OutOfSync(u64),

    #[error("consensus payload error: {0}")]
    ConsensusPayloadError(anyhow::Error),

    #[error("execution payload error: {0}")]
    ExecutionPayloadError(anyhow::Error),

    #[error("consensus client creation error: {0}")]
    ConsensusClientCreationError(anyhow::Error),

    #[error("execution client creation error: {0}")]
    ExecutionClientCreationError(anyhow::Error),

    #[error("consensus advance error: {0}")]
    ConsensusAdvanceError(anyhow::Error),

    #[error("consensus sync error: {0}")]
    ConsensusSyncError(anyhow::Error),

    #[error(transparent)]
    BlockNotFoundError(#[from] BlockNotFoundError),
}

#[cfg(not(target_arch = "wasm32"))]
impl NodeError {
    pub fn to_json_rpsee_error(self) -> jsonrpsee::core::Error {
        match self {
            NodeError::ExecutionEvmError(evm_err) => match evm_err {
                EvmError::Revert(data) => {
                    let mut msg = "execution reverted".to_string();
                    if let Some(reason) = data.as_ref().and_then(EvmError::decode_revert_reason) {
                        msg = format!("{msg}: {reason}")
                    }
                    jsonrpsee::core::Error::Call(jsonrpsee::types::error::CallError::Custom(
                        jsonrpsee::types::error::ErrorObject::owned(
                            3,
                            msg,
                            data.map(|data| format!("0x{}", hex::encode(data))),
                        ),
                    ))
                }
                _ => jsonrpsee::core::Error::Custom(evm_err.to_string()),
            },
            _ => jsonrpsee::core::Error::Custom(self.to_string()),
        }
    }
}

impl From<NodeError> for anyhow::Error {
    fn from(error: NodeError) -> Self {
        anyhow::Error::msg(error.to_string())
    }
}
