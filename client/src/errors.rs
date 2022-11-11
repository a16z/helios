use execution::errors::EvmError;
use jsonrpsee::tracing::info;
use thiserror::Error;
use eyre::Report;

/// Errors that can occur during evm.rs calls
#[derive(Debug, Error)]
pub enum NodeError {
    #[error(transparent)]
    ExecutionError(#[from] EvmError),

    #[error("out of sync: {0} slots behind")]
    OutOfSync(u64),

    #[error("node error: {0:?}")]
    Generic (String),

    // Casting existing errors
    #[error(transparent)]
    Eyre(#[from] Report),
}

impl NodeError {
    pub fn to_json_rpsee_error(self) -> jsonrpsee::core::Error {
        info!("to_json_rpsee_error");
        match self {
            NodeError::ExecutionError(evm_err) => match evm_err {
                EvmError::Revert(data) => {
                    let mut msg = "execution reverted".to_string();
                    if let Some(reason) = data.as_ref().and_then(EvmError::decode_revert_reason) {
                        msg = format!("{msg}: {reason}")
                    }
                    jsonrpsee::core::Error::Call(
                        jsonrpsee::types::error::CallError::Custom(
                            jsonrpsee::types::error::ErrorObject::owned(
                                3, 
                                msg, 
                                data.map(|data| format!("0x{}", hex::encode(data))))))
                }
                _ => jsonrpsee::core::Error::Custom(evm_err.to_string())
            }
            _ => jsonrpsee::core::Error::Custom(self.to_string())
        }
    }
}