#[cfg(not(target_arch = "wasm32"))]
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use alloy::primitives::{Address, B256, U256};
use alloy::rpc::types::{Filter, Log, SyncStatus, Transaction, TransactionReceipt};
use eyre::{eyre, Result};
use tracing::{info, warn};
use zduny_wasm_timer::Delay;

use common::types::{Block, BlockTag};
use config::networks::Network;
use config::Config;
use consensus::database::Database;
use execution::types::CallOpts;

use crate::node::Node;

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
use crate::rpc::Rpc;

#[derive(Default)]
pub struct ClientBuilder {
    network: Option<Network>,
    consensus_rpc: Option<String>,
    execution_rpc: Option<String>,
    checkpoint: Option<Vec<u8>>,
    #[cfg(not(target_arch = "wasm32"))]
    rpc_bind_ip: Option<IpAddr>,
    #[cfg(not(target_arch = "wasm32"))]
    rpc_port: Option<u16>,
    #[cfg(not(target_arch = "wasm32"))]
    data_dir: Option<PathBuf>,
    config: Option<Config>,
    fallback: Option<String>,
    load_external_fallback: bool,
    strict_checkpoint_age: bool,
}

impl ClientBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn network(mut self, network: Network) -> Self {
        self.network = Some(network);
        self
    }

    pub fn consensus_rpc(mut self, consensus_rpc: &str) -> Self {
        self.consensus_rpc = Some(consensus_rpc.to_string());
        self
    }

    pub fn execution_rpc(mut self, execution_rpc: &str) -> Self {
        self.execution_rpc = Some(execution_rpc.to_string());
        self
    }

    pub fn checkpoint(mut self, checkpoint: &str) -> Self {
        let checkpoint = hex::decode(checkpoint.strip_prefix("0x").unwrap_or(checkpoint))
            .expect("cannot parse checkpoint");
        self.checkpoint = Some(checkpoint);
        self
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn rpc_bind_ip(mut self, ip: IpAddr) -> Self {
        self.rpc_bind_ip = Some(ip);
        self
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn rpc_port(mut self, port: u16) -> Self {
        self.rpc_port = Some(port);
        self
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn data_dir(mut self, data_dir: PathBuf) -> Self {
        self.data_dir = Some(data_dir);
        self
    }

    pub fn config(mut self, config: Config) -> Self {
        self.config = Some(config);
        self
    }

    pub fn fallback(mut self, fallback: &str) -> Self {
        self.fallback = Some(fallback.to_string());
        self
    }

    pub fn load_external_fallback(mut self) -> Self {
        self.load_external_fallback = true;
        self
    }

    pub fn strict_checkpoint_age(mut self) -> Self {
        self.strict_checkpoint_age = true;
        self
    }

    pub fn build<DB: Database>(self) -> Result<Client<DB>> {
        let base_config = if let Some(network) = self.network {
            network.to_base_config()
        } else {
            let config = self
                .config
                .as_ref()
                .ok_or(eyre!("missing network config"))?;
            config.to_base_config()
        };

        let consensus_rpc = self.consensus_rpc.unwrap_or_else(|| {
            self.config
                .as_ref()
                .expect("missing consensus rpc")
                .consensus_rpc
                .clone()
        });

        let execution_rpc = self.execution_rpc.unwrap_or_else(|| {
            self.config
                .as_ref()
                .expect("missing execution rpc")
                .execution_rpc
                .clone()
        });

        let checkpoint = if let Some(checkpoint) = self.checkpoint {
            Some(checkpoint)
        } else if let Some(config) = &self.config {
            config.checkpoint.clone()
        } else {
            None
        };

        let default_checkpoint = if let Some(config) = &self.config {
            config.default_checkpoint.clone()
        } else {
            base_config.default_checkpoint.clone()
        };

        #[cfg(not(target_arch = "wasm32"))]
        let rpc_bind_ip = if self.rpc_bind_ip.is_some() {
            self.rpc_bind_ip
        } else if let Some(config) = &self.config {
            config.rpc_bind_ip
        } else {
            None
        };

        #[cfg(not(target_arch = "wasm32"))]
        let rpc_port = if self.rpc_port.is_some() {
            self.rpc_port
        } else if let Some(config) = &self.config {
            config.rpc_port
        } else {
            None
        };

        #[cfg(not(target_arch = "wasm32"))]
        let data_dir = if self.data_dir.is_some() {
            self.data_dir
        } else if let Some(config) = &self.config {
            config.data_dir.clone()
        } else {
            None
        };

        let fallback = if self.fallback.is_some() {
            self.fallback
        } else if let Some(config) = &self.config {
            config.fallback.clone()
        } else {
            None
        };

        let load_external_fallback = if let Some(config) = &self.config {
            self.load_external_fallback || config.load_external_fallback
        } else {
            self.load_external_fallback
        };

        let strict_checkpoint_age = if let Some(config) = &self.config {
            self.strict_checkpoint_age || config.strict_checkpoint_age
        } else {
            self.strict_checkpoint_age
        };

        let config = Config {
            consensus_rpc,
            execution_rpc,
            checkpoint,
            default_checkpoint,
            #[cfg(not(target_arch = "wasm32"))]
            rpc_bind_ip,
            #[cfg(target_arch = "wasm32")]
            rpc_bind_ip: None,
            #[cfg(not(target_arch = "wasm32"))]
            rpc_port,
            #[cfg(target_arch = "wasm32")]
            rpc_port: None,
            #[cfg(not(target_arch = "wasm32"))]
            data_dir,
            #[cfg(target_arch = "wasm32")]
            data_dir: None,
            chain: base_config.chain,
            forks: base_config.forks,
            max_checkpoint_age: base_config.max_checkpoint_age,
            fallback,
            load_external_fallback,
            strict_checkpoint_age,
            database_type: None,
        };

        Client::<DB>::new(config)
    }
}

pub struct Client<DB: Database> {
    node: Arc<Node<DB>>,
    #[cfg(not(target_arch = "wasm32"))]
    rpc: Option<Rpc<DB>>,
}

impl<DB: Database> Client<DB> {
    fn new(config: Config) -> Result<Self> {
        let config = Arc::new(config);
        let node = Node::new(config.clone())?;
        let node = Arc::new(node);

        #[cfg(not(target_arch = "wasm32"))]
        let mut rpc: Option<Rpc<DB>> = None;

        #[cfg(not(target_arch = "wasm32"))]
        if config.rpc_bind_ip.is_some() || config.rpc_port.is_some() {
            rpc = Some(Rpc::new(node.clone(), config.rpc_bind_ip, config.rpc_port));
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

    pub async fn call(&self, opts: &CallOpts, block: BlockTag) -> Result<Vec<u8>> {
        self.node.call(opts, block).await.map_err(|err| err.into())
    }

    pub async fn estimate_gas(&self, opts: &CallOpts) -> Result<u64> {
        self.node.estimate_gas(opts).await.map_err(|err| err.into())
    }

    pub async fn get_balance(&self, address: &Address, block: BlockTag) -> Result<U256> {
        self.node.get_balance(address, block).await
    }

    pub async fn get_nonce(&self, address: &Address, block: BlockTag) -> Result<u64> {
        self.node.get_nonce(address, block).await
    }

    pub async fn get_block_transaction_count_by_hash(&self, hash: &B256) -> Result<u64> {
        self.node.get_block_transaction_count_by_hash(hash).await
    }

    pub async fn get_block_transaction_count_by_number(&self, block: BlockTag) -> Result<u64> {
        self.node.get_block_transaction_count_by_number(block).await
    }

    pub async fn get_code(&self, address: &Address, block: BlockTag) -> Result<Vec<u8>> {
        self.node.get_code(address, block).await
    }

    pub async fn get_storage_at(
        &self,
        address: &Address,
        slot: B256,
        block: BlockTag,
    ) -> Result<U256> {
        self.node.get_storage_at(address, slot, block).await
    }

    pub async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<B256> {
        self.node.send_raw_transaction(bytes).await
    }

    pub async fn get_transaction_receipt(
        &self,
        tx_hash: &B256,
    ) -> Result<Option<TransactionReceipt>> {
        self.node.get_transaction_receipt(tx_hash).await
    }

    pub async fn get_transaction_by_hash(&self, tx_hash: &B256) -> Option<Transaction> {
        self.node.get_transaction_by_hash(tx_hash).await
    }

    pub async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>> {
        self.node.get_logs(filter).await
    }

    pub async fn get_filter_changes(&self, filter_id: &U256) -> Result<bool> {
        self.node.uninstall_filter(filter_id).await
    }

    pub async fn uninstall_filter(&self, filter_id: &U256) -> Result<bool> {
        self.node.uninstall_filter(filter_id).await
    }

    pub async fn get_new_filter(&self, filter: &Filter) -> Result<U256> {
        self.node.get_new_filter(filter).await
    }

    pub async fn get_new_block_filter(&self) -> Result<U256> {
        self.node.get_new_block_filter().await
    }

    pub async fn get_new_pending_transaction_filter(&self) -> Result<U256> {
        self.node.get_new_pending_transaction_filter().await
    }

    pub async fn get_gas_price(&self) -> Result<U256> {
        self.node.get_gas_price().await
    }

    pub async fn get_priority_fee(&self) -> Result<U256> {
        self.node.get_priority_fee()
    }

    pub async fn get_block_number(&self) -> Result<U256> {
        self.node.get_block_number().await
    }

    pub async fn get_block_by_number(
        &self,
        block: BlockTag,
        full_tx: bool,
    ) -> Result<Option<Block>> {
        self.node.get_block_by_number(block, full_tx).await
    }

    pub async fn get_block_by_hash(&self, hash: &B256, full_tx: bool) -> Result<Option<Block>> {
        self.node.get_block_by_hash(hash, full_tx).await
    }

    pub async fn get_transaction_by_block_hash_and_index(
        &self,
        block_hash: &B256,
        index: u64,
    ) -> Option<Transaction> {
        self.node
            .get_transaction_by_block_hash_and_index(block_hash, index)
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
        loop {
            if let Ok(SyncStatus::None) = self.syncing().await {
                break;
            }

            Delay::new(Duration::from_millis(100)).await.unwrap();
        }
    }
}
