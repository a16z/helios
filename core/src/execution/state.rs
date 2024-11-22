use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use alloy::{
    primitives::{Address, B256, U256},
    signers::k256::elliptic_curve::rand_core::block,
};
use eyre::{eyre, Result};
use tokio::{
    select,
    sync::{mpsc::Receiver, watch, RwLock},
};
use tracing::{info, warn};

use crate::network_spec::NetworkSpec;
use crate::types::{Block, BlockTag, Transactions};

use super::rpc::ExecutionRpc;

#[derive(Clone)]
pub struct State<N: NetworkSpec, R: ExecutionRpc<N>> {
    inner: Arc<RwLock<Inner<N, R>>>,
}

impl<N: NetworkSpec, R: ExecutionRpc<N>> State<N, R> {
    pub fn new(
        mut block_recv: Receiver<Block<N::TransactionResponse>>,
        mut finalized_block_recv: watch::Receiver<Option<Block<N::TransactionResponse>>>,
        history_length: u64,
        rpc: &str,
    ) -> Self {
        let rpc = R::new(rpc).unwrap();
        let inner = Arc::new(RwLock::new(Inner::new(history_length, rpc)));
        let inner_ref = inner.clone();

        #[cfg(not(target_arch = "wasm32"))]
        let run = tokio::spawn;
        #[cfg(target_arch = "wasm32")]
        let run = wasm_bindgen_futures::spawn_local;

        run(async move {
            loop {
                select! {
                    block = block_recv.recv() => {
                        if let Some(block) = block {
                            inner_ref.write().await.push_block(block).await;
                        }
                    },
                    _ = finalized_block_recv.changed() => {
                        let block = finalized_block_recv.borrow_and_update().clone();
                        if let Some(block) = block {
                            inner_ref.write().await.push_finalized_block(block).await;
                        }

                    },
                }
            }
        });

        Self { inner }
    }

    pub async fn push_block(&self, block: Block<N::TransactionResponse>) {
        self.inner.write().await.push_block(block).await;
    }

    // full block fetch

    pub async fn get_block(&self, tag: BlockTag) -> Option<Block<N::TransactionResponse>> {
        match tag {
            BlockTag::Latest => self
                .inner
                .read()
                .await
                .blocks
                .last_key_value()
                .map(|entry| entry.1)
                .cloned(),
            BlockTag::Finalized => self.inner.read().await.finalized_block.clone(),
            BlockTag::Number(number) => self.inner.read().await.blocks.get(&number).cloned(),
        }
    }

    pub async fn get_block_by_hash(&self, hash: B256) -> Option<Block<N::TransactionResponse>> {
        let inner = self.inner.read().await;
        inner
            .hashes
            .get(&hash)
            .and_then(|number| inner.blocks.get(number))
            .cloned()
    }

    // transaction fetch

    pub async fn get_transaction(&self, hash: B256) -> Option<N::TransactionResponse> {
        let inner = self.inner.read().await;
        inner
            .txs
            .get(&hash)
            .and_then(|loc| {
                inner
                    .blocks
                    .get(&loc.block)
                    .and_then(|block| match &block.transactions {
                        Transactions::Full(txs) => txs.get(loc.index),
                        Transactions::Hashes(_) => unreachable!(),
                    })
            })
            .cloned()
    }

    pub async fn get_transaction_by_block_hash_and_index(
        &self,
        block_hash: B256,
        index: u64,
    ) -> Option<N::TransactionResponse> {
        let inner = self.inner.read().await;
        inner
            .hashes
            .get(&block_hash)
            .and_then(|number| inner.blocks.get(number))
            .and_then(|block| match &block.transactions {
                Transactions::Full(txs) => txs.get(index as usize),
                Transactions::Hashes(_) => unreachable!(),
            })
            .cloned()
    }

    pub async fn get_transaction_by_block_and_index(
        &self,
        tag: BlockTag,
        index: u64,
    ) -> Option<N::TransactionResponse> {
        let block = self.get_block(tag).await?;
        match &block.transactions {
            Transactions::Full(txs) => txs.get(index as usize).cloned(),
            Transactions::Hashes(_) => unreachable!(),
        }
    }

    // block field fetch

    pub async fn get_state_root(&self, tag: BlockTag) -> Option<B256> {
        self.get_block(tag).await.map(|block| block.state_root)
    }

    pub async fn get_receipts_root(&self, tag: BlockTag) -> Option<B256> {
        self.get_block(tag).await.map(|block| block.receipts_root)
    }

    pub async fn get_base_fee(&self, tag: BlockTag) -> Option<U256> {
        self.get_block(tag)
            .await
            .map(|block| block.base_fee_per_gas)
    }

    pub async fn get_coinbase(&self, tag: BlockTag) -> Option<Address> {
        self.get_block(tag).await.map(|block| block.miner)
    }

    // misc

    pub async fn latest_block_number(&self) -> Option<u64> {
        let inner = self.inner.read().await;
        inner.blocks.last_key_value().map(|entry| *entry.0)
    }

    pub async fn oldest_block_number(&self) -> Option<u64> {
        let inner = self.inner.read().await;
        inner.blocks.first_key_value().map(|entry| *entry.0)
    }
}

struct Inner<N: NetworkSpec, R: ExecutionRpc<N>> {
    blocks: BTreeMap<u64, Block<N::TransactionResponse>>,
    finalized_block: Option<Block<N::TransactionResponse>>,
    hashes: HashMap<B256, u64>,
    txs: HashMap<B256, TransactionLocation>,
    history_length: u64,
    rpc: R,
}

impl<N: NetworkSpec, R: ExecutionRpc<N>> Inner<N, R> {
    pub fn new(history_length: u64, rpc: R) -> Self {
        Self {
            history_length,
            blocks: BTreeMap::default(),
            finalized_block: None,
            hashes: HashMap::default(),
            txs: HashMap::default(),
            rpc,
        }
    }

    pub async fn push_block(&mut self, block: Block<N::TransactionResponse>) {
        let block_number = block.number.to::<u64>();
        if self.try_insert_tip(block) {
            let mut n = block_number;

            loop {
                if let Ok(backfilled) = self.backfill_behind(n).await {
                    if !backfilled {
                        break;
                    }
                    n -= 1;
                } else {
                    self.prune_before(n);
                    break;
                }
            }

            let link_child = self.blocks.get(&n);
            let link_parent = self.blocks.get(&(n - 1));

            if let (Some(parent), Some(child)) = (link_parent, link_child) {
                if child.parent_hash != parent.hash {
                    warn!("detected block reorganization");
                    self.prune_before(n);
                }
            }

            self.prune();
        }
    }

    fn try_insert_tip(&mut self, block: Block<N::TransactionResponse>) -> bool {
        if let Some((num, _)) = self.blocks.last_key_value() {
            if num > &block.number.to() {
                return false;
            }
        }

        self.hashes.insert(block.hash, block.number.to());
        block
            .transactions
            .hashes()
            .iter()
            .enumerate()
            .for_each(|(i, tx)| {
                let location = TransactionLocation {
                    block: block.number.to(),
                    index: i,
                };
                self.txs.insert(*tx, location);
            });

        self.blocks.insert(block.number.to(), block);
        true
    }

    fn prune(&mut self) {
        while self.blocks.len() as u64 > self.history_length {
            if let Some((number, _)) = self.blocks.first_key_value() {
                self.remove_block(*number);
            }
        }
    }

    fn prune_before(&mut self, n: u64) {
        loop {
            if let Some((oldest, _)) = self.blocks.first_key_value() {
                let oldest = *oldest;
                if oldest < n {
                    self.blocks.remove(&oldest);
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    async fn backfill_behind(&mut self, n: u64) -> Result<bool> {
        if self.blocks.len() < 2 {
            return Ok(false);
        }

        if let Some(block) = self.blocks.get(&n) {
            let prev = n - 1;
            if self.blocks.get(&prev).is_none() {
                let backfilled = self.rpc.get_block(block.parent_hash).await?;
                if backfilled.is_hash_valid() && block.parent_hash == backfilled.hash {
                    info!("backfilled: block={}", backfilled.number);
                    self.blocks.insert(backfilled.number.to(), backfilled);
                    Ok(true)
                } else {
                    warn!("bad block backfill");
                    Err(eyre!("bad backfill"))
                }
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }

    pub async fn push_finalized_block(&mut self, block: Block<N::TransactionResponse>) {
        if let Some(old_block) = self.blocks.get(&block.number.to()) {
            if old_block.hash != block.hash {
                self.blocks = BTreeMap::new();
            }
        }

        self.finalized_block = Some(block.clone());
        self.push_block(block).await;
    }

    fn remove_block(&mut self, number: u64) {
        if let Some(block) = self.blocks.remove(&number) {
            self.hashes.remove(&block.hash);
            block.transactions.hashes().iter().for_each(|tx| {
                self.txs.remove(tx);
            });
        }
    }
}

struct TransactionLocation {
    block: u64,
    index: usize,
}
