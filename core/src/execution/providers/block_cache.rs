use std::collections::BTreeMap;
use std::sync::Arc;

use alloy::consensus::BlockHeader;
use alloy::eips::{BlockId, BlockNumberOrTag};
use alloy::network::{primitives::HeaderResponse, BlockResponse};
use alloy::primitives::B256;
use async_trait::async_trait;

use eyre::Result;
use helios_common::network_spec::NetworkSpec;
use tokio::sync::RwLock;

use super::BlockProvider;

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
            size: 256,
        }
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
                BlockNumberOrTag::Finalized => self.finalized.read().await.clone(),
                BlockNumberOrTag::Number(number) => self.blocks.read().await.get(&number).cloned(),
                _ => None,
            },
            BlockId::Hash(hash) => {
                let hash: B256 = hash.into();
                if let Some(number) = self.hashes.read().await.get(&hash) {
                    self.blocks.read().await.get(&number).cloned()
                } else {
                    None
                }
            }
        };

        if !full_tx {
            if let Some(mut block) = block {
                block.transactions_mut().hashes();
                Ok(Some(block))
            } else {
                Ok(None)
            }
        } else {
            Ok(block)
        }
    }

    async fn push_block(&self, block: N::BlockResponse, block_id: BlockId) {
        match block_id {
            BlockId::Number(tag) => match tag {
                BlockNumberOrTag::Latest => *self.latest.write().await = Some(block.clone()),
                BlockNumberOrTag::Finalized => *self.finalized.write().await = Some(block.clone()),
                _ => (),
            },
            _ => (),
        }

        self.hashes
            .write()
            .await
            .insert(block.header().hash(), block.header().number());

        self.blocks
            .write()
            .await
            .insert(block.header().number(), block);

        while self.blocks.read().await.len() > self.size {
            let blocks = self.blocks.read().await;
            let entry = blocks.first_key_value();
            let (num, block) = entry.as_ref().unwrap();
            let block_hash = block.header().hash();
            self.blocks.write().await.remove(&num).unwrap();
            self.hashes.write().await.remove(&block_hash);
        }
    }
}
