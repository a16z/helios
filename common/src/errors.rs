use alloc::string::{String, ToString};
use alloy::primitives::B256;
use thiserror_no_std::Error;

use crate::types::BlockTag;

#[derive(Debug, Error)]
#[error("block not available: {block}")]
pub struct BlockNotFoundError {
    block: BlockTag,
}

impl BlockNotFoundError {
    pub fn new(block: BlockTag) -> Self {
        Self { block }
    }
}

#[derive(Debug, Error)]
#[error("slot not found: {slot:?}")]
pub struct SlotNotFoundError {
    slot: B256,
}

impl SlotNotFoundError {
    pub fn new(slot: B256) -> Self {
        Self { slot }
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

impl<E: ToString> From<RpcError<E>> for anyhow::Error {
    fn from(error: RpcError<E>) -> Self {
        anyhow::anyhow!(
            "RPC error in method '{}': {}",
            error.method,
            error.error.to_string()
        )
    }
}
