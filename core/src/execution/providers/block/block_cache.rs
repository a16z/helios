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

impl<N: NetworkSpec> BlockCache<N> {
    /// If a block other than `block_hash` is cached at `block_number`, remove it together with
    /// the cached blocks above it, and their hash mappings.
    async fn remove_replaced_block(&self, block_number: u64, block_hash: B256) {
        let mut blocks = self.blocks.write().await;
        let replaced = blocks
            .get(&block_number)
            .is_some_and(|block| block.header().hash() != block_hash);
        if !replaced {
            return;
        }

        let mut hashes = self.hashes.write().await;
        for (_, block) in blocks.split_off(&block_number) {
            hashes.remove(&block.header().hash());
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
                let number = self.hashes.read().await.get(&hash).copied();
                if let Some(number) = number {
                    // Only return the cached block if it is the one that was asked for.
                    self.blocks
                        .read()
                        .await
                        .get(&number)
                        .filter(|block| block.header().hash() == hash)
                        .cloned()
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
                // No parent block in cache - check if cache is empty except finalized blocks
                let finalized = self.finalized.read().await;
                if let Some((latest_number, _)) = blocks.last_key_value() {
                    // All blocks in cache are finalized
                    *latest_number
                        <= finalized
                            .as_ref()
                            .map(|block| block.header().number())
                            .unwrap_or_default()
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

        // A different block already cached at this height is being replaced: drop its hash
        // mapping and every block cached on top of it, which no longer has a cached parent.
        self.remove_replaced_block(block_number, block_hash).await;

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

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::rpc::types::{Block, Header};
    use helios_ethereum::spec::Ethereum;

    fn block(number: u64, hash: u8, parent: u8) -> Block {
        let mut block = Block::<_, Header>::default();
        block.header.hash = B256::repeat_byte(hash);
        block.header.inner.number = number;
        block.header.inner.parent_hash = B256::repeat_byte(parent);
        block
    }

    async fn get_by_hash(cache: &BlockCache<Ethereum>, hash: u8) -> Option<B256> {
        cache
            .get_block(BlockId::Hash(B256::repeat_byte(hash).into()), false)
            .await
            .unwrap()
            .map(|block| block.header.hash)
    }

    #[tokio::test]
    async fn replaced_sibling_is_no_longer_found_by_hash() {
        let cache = BlockCache::<Ethereum>::new();
        let latest: BlockId = BlockNumberOrTag::Latest.into();
        cache.push_block(block(9, 0x09, 0x08), latest).await;
        cache.push_block(block(10, 0x0a, 0x09), latest).await;
        cache.push_block(block(10, 0x0b, 0x09), latest).await;

        assert_eq!(get_by_hash(&cache, 0x0a).await, None);
        assert_eq!(
            get_by_hash(&cache, 0x0b).await,
            Some(B256::repeat_byte(0x0b))
        );
        assert_eq!(
            get_by_hash(&cache, 0x09).await,
            Some(B256::repeat_byte(0x09))
        );
    }

    #[tokio::test]
    async fn replacing_a_block_drops_the_blocks_built_on_it() {
        let cache = BlockCache::<Ethereum>::new();
        let latest: BlockId = BlockNumberOrTag::Latest.into();
        cache.push_block(block(9, 0x09, 0x08), latest).await;
        cache.push_block(block(10, 0x0a, 0x09), latest).await;
        cache.push_block(block(11, 0x1a, 0x0a), latest).await;
        // A sibling of block 10 arrives through a by-number push.
        let number: BlockId = BlockNumberOrTag::Number(10).into();
        cache.push_block(block(10, 0x0b, 0x09), number).await;

        assert_eq!(get_by_hash(&cache, 0x0a).await, None);
        assert_eq!(get_by_hash(&cache, 0x1a).await, None);
        let at_11 = cache
            .get_block(BlockNumberOrTag::Number(11).into(), false)
            .await
            .unwrap();
        assert!(at_11.is_none());
        assert_eq!(
            get_by_hash(&cache, 0x0b).await,
            Some(B256::repeat_byte(0x0b))
        );
    }

    #[tokio::test]
    async fn pushing_the_same_block_again_keeps_its_descendants() {
        let cache = BlockCache::<Ethereum>::new();
        let latest: BlockId = BlockNumberOrTag::Latest.into();
        cache.push_block(block(9, 0x09, 0x08), latest).await;
        cache.push_block(block(10, 0x0a, 0x09), latest).await;
        cache.push_block(block(11, 0x1a, 0x0a), latest).await;
        let number: BlockId = BlockNumberOrTag::Number(10).into();
        cache.push_block(block(10, 0x0a, 0x09), number).await;

        assert_eq!(
            get_by_hash(&cache, 0x0a).await,
            Some(B256::repeat_byte(0x0a))
        );
        assert_eq!(
            get_by_hash(&cache, 0x1a).await,
            Some(B256::repeat_byte(0x1a))
        );
    }
}
