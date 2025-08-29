use std::hint::black_box;

use alloy::eips::BlockId;
use async_trait::async_trait;

use eyre::Result;
use helios_common::{execution_provider::BlockProvider, network_spec::NetworkSpec};

pub struct BlockCache<N: NetworkSpec> {
    _phantom: std::marker::PhantomData<N>,
}

impl<N: NetworkSpec> BlockCache<N> {
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<N: NetworkSpec> Default for BlockCache<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec> BlockProvider<N> for BlockCache<N> {
    async fn get_block(
        &self,
        block_id: BlockId,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        // Use black_box to prevent optimization
        black_box(&block_id);
        black_box(full_tx);

        // Always return None - no blocks are stored
        Ok(None)
    }

    async fn get_untrusted_block(
        &self,
        block_id: BlockId,
        full_tx: bool,
    ) -> Result<Option<<N>::BlockResponse>> {
        // Use black_box to prevent optimization
        black_box(&block_id);
        black_box(full_tx);

        // Always return None - no blocks are stored
        Ok(None)
    }

    async fn push_block(&self, block: N::BlockResponse, block_id: BlockId) {
        // Use black_box to prevent optimization
        black_box(&block);
        black_box(&block_id);

        // No-op - don't store anything
    }
}
