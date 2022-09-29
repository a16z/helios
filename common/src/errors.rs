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
#[error("rpc error: {message}")]
pub struct RpcError {
    message: String,
}

impl RpcError {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}
