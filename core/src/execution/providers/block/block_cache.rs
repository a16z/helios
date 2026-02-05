use std::collections::BTreeMap;
use std::sync::Arc;

use alloy::consensus::BlockHeader;
use alloy::eips::{BlockId, BlockNumberOrTag};
use alloy::network::{primitives::HeaderResponse, BlockResponse};
use alloy::primitives::B256;
use alloy::rpc::types::BlockTransactions;
use async_trait::async_trait;

use eyre::Result;
use helios_common::{execution_provider::BlockProvider, network_spec::NetworkSpec};
use tokio::sync::RwLock;
use tracing::warn;

use crate::execution::constants::MAX_STATE_HISTORY_LENGTH;

pub struct BlockCache<N: NetworkSpec> {
    latest: Arc<RwLock<Option<N::BlockResponse>>>,
    finalized: Arc<RwLock<Option<N::BlockResponse>>>,
    blocks: Arc<RwLock<BTreeMap<u64, N::BlockResponse>>>,
    hashes: Arc<RwLock<BTreeMap<B256, u64>>>,
    size: usize,
}

impl<N: NetworkSpec> BlockCache<N> {
    pub fn new() -> Self {
        Self {
            latest: Arc::default(),
            finalized: Arc::default(),
            blocks: Arc::default(),
            hashes: Arc::default(),
            size: MAX_STATE_HISTORY_LENGTH,
        }
    }

    /// Clear all cached blocks except the finalized block
    /// Finalized blocks are preserved since they cannot be reorganized
    async fn clear(&self) {
        let finalized_block = self.finalized.read().await.clone();

        self.blocks.write().await.clear();
        self.hashes.write().await.clear();
        *self.latest.write().await = None;

        // Re-insert finalized block if it exists
        if let Some(finalized) = finalized_block {
            let block_number = finalized.header().number();
            let block_hash = finalized.header().hash();

            self.blocks.write().await.insert(block_number, finalized);
            self.hashes.write().await.insert(block_hash, block_number);
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
        let block = match block_id {
            BlockId::Number(tag) => match tag {
                BlockNumberOrTag::Latest => self.latest.read().await.clone(),
                BlockNumberOrTag::Finalized | BlockNumberOrTag::Safe => {
                    self.finalized.read().await.clone()
                }
                BlockNumberOrTag::Number(number) => self.blocks.read().await.get(&number).cloned(),
                BlockNumberOrTag::Pending | BlockNumberOrTag::Earliest => None,
            },
            BlockId::Hash(hash) => {
                let hash: B256 = hash.into();
                if let Some(number) = self.hashes.read().await.get(&hash) {
                    self.blocks.read().await.get(number).cloned()
                } else {
                    None
                }
            }
        };

        if !full_tx {
            if let Some(mut block) = block {
                *block.transactions_mut() =
                    BlockTransactions::Hashes(block.transactions().hashes().collect());

                Ok(Some(block))
            } else {
                Ok(None)
            }
        } else {
            Ok(block)
        }
    }

    async fn get_untrusted_block(
        &self,
        _block_id: BlockId,
        _full_tx: bool,
    ) -> Result<Option<<N>::BlockResponse>> {
        Ok(None)
    }

    async fn push_block(&self, block: N::BlockResponse, block_id: BlockId) {
        let block_number = block.header().number();
        let block_hash = block.header().hash();
        let parent_hash = block.header().parent_hash();

        // Check if this block builds on top of existing history
        let is_consistent = if block_id.is_finalized() {
            // Skip check for finalized blocks
            true
        } else if block_number == 0 {
            // Genesis block is always consistent
            true
        } else {
            let blocks = self.blocks.read().await;

            // Check if parent block exists and has the expected hash
            if let Some(parent_block) = blocks.get(&(block_number - 1)) {
                parent_block.header().hash() == parent_hash
            } else {
                // No parent block in cache - check if new block continues from finalized or latest block
                let finalized = self.finalized.read().await;
                if let Some((latest_number, latest_block)) = blocks.last_key_value() {
                    // Check if new block continues from finalized block
                    if let Some(finalized_block) = finalized.as_ref() {
                        let finalized_number = finalized_block.header().number();
                        if block_number == finalized_number + 1 {
                            parent_hash == finalized_block.header().hash()
                        } else if block_number == *latest_number + 1 {
                            // Check if new block continues from latest block in cache
                            latest_block.header().hash() == parent_hash
                        } else {
                            // Block doesn't continue from any known block
                            false
                        }
                    } else {
                        // No finalized block - check if new block continues from latest block
                        if block_number == *latest_number + 1 {
                            latest_block.header().hash() == parent_hash
                        } else {
                            // Block doesn't continue from any known block
                            false
                        }
                    }
                } else {
                    // Block cache is completely empty
                    true
                }
            }
        };

        // If the block is inconsistent with existing history, clear the cache
        if !is_consistent {
            warn!("inconsistent block history detected: clearing cache");
            self.clear().await;
        }

        // Update latest/finalized references
        if let BlockId::Number(tag) = block_id {
            match tag {
                BlockNumberOrTag::Latest => *self.latest.write().await = Some(block.clone()),
                BlockNumberOrTag::Finalized => *self.finalized.write().await = Some(block.clone()),
                _ => (),
            }
        }

        // Insert the new block
        self.hashes.write().await.insert(block_hash, block_number);

        self.blocks.write().await.insert(block_number, block);

        // Maintain cache size limit
        while self.blocks.read().await.len() > self.size {
            let (num, old_block_hash) = {
                let blocks = self.blocks.read().await;
                if let Some((num, block)) = blocks.first_key_value() {
                    (*num, block.header().hash())
                } else {
                    break;
                }
            };

            self.blocks.write().await.remove(&num).unwrap();
            self.hashes.write().await.remove(&old_block_hash);
        }
    }
}
