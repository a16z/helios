use std::path::PathBuf;
use std::sync::Arc;

use config::networks::Network;
use ethers::prelude::{Address, U256};
use ethers::types::{Filter, Log, Transaction, TransactionReceipt, H256};
use eyre::{eyre, Result};

use common::types::BlockTag;
use config::Config;
use consensus::{types::Header, ConsensusClient};
use execution::types::{CallOpts, ExecutionBlock};
use log::{info, warn};
use tokio::spawn;
use tokio::sync::RwLock;
use tokio::time::sleep;

use crate::database::{Database, FileDB};
use crate::node::Node;
use crate::rpc::Rpc;

#[derive(Default)]
pub struct ClientBuilder {
    network: Option<Network>,
    consensus_rpc: Option<String>,
    execution_rpc: Option<String>,
    checkpoint: Option<Vec<u8>>,
    rpc_port: Option<u16>,
    data_dir: Option<PathBuf>,
    config: Option<Config>,
    fallback: Option<String>,
    load_external_fallback: bool,
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
        let checkpoint = hex::decode(checkpoint).expect("cannot parse checkpoint");
        self.checkpoint = Some(checkpoint);
        self
    }

    pub fn rpc_port(mut self, port: u16) -> Self {
        self.rpc_port = Some(port);
        self
    }

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

    pub fn build(self) -> Result<Client<FileDB>> {
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
            checkpoint
        } else if let Some(config) = &self.config {
            config.checkpoint.clone()
        } else {
            base_config.checkpoint
        };

        let rpc_port = if self.rpc_port.is_some() {
            self.rpc_port
        } else if let Some(config) = &self.config {
            config.rpc_port
        } else {
            None
        };

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

        let config = Config {
            consensus_rpc,
            execution_rpc,
            checkpoint,
            rpc_port,
            data_dir,
            chain: base_config.chain,
            forks: base_config.forks,
            max_checkpoint_age: base_config.max_checkpoint_age,
            fallback,
            load_external_fallback,
        };

        Client::new(config)
    }
}

pub struct Client<DB: Database> {
    node: Arc<RwLock<Node>>,
    rpc: Option<Rpc>,
    db: Option<DB>,
    fallback: Option<String>,
    load_external_fallback: bool,
}

impl Client<FileDB> {
    fn new(config: Config) -> Result<Self> {
        let config = Arc::new(config);
        let node = Node::new(config.clone())?;
        let node = Arc::new(RwLock::new(node));

        let rpc = config.rpc_port.map(|port| Rpc::new(node.clone(), port));

        let data_dir = config.data_dir.clone();
        let db = data_dir.map(FileDB::new);

        Ok(Client {
            node,
            rpc,
            db,
            fallback: config.fallback.clone(),
            load_external_fallback: config.load_external_fallback,
        })
    }
}

impl<DB: Database> Client<DB> {
    pub async fn start(&mut self) -> Result<()> {
        if let Some(rpc) = &mut self.rpc {
            rpc.start().await?;
        }

        let res = self.node.write().await.sync().await;
        if let Err(err) = res {
            warn!(
                "failed to sync consensus node with checkpoint: {:?}. Error: {}",
                self.node.read().await.config.checkpoint,
                err
            );
            let fallback = if let Some(fallback) = &self.fallback {
                info!("attempting to load checkpoint from fallback {}", fallback);

                let checkpoint =
                    config::checkpoints::CheckpointFallbackList::fetch_checkpoint_from_api(
                        fallback,
                    )
                    .await
                    .map_err(|_| {
                        eyre::eyre!("Failed to fetch checkpoint from fallback: {}", fallback)
                    })?;

                info!(
                    "external fallbacks responded with checkpoint {:?}",
                    checkpoint
                );

                // Try to sync again with the new checkpoint by reconstructing the consensus client
                // We fail fast here since the node is unrecoverable at this point
                let config = self.node.read().await.config.clone();
                let consensus = ConsensusClient::new(
                    &config.consensus_rpc,
                    checkpoint.as_bytes(),
                    config.clone(),
                )?;
                self.node.write().await.consensus = consensus;
                self.node.write().await.sync().await?;

                Ok(())
            } else {
                Err("missing fallback")
            };

            if fallback.is_err() && self.load_external_fallback {
                info!("attempting to fetch checkpoint from external fallbacks...");
                // Fetch the checkpoint from the external fallbacks
                let mut list = config::checkpoints::CheckpointFallbackList::new();
                list.construct().await.map_err(|_| {
                    eyre::eyre!("Failed to construct external checkpoint sync fallbacks")
                })?;

                let checkpoint = if self.node.read().await.config.chain.chain_id == 5 {
                    list.fetch_latest_goerli_checkpoint().await.map_err(|_| {
                        eyre::eyre!(
                            "Failed to fetch latest goerli checkpoint from external fallbacks"
                        )
                    })?
                } else {
                    list.fetch_latest_mainnet_checkpoint().await.map_err(|_| {
                        eyre::eyre!(
                            "Failed to fetch latest mainnet checkpoint from external fallbacks"
                        )
                    })?
                };

                info!(
                    "external fallbacks responded with checkpoint {:?}",
                    checkpoint
                );

                // Try to sync again with the new checkpoint by reconstructing the consensus client
                // We fail fast here since the node is unrecoverable at this point
                let config = self.node.read().await.config.clone();
                let consensus = ConsensusClient::new(
                    &config.consensus_rpc,
                    checkpoint.as_bytes(),
                    config.clone(),
                )?;
                self.node.write().await.consensus = consensus;
                self.node.write().await.sync().await?;
            }
        }

        let node = self.node.clone();
        spawn(async move {
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
        let res = self.db.as_ref().map(|db| db.save_checkpoint(checkpoint));
        if res.is_some() && res.unwrap().is_err() {
            warn!("checkpoint save failed");
        }
    }

    pub async fn call(&self, opts: &CallOpts, block: BlockTag) -> Result<Vec<u8>> {
        self.node
            .read()
            .await
            .call(opts, block)
            .await
            .map_err(|err| err.into())
    }

    pub async fn estimate_gas(&self, opts: &CallOpts) -> Result<u64> {
        self.node
            .read()
            .await
            .estimate_gas(opts)
            .await
            .map_err(|err| err.into())
    }

    pub async fn get_balance(&self, address: &Address, block: BlockTag) -> Result<U256> {
        self.node.read().await.get_balance(address, block).await
    }

    pub async fn get_nonce(&self, address: &Address, block: BlockTag) -> Result<u64> {
        self.node.read().await.get_nonce(address, block).await
    }

    pub async fn get_code(&self, address: &Address, block: BlockTag) -> Result<Vec<u8>> {
        self.node.read().await.get_code(address, block).await
    }

    pub async fn get_storage_at(&self, address: &Address, slot: H256) -> Result<U256> {
        self.node.read().await.get_storage_at(address, slot).await
    }

    pub async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<H256> {
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

    pub async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>> {
        self.node.read().await.get_logs(filter).await
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
        block: BlockTag,
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
