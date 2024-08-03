use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use alloy::primitives::{Address, B256, U256};
use alloy::rpc::types::Transaction;
use tokio::{
    select,
    sync::{mpsc::Receiver, watch, RwLock},
};

use common::types::{Block, BlockTag, Transactions};

#[derive(Clone)]
pub struct State {
    inner: Arc<RwLock<Inner>>,
}

impl State {
    pub fn new(
        mut block_recv: Receiver<Block>,
        mut finalized_block_recv: watch::Receiver<Option<Block>>,
        history_length: u64,
    ) -> Self {
        let inner = Arc::new(RwLock::new(Inner::new(history_length)));
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
                            inner_ref.write().await.push_block(block);
                        }
                    },
                    _ = finalized_block_recv.changed() => {
                        let block = finalized_block_recv.borrow_and_update().clone();
                        if let Some(block) = block {
                            inner_ref.write().await.push_finalized_block(block);
                        }

                    }
                }
            }
        });

        Self { inner }
    }

    pub async fn push_block(&self, block: Block) {
        self.inner.write().await.push_block(block);
    }

    // full block fetch

    pub async fn get_block(&self, tag: BlockTag) -> Option<Block> {
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

    pub async fn get_block_by_hash(&self, hash: B256) -> Option<Block> {
        let inner = self.inner.read().await;
        inner
            .hashes
            .get(&hash)
            .and_then(|number| inner.blocks.get(number))
            .cloned()
    }

    // transaction fetch

    pub async fn get_transaction(&self, hash: B256) -> Option<Transaction> {
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

    pub async fn get_transaction_by_block_and_index(
        &self,
        block_hash: B256,
        index: u64,
    ) -> Option<Transaction> {
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

#[derive(Default)]
struct Inner {
    blocks: BTreeMap<u64, Block>,
    finalized_block: Option<Block>,
    hashes: HashMap<B256, u64>,
    txs: HashMap<B256, TransactionLocation>,
    history_length: u64,
}

impl Inner {
    pub fn new(history_length: u64) -> Self {
        Self {
            history_length,
            ..Default::default()
        }
    }

    pub fn push_block(&mut self, block: Block) {
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

        while self.blocks.len() as u64 > self.history_length {
            if let Some((number, _)) = self.blocks.first_key_value() {
                self.remove_block(*number);
            }
        }
    }

    pub fn push_finalized_block(&mut self, block: Block) {
        self.finalized_block = Some(block.clone());

        if let Some(old_block) = self.blocks.get(&block.number.to()) {
            if old_block.hash != block.hash {
                self.remove_block(old_block.number.to());
                self.push_block(block)
            }
        } else {
            self.push_block(block);
        }
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
