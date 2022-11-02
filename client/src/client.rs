use std::sync::Arc;

use ethers::prelude::{Address, U256};
use ethers::types::{Transaction, TransactionReceipt, H256};
use eyre::{eyre, Result};

use common::types::BlockTag;
use config::Config;
use consensus::types::Header;
use execution::types::{CallOpts, ExecutionBlock};
use log::{info, warn};
use tokio::spawn;
use tokio::sync::RwLock;
use tokio::time::sleep;

use crate::database::{Database, FileDB};
use crate::node::Node;
use crate::rpc::Rpc;

pub struct Client<DB: Database> {
    node: Arc<RwLock<Node>>,
    rpc: Option<Rpc>,
    db: DB,
}

impl Client<FileDB> {
    pub async fn new(config: Config) -> Result<Self> {
        let config = Arc::new(config);
        let node = Node::new(config.clone()).await?;
        let node = Arc::new(RwLock::new(node));

        let rpc = if let Some(port) = config.rpc_port {
            Some(Rpc::new(node.clone(), port))
        } else {
            None
        };

        let data_dir = config.data_dir.clone();
        let db = FileDB::new(data_dir.ok_or(eyre!("data dir not found"))?);

        Ok(Client { node, rpc, db })
    }
}

impl<DB: Database> Client<DB> {
    pub async fn start(&mut self) -> Result<()> {
        self.rpc.as_mut().unwrap().start().await?;

        let node = self.node.clone();
        spawn(async move {
            let res = node.write().await.sync().await;
            if let Err(err) = res {
                warn!("consensus error: {}", err);
            }

            loop {
                let res = node.write().await.advance().await;
                if let Err(err) = res {
                    warn!("consensus error: {}", err);
                }

                let next_update = node.read().await.duration_until_next_update();
                sleep(next_update).await;
            }
        });

        Ok(())
    }

    pub async fn shutdown(&self) {
        let node = self.node.read().await;
        let checkpoint = if let Some(checkpoint) = node.get_last_checkpoint() {
            checkpoint
        } else {
            return;
        };

        info!("saving last checkpoint hash");
        let res = self.db.save_checkpoint(checkpoint);
        if res.is_err() {
            warn!("checkpoint save failed");
        }
    }

    pub async fn call(&self, opts: &CallOpts, block: &BlockTag) -> Result<Vec<u8>> {
        self.node.read().await.call(opts, block).await
    }

    pub async fn estimate_gas(&self, opts: &CallOpts) -> Result<u64> {
        self.node.read().await.estimate_gas(opts).await
    }

    pub async fn get_balance(&self, address: &Address, block: &BlockTag) -> Result<U256> {
        self.node.read().await.get_balance(address, block).await
    }

    pub async fn get_nonce(&self, address: &Address, block: &BlockTag) -> Result<u64> {
        self.node.read().await.get_nonce(address, block).await
    }

    pub async fn get_code(&self, address: &Address, block: &BlockTag) -> Result<Vec<u8>> {
        self.node.read().await.get_code(address, block).await
    }

    pub async fn get_storage_at(&self, address: &Address, slot: H256) -> Result<U256> {
        self.node.read().await.get_storage_at(address, slot).await
    }

    pub async fn send_raw_transaction(&self, bytes: &Vec<u8>) -> Result<H256> {
        self.node.read().await.send_raw_transaction(bytes).await
    }

    pub async fn get_transaction_receipt(
        &self,
        tx_hash: &H256,
    ) -> Result<Option<TransactionReceipt>> {
        self.node
            .read()
            .await
            .get_transaction_receipt(tx_hash)
            .await
    }

    pub async fn get_transaction_by_hash(&self, tx_hash: &H256) -> Result<Option<Transaction>> {
        self.node
            .read()
            .await
            .get_transaction_by_hash(tx_hash)
            .await
    }

    pub async fn get_gas_price(&self) -> Result<U256> {
        self.node.read().await.get_gas_price()
    }

    pub async fn get_priority_fee(&self) -> Result<U256> {
        self.node.read().await.get_priority_fee()
    }

    pub async fn get_block_number(&self) -> Result<u64> {
        self.node.read().await.get_block_number()
    }

    pub async fn get_block_by_number(
        &self,
        block: &BlockTag,
        full_tx: bool,
    ) -> Result<Option<ExecutionBlock>> {
        self.node
            .read()
            .await
            .get_block_by_number(block, full_tx)
            .await
    }

    pub async fn get_block_by_hash(
        &self,
        hash: &Vec<u8>,
        full_tx: bool,
    ) -> Result<Option<ExecutionBlock>> {
        self.node
            .read()
            .await
            .get_block_by_hash(hash, full_tx)
            .await
    }

    pub async fn chain_id(&self) -> u64 {
        self.node.read().await.chain_id()
    }

    pub async fn get_header(&self) -> Result<Header> {
        self.node.read().await.get_header()
    }
}
