use thiserror::Error;

use crate::types::BlockTag;

#[derive(Debug, Error)]
#[error("block {block} not available")]
pub struct BlockNotFoundError {
    block: BlockTag,
}

impl BlockNotFoundError {
    pub fn new(block: BlockTag) -> Self {
        Self { block }
    }
}
