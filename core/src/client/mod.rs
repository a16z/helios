use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use alloy::primitives::{Address, Bytes, B256, U256};
use alloy::rpc::types::serde_helpers::JsonStorageKey;
use alloy::rpc::types::{Filter, FilterChanges, Log, SyncStatus};
use eyre::Result;
use tracing::{info, warn};

use helios_common::{fork_schedule::ForkSchedule, network_spec::NetworkSpec, types::BlockTag};

use crate::client::node::Node;
#[cfg(not(target_arch = "wasm32"))]
use crate::client::rpc::Rpc;
use crate::consensus::Consensus;
use crate::time::interval;

pub mod node;
#[cfg(not(target_arch = "wasm32"))]
pub mod rpc;

pub struct Client<N: NetworkSpec, C: Consensus<N::BlockResponse>> {
    node: Arc<Node<N, C>>,
    #[cfg(not(target_arch = "wasm32"))]
    rpc: Option<Rpc<N, C>>,
}

impl<N: NetworkSpec, C: Consensus<N::BlockResponse>> Client<N, C> {
    pub fn new(
        execution_rpc: &str,
        verifiable_api: Option<&str>,
        consensus: C,
        fork_schedule: ForkSchedule,
        #[cfg(not(target_arch = "wasm32"))] rpc_address: Option<SocketAddr>,
    ) -> Result<Self> {
        let node = Node::new(execution_rpc, verifiable_api, consensus, fork_schedule)?;
        let node = Arc::new(node);

        #[cfg(not(target_arch = "wasm32"))]
        let mut rpc: Option<Rpc<N, C>> = None;

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(rpc_address) = rpc_address {
            rpc = Some(Rpc::new(node.clone(), rpc_address));
        }

        Ok(Client {
            node,
            #[cfg(not(target_arch = "wasm32"))]
            rpc,
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(rpc) = &mut self.rpc {
            rpc.start().await?;
        }

        Ok(())
    }

    pub async fn shutdown(&self) {
        info!(target: "helios::client","shutting down");
        if let Err(err) = self.node.consensus.shutdown() {
            warn!(target: "helios::client", error = %err, "graceful shutdown failed");
        }
    }

    pub async fn call(&self, tx: &N::TransactionRequest, block: BlockTag) -> Result<Bytes> {
        self.node.call(tx, block).await.map_err(|err| err.into())
    }

    pub async fn estimate_gas(&self, tx: &N::TransactionRequest) -> Result<u64> {
        self.node.estimate_gas(tx).await.map_err(|err| err.into())
    }

    pub async fn get_balance(&self, address: Address, block: BlockTag) -> Result<U256> {
        self.node.get_balance(address, block).await
    }

    pub async fn get_nonce(&self, address: Address, block: BlockTag) -> Result<u64> {
        self.node.get_nonce(address, block).await
    }

    pub async fn get_block_transaction_count_by_hash(&self, hash: B256) -> Result<Option<u64>> {
        self.node.get_block_transaction_count_by_hash(hash).await
    }

    pub async fn get_block_transaction_count_by_number(
        &self,
        block: BlockTag,
    ) -> Result<Option<u64>> {
        self.node.get_block_transaction_count_by_number(block).await
    }

    pub async fn get_code(&self, address: Address, block: BlockTag) -> Result<Bytes> {
        self.node.get_code(address, block).await
    }

    pub async fn get_storage_at(
        &self,
        address: Address,
        slot: JsonStorageKey,
        block: BlockTag,
    ) -> Result<B256> {
        self.node.get_storage_at(address, slot, block).await
    }

    pub async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<B256> {
        self.node.send_raw_transaction(bytes).await
    }

    pub async fn get_transaction_receipt(
        &self,
        tx_hash: B256,
    ) -> Result<Option<N::ReceiptResponse>> {
        self.node.get_transaction_receipt(tx_hash).await
    }

    pub async fn get_block_receipts(
        &self,
        block: BlockTag,
    ) -> Result<Option<Vec<N::ReceiptResponse>>> {
        self.node.get_block_receipts(block).await
    }

    pub async fn get_transaction_by_hash(&self, tx_hash: B256) -> Option<N::TransactionResponse> {
        self.node.get_transaction_by_hash(tx_hash).await
    }

    pub async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>> {
        self.node.get_logs(filter).await
    }

    pub async fn get_filter_changes(&self, filter_id: U256) -> Result<FilterChanges> {
        self.node.get_filter_changes(filter_id).await
    }

    pub async fn get_filter_logs(&self, filter_id: U256) -> Result<Vec<Log>> {
        self.node.get_filter_logs(filter_id).await
    }

    pub async fn uninstall_filter(&self, filter_id: U256) -> Result<bool> {
        self.node.uninstall_filter(filter_id).await
    }

    pub async fn new_filter(&self, filter: &Filter) -> Result<U256> {
        self.node.new_filter(filter).await
    }

    pub async fn new_block_filter(&self) -> Result<U256> {
        self.node.new_block_filter().await
    }

    pub async fn new_pending_transaction_filter(&self) -> Result<U256> {
        self.node.new_pending_transaction_filter().await
    }

    pub async fn get_gas_price(&self) -> Result<U256> {
        self.node.get_gas_price().await
    }

    pub async fn get_priority_fee(&self) -> Result<U256> {
        self.node.get_priority_fee()
    }

    pub async fn blob_base_fee(&self, block: BlockTag) -> Result<U256> {
        self.node.blob_base_fee(block).await
    }

    pub async fn get_block_number(&self) -> Result<U256> {
        self.node.get_block_number().await
    }

    pub async fn client_version(&self) -> String {
        self.node.client_version().await
    }

    pub async fn get_block_by_number(
        &self,
        block: BlockTag,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        self.node.get_block_by_number(block, full_tx).await
    }

    pub async fn get_block_by_hash(
        &self,
        hash: B256,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        self.node.get_block_by_hash(hash, full_tx).await
    }

    pub async fn get_transaction_by_block_hash_and_index(
        &self,
        block_hash: B256,
        index: u64,
    ) -> Option<N::TransactionResponse> {
        self.node
            .get_transaction_by_block_hash_and_index(block_hash, index)
            .await
    }

    pub async fn get_transaction_by_block_number_and_index(
        &self,
        block: BlockTag,
        index: u64,
    ) -> Result<Option<N::TransactionResponse>> {
        self.node
            .get_transaction_by_block_number_and_index(block, index)
            .await
    }

    pub async fn chain_id(&self) -> u64 {
        self.node.chain_id()
    }

    pub async fn syncing(&self) -> Result<SyncStatus> {
        self.node.syncing().await
    }

    pub async fn get_coinbase(&self) -> Result<Address> {
        self.node.get_coinbase().await
    }

    pub async fn wait_synced(&self) {
        let mut interval = interval(Duration::from_millis(100));
        loop {
            interval.tick().await;
            if let Ok(SyncStatus::None) = self.syncing().await {
                break;
            }
        }
    }
}
