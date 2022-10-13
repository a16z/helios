use thiserror::Error;

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
#[error("rpc error on method: {method}, message: {message}")]
pub struct RpcError {
    method: String,
    message: String,
}

impl RpcError {
    pub fn new<E: ToString>(method: &str, err: E) -> Self {
        Self {
            method: method.to_string(),
            message: err.to_string(),
        }
    }
}
