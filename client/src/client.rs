use std::sync::Arc;

use ethers::prelude::{Address, U256};
use ethers::types::{Transaction, TransactionReceipt, H256};
use eyre::Result;

use config::Config;
use consensus::types::Header;
use execution::types::{CallOpts, ExecutionBlock};

use crate::node::Node;

pub struct Client {
    node: Node,
}

impl Client {
    pub async fn new(config: Arc<Config>) -> Result<Self> {
        Ok(Client {
            node: Node::new(config).await?,
        })
    }

    pub async fn sync(&mut self) -> Result<()> {
        self.node.sync().await
    }

    pub async fn advance(&mut self) -> Result<()> {
        self.node.advance().await
    }

    pub fn call(&self, opts: &CallOpts, block: &Option<u64>) -> Result<Vec<u8>> {
        self.node.call(opts, block)
    }

    pub fn estimate_gas(&self, opts: &CallOpts) -> Result<u64> {
        self.node.estimate_gas(opts)
    }

    pub async fn get_balance(&self, address: &Address, block: &Option<u64>) -> Result<U256> {
        self.node.get_balance(address, block).await
    }

    pub async fn get_nonce(&self, address: &Address, block: &Option<u64>) -> Result<u64> {
        self.node.get_nonce(address, block).await
    }

    pub async fn get_code(&self, address: &Address, block: &Option<u64>) -> Result<Vec<u8>> {
        self.node.get_code(address, block).await
    }

    pub async fn get_storage_at(&self, address: &Address, slot: H256) -> Result<U256> {
        self.node.get_storage_at(address, slot).await
    }

    pub async fn send_raw_transaction(&self, bytes: &Vec<u8>) -> Result<H256> {
        self.node.send_raw_transaction(bytes).await
    }

    pub async fn get_transaction_receipt(
        &self,
        tx_hash: &H256,
    ) -> Result<Option<TransactionReceipt>> {
        self.node.get_transaction_receipt(tx_hash).await
    }

    pub async fn get_transaction_by_hash(&self, tx_hash: &H256) -> Result<Option<Transaction>> {
        self.node.get_transaction_by_hash(tx_hash).await
    }

    pub fn get_gas_price(&self) -> Result<U256> {
        self.node.get_gas_price()
    }

    pub fn get_priority_fee(&self) -> Result<U256> {
        self.node.get_priority_fee()
    }

    pub fn get_block_number(&self) -> Result<u64> {
        self.node.get_block_number()
    }

    pub fn get_block_by_number(&self, block: &Option<u64>) -> Result<ExecutionBlock> {
        self.node.get_block_by_number(block)
    }

    pub fn get_block_by_hash(&self, hash: &Vec<u8>) -> Result<ExecutionBlock> {
        self.node.get_block_by_hash(hash)
    }

    pub fn chain_id(&self) -> u64 {
        self.node.chain_id()
    }

    pub fn get_header(&self) -> &Header {
        self.node.get_header()
    }
}
