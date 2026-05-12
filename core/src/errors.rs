use alloy::eips::BlockId;
use eyre::Report;
use helios_common::types::EvmError;
use thiserror::Error;

use crate::execution::errors::ExecutionError;

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("block not found: {0}")]
    BlockNotFound(BlockId),
    #[error("out of sync: {0} seconds behind")]
    OutOfSync(u64),
    #[error("execution error: {0}")]
    ExecutionError(ExecutionError),
    #[error("evm error: {0}")]
    EvmError(EvmError),
    #[error("consensus error: {0}")]
    ConsensusError(Report),
    #[error("internal error: {0}")]
    InternalError(Report),
}

impl From<ExecutionError> for ClientError {
    fn from(value: ExecutionError) -> Self {
        match value {
            ExecutionError::BlockNotFound(tag) => ClientError::BlockNotFound(tag),
            err => ClientError::ExecutionError(err),
        }
    }
}

#[derive(Debug, Error)]
#[error("rpc error on method: {method}, message: {error}")]
pub struct RpcError<E: ToString> {
    method: String,
    error: E,
}

impl<E: ToString> RpcError<E> {
    pub fn new(method: &str, err: E) -> Self {
        Self {
            method: method.to_string(),
            error: err,
        }
    }
}
