use std::{collections::BTreeMap, sync::Arc, time::Duration};

use common::types::{Block, BlockTag};
use ethers::types::Transaction;
use tokio::sync::{broadcast::Receiver, watch, RwLock};

pub struct Blocks {
    inner: Arc<RwLock<Inner>>,
}

struct Inner {
    blocks: BTreeMap<u64, Block<Transaction>>,
    finalized_block: Option<Block<Transaction>>,
    block_recv: Receiver<Block<Transaction>>,
    finalized_block_recv: watch::Receiver<Option<Block<Transaction>>>,
}

impl Blocks {
    pub fn new(
        block_recv: Receiver<Block<Transaction>>,
        finalized_block_recv: watch::Receiver<Option<Block<Transaction>>>,
    ) -> Self {
        let inner = Arc::new(RwLock::new(Inner {
            blocks: BTreeMap::new(),
            finalized_block: None,
            block_recv,
            finalized_block_recv,
        }));

        let inner_copy = inner.clone();

        tokio::spawn(async move {
            loop {
                let inner_read = inner_copy.read().await;
                println!("check: {}", inner_read.block_recv.len());
                if !inner_read.block_recv.is_empty()
                    || inner_read.finalized_block_recv.has_changed().unwrap()
                {
                    println!("inside");
                    drop(inner_read);

                    let mut inner_write = inner_copy.write().await;
                    while let Ok(block) = inner_write.block_recv.try_recv() {
                        inner_write.blocks.insert(block.number.as_u64(), block);
                    }

                    let finalized_block =
                        inner_write.finalized_block_recv.borrow_and_update().clone();
                    inner_write.finalized_block = finalized_block;

                    while inner_write.blocks.len() > 64 {
                        inner_write.blocks.pop_first();
                    }
                }

                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });

        Self { inner }
    }

    pub async fn get_block(&self, tag: BlockTag) -> Option<Block<Transaction>> {
        match tag {
            BlockTag::Latest => self
                .inner
                .read()
                .await
                .blocks
                .last_key_value()
                .map(|e| e.1.clone()),
            BlockTag::Finalized => self.inner.read().await.finalized_block.clone(),
            BlockTag::Number(num) => self.inner.read().await.blocks.get(&num).cloned(),
        }
    }
}
