use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use ethers::prelude::{Address, U256};
use ethers::types::{Transaction, TransactionReceipt, H256};
use eyre::Result;

use config::Config;
use consensus::types::Header;
use execution::types::{CallOpts, ExecutionBlock};
use tokio::spawn;
use tokio::sync::Mutex;
use tokio::time::sleep;

use crate::node::Node;
use crate::rpc::Rpc;

pub struct Client {
    node: Arc<Mutex<Node>>,
    rpc: Option<Rpc>,
}

impl Client {
    pub async fn new(config: Arc<Config>) -> Result<Self> {
        let node = Node::new(config.clone()).await?;
        let node = Arc::new(Mutex::new(node));

        let rpc = if let Some(port) = config.general.rpc_port {
            Some(Rpc::new(node.clone(), port))
        } else {
            None
        };

        Ok(Client { node, rpc })
    }

    pub async fn start_rpc(&mut self) -> Result<SocketAddr> {
        self.rpc.as_mut().unwrap().start().await
    }

    pub async fn sync(&mut self) -> Result<()> {
        self.node.lock().await.sync().await
    }

    pub async fn advance(&mut self) -> Result<()> {
        self.node.lock().await.advance().await
    }

    pub fn track(&self) -> Result<()> {
        let node = self.node.clone();
        spawn(async move {
            loop {
                node.lock().await.advance().await.unwrap();
                sleep(Duration::from_secs(10)).await;
            }
        });

        Ok(())
    }

    pub async fn call(&self, opts: &CallOpts, block: &Option<u64>) -> Result<Vec<u8>> {
        self.node.lock().await.call(opts, block)
    }

    pub async fn estimate_gas(&self, opts: &CallOpts) -> Result<u64> {
        self.node.lock().await.estimate_gas(opts)
    }

    pub async fn get_balance(&self, address: &Address, block: &Option<u64>) -> Result<U256> {
        self.node.lock().await.get_balance(address, block).await
    }

    pub async fn get_nonce(&self, address: &Address, block: &Option<u64>) -> Result<u64> {
        self.node.lock().await.get_nonce(address, block).await
    }

    pub async fn get_code(&self, address: &Address, block: &Option<u64>) -> Result<Vec<u8>> {
        self.node.lock().await.get_code(address, block).await
    }

    pub async fn get_storage_at(&self, address: &Address, slot: H256) -> Result<U256> {
        self.node.lock().await.get_storage_at(address, slot).await
    }

    pub async fn send_raw_transaction(&self, bytes: &Vec<u8>) -> Result<H256> {
        self.node.lock().await.send_raw_transaction(bytes).await
    }

    pub async fn get_transaction_receipt(
        &self,
        tx_hash: &H256,
    ) -> Result<Option<TransactionReceipt>> {
        self.node
            .lock()
            .await
            .get_transaction_receipt(tx_hash)
            .await
    }

    pub async fn get_transaction_by_hash(&self, tx_hash: &H256) -> Result<Option<Transaction>> {
        self.node
            .lock()
            .await
            .get_transaction_by_hash(tx_hash)
            .await
    }

    pub async fn get_gas_price(&self) -> Result<U256> {
        self.node.lock().await.get_gas_price()
    }

    pub async fn get_priority_fee(&self) -> Result<U256> {
        self.node.lock().await.get_priority_fee()
    }

    pub async fn get_block_number(&self) -> Result<u64> {
        self.node.lock().await.get_block_number()
    }

    pub async fn get_block_by_number(&self, block: &Option<u64>) -> Result<ExecutionBlock> {
        self.node.lock().await.get_block_by_number(block)
    }

    pub async fn get_block_by_hash(&self, hash: &Vec<u8>) -> Result<ExecutionBlock> {
        self.node.lock().await.get_block_by_hash(hash)
    }

    pub async fn chain_id(&self) -> u64 {
        self.node.lock().await.chain_id()
    }

    pub async fn get_header(&self) -> Header {
        self.node.lock().await.get_header().clone()
    }
}
