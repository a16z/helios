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

pub struct BlockCache<N: NetworkSpec> {
    latest: Arc<RwLock<Option<N::BlockResponse>>>,
    finalized: Arc<RwLock<Option<N::BlockResponse>>>,
}

impl<N: NetworkSpec> BlockCache<N> {
    pub fn new() -> Self {
        Self {
            latest: Arc::default(),
            finalized: Arc::default(),
        }
    }

    fn handle_full_tx(
        &self,
        block: Option<N::BlockResponse>,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
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
                BlockNumberOrTag::Number(number) => {
                    // Check if the requested number matches latest or finalized
                    let latest = self.latest.read().await;
                    if let Some(ref latest_block) = *latest {
                        if latest_block.header().number() == number {
                            return self.handle_full_tx(latest.clone(), full_tx);
                        }
                    }

                    let finalized = self.finalized.read().await;
                    if let Some(ref finalized_block) = *finalized {
                        if finalized_block.header().number() == number {
                            return self.handle_full_tx(finalized.clone(), full_tx);
                        }
                    }

                    None
                }
                BlockNumberOrTag::Pending | BlockNumberOrTag::Earliest => None,
            },
            BlockId::Hash(hash) => {
                let hash: B256 = hash.into();

                // Check if the hash matches latest block
                let latest = self.latest.read().await;
                if let Some(ref latest_block) = *latest {
                    if latest_block.header().hash() == hash {
                        return self.handle_full_tx(latest.clone(), full_tx);
                    }
                }

                // Check if the hash matches finalized block
                let finalized = self.finalized.read().await;
                if let Some(ref finalized_block) = *finalized {
                    if finalized_block.header().hash() == hash {
                        return self.handle_full_tx(finalized.clone(), full_tx);
                    }
                }

                None
            }
        };

        self.handle_full_tx(block, full_tx)
    }

    async fn get_untrusted_block(
        &self,
        _block_id: BlockId,
        _full_tx: bool,
    ) -> Result<Option<<N>::BlockResponse>> {
        // We don't store untrusted blocks
        Ok(None)
    }

    async fn push_block(&self, block: N::BlockResponse, block_id: BlockId) {
        // Only store latest and finalized blocks
        if let BlockId::Number(tag) = block_id {
            match tag {
                BlockNumberOrTag::Latest => {
                    *self.latest.write().await = Some(block);
                }
                BlockNumberOrTag::Finalized => {
                    *self.finalized.write().await = Some(block);
                }
                _ => {
                    // Don't store any other blocks
                }
            }
        }
        // Don't store blocks identified by hash or other means
    }
}
