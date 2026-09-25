use std::collections::{BTreeMap, VecDeque};

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
    // Update the canonical height index and immutable hash cache atomically.
    state: RwLock<CacheState<N>>,
}

struct CacheState<N: NetworkSpec> {
    latest: Option<N::BlockResponse>,
    finalized: Option<N::BlockResponse>,
    canonical: BTreeMap<u64, B256>,
    blocks: BTreeMap<B256, N::BlockResponse>,
    insertion_order: VecDeque<B256>,
}

impl<N: NetworkSpec> BlockCache<N> {
    pub fn new() -> Self {
        Self {
            state: RwLock::new(CacheState {
                latest: None,
                finalized: None,
                canonical: BTreeMap::new(),
                blocks: BTreeMap::new(),
                insertion_order: VecDeque::new(),
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
                BlockNumberOrTag::Number(number) => state
                    .canonical
                    .get(&number)
                    .and_then(|hash| state.blocks.get(hash))
                    .cloned(),
                BlockNumberOrTag::Pending | BlockNumberOrTag::Earliest => None,
            },
            BlockId::Hash(hash) => state
                .blocks
                .get(&hash.block_hash)
                .filter(|block| {
                    hash.require_canonical != Some(true)
                        || state.canonical.get(&block.header().number()) == Some(&hash.block_hash)
                })
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
        let known_canonical = state.canonical.get(&number) == Some(&hash);
        let conflicting_height = state.canonical.get(&number).is_some_and(|old| *old != hash);
        let inconsistent_parent = number.checked_sub(1).is_some_and(|parent_number| {
            if let Some(parent) = state.canonical.get(&parent_number) {
                *parent != block.header().parent_hash()
            } else {
                state.canonical.last_key_value().is_some_and(|(last, _)| {
                    *last
                        > state
                            .finalized
                            .as_ref()
                            .map(|b| b.header().number())
                            .unwrap_or_default()
                })
            }
        });
        if conflicting_height
            || (!known_canonical && !block_id.is_finalized() && inconsistent_parent)
        {
            warn!("inconsistent block history detected: clearing canonical index");
            state.canonical.clear();
            state.latest = None;
            if let Some(finalized) = state.finalized.clone() {
                state
                    .canonical
                    .insert(finalized.header().number(), finalized.header().hash());
            }
            // Previously authenticated blocks remain valid by hash. Keep them
            // for pinned executions, but no longer resolve them by height.
        }

        match block_id {
            BlockId::Number(BlockNumberOrTag::Latest) => state.latest = Some(block.clone()),
            BlockId::Number(BlockNumberOrTag::Finalized) => state.finalized = Some(block.clone()),
            _ => (),
        }
        state.canonical.insert(number, hash);
        if state.blocks.insert(hash, block).is_none() {
            state.insertion_order.push_back(hash);
        }
        while state.blocks.len() > MAX_STATE_HISTORY_LENGTH {
            if let Some(old_hash) = state.insertion_order.pop_front() {
                // Keep finalized available by hash, even when finality stalls.
                if state
                    .finalized
                    .as_ref()
                    .is_some_and(|block| block.header().hash() == old_hash)
                {
                    state.insertion_order.push_back(old_hash);
                    continue;
                }
                if let Some(old) = state.blocks.remove(&old_hash) {
                    let old_number = old.header().number();
                    if state.canonical.get(&old_number) == Some(&old_hash) {
                        state.canonical.remove(&old_number);
                    }
                }
            }
        }
    }
}
