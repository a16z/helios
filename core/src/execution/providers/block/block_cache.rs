use std::collections::BTreeMap;

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
    state: RwLock<CacheState<N>>,
}

struct CacheState<N: NetworkSpec> {
    latest: Option<N::BlockResponse>,
    finalized: Option<N::BlockResponse>,
    blocks: BTreeMap<u64, N::BlockResponse>,
    hashes: BTreeMap<B256, u64>,
    reorg_generation: u64,
}

impl<N: NetworkSpec> BlockCache<N> {
    pub fn new() -> Self {
        Self {
            state: RwLock::new(CacheState {
                latest: None,
                finalized: None,
                blocks: BTreeMap::new(),
                hashes: BTreeMap::new(),
                reorg_generation: 0,
            }),
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
    async fn reorg_generation(&self) -> u64 {
        self.state.read().await.reorg_generation
    }

    async fn get_block(
        &self,
        block_id: BlockId,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        let state = self.state.read().await;
        let mut block = match block_id {
            BlockId::Number(tag) => match tag {
                BlockNumberOrTag::Latest => state.latest.clone(),
                BlockNumberOrTag::Finalized | BlockNumberOrTag::Safe => state.finalized.clone(),
                BlockNumberOrTag::Number(number) => state.blocks.get(&number).cloned(),
                BlockNumberOrTag::Pending | BlockNumberOrTag::Earliest => None,
            },
            // Only canonical blocks are retained, regardless of requireCanonical.
            BlockId::Hash(hash) => state
                .hashes
                .get(&hash.block_hash)
                .and_then(|number| state.blocks.get(number))
                .cloned(),
        };

        if !full_tx {
            if let Some(block) = &mut block {
                *block.transactions_mut() =
                    BlockTransactions::Hashes(block.transactions().hashes().collect());
            }
        }
        Ok(block)
    }

    async fn get_untrusted_block(
        &self,
        _block_id: BlockId,
        _full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        Ok(None)
    }

    async fn push_block(&self, block: N::BlockResponse, block_id: BlockId) {
        let number = block.header().number();
        let hash = block.header().hash();
        let mut state = self.state.write().await;
        let existing_hash = state.blocks.get(&number).map(|block| block.header().hash());
        let known_block = existing_hash == Some(hash);
        let conflicting_height = existing_hash.is_some_and(|old| old != hash);
        let inconsistent_parent = number.checked_sub(1).is_some_and(|parent_number| {
            if let Some(parent) = state.blocks.get(&parent_number) {
                parent.header().hash() != block.header().parent_hash()
            } else {
                state.blocks.last_key_value().is_some_and(|(last, _)| {
                    *last
                        > state
                            .finalized
                            .as_ref()
                            .map(|block| block.header().number())
                            .unwrap_or_default()
                })
            }
        });

        if conflicting_height || (!known_block && !block_id.is_finalized() && inconsistent_parent) {
            warn!("inconsistent block history detected: clearing cache");
            // Invalidate executions together with the history they were reading.
            state.reorg_generation += 1;
            state.blocks.clear();
            state.hashes.clear();
            state.latest = None;
            if let Some(finalized) = state.finalized.clone() {
                let number = finalized.header().number();
                let hash = finalized.header().hash();
                state.blocks.insert(number, finalized);
                state.hashes.insert(hash, number);
            }
        }

        match block_id {
            BlockId::Number(BlockNumberOrTag::Latest) => state.latest = Some(block.clone()),
            BlockId::Number(BlockNumberOrTag::Finalized) => state.finalized = Some(block.clone()),
            _ => (),
        }
        if let Some(previous) = state.blocks.insert(number, block) {
            state.hashes.remove(&previous.header().hash());
        }
        state.hashes.insert(hash, number);

        // Keep finalized available by hash even when finality stalls.
        let finalized_number = state
            .finalized
            .as_ref()
            .map(|block| block.header().number());
        while state.blocks.len() > MAX_STATE_HISTORY_LENGTH {
            let oldest = *state
                .blocks
                .keys()
                .find(|number| Some(**number) != finalized_number)
                .unwrap();
            let removed = state.blocks.remove(&oldest).unwrap();
            state.hashes.remove(&removed.header().hash());
        }
    }
}
