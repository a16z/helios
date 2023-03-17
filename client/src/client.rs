use std::sync::Arc;

use config::networks::Network;
use consensus::errors::ConsensusError;
use ethers::prelude::{Address, U256};
use ethers::types::{
    FeeHistory, Filter, Log, SyncingStatus, Transaction, TransactionReceipt, H256,
};
use eyre::Result;

use common::types::BlockTag;
use config::{CheckpointFallback, Config};
use consensus::{types::Header, ConsensusClient};
use execution::rpc::ExecutionRpc;
use execution::types::{CallOpts, ExecutionBlock};
use log::{error, info, warn};
use tokio::sync::RwLock;

#[cfg(not(target_arch = "wasm32"))]
use tokio::spawn;
#[cfg(not(target_arch = "wasm32"))]
use tokio::time::sleep;

#[cfg(target_arch = "wasm32")]
use gloo_timers::callback::Interval;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local;

use crate::database::Database;
use crate::errors::NodeError;
use crate::node::Node;

#[cfg(not(target_arch = "wasm32"))]
use crate::rpc::Rpc;

pub struct Client<DB: Database, R: ExecutionRpc> {
    node: Arc<RwLock<Node<R>>>,
    #[cfg(not(target_arch = "wasm32"))]
    rpc: Option<Rpc<R>>,
    db: DB,
    fallback: Option<String>,
    load_external_fallback: bool,
    ws: bool,
    http: bool,
}

impl<DB: Database, R: ExecutionRpc> Client<DB, R> {
    pub fn new(mut config: Config) -> Result<Self> {
        let db = DB::new(&config)?;
        if config.checkpoint.is_none() {
            let checkpoint = db.load_checkpoint()?;
            config.checkpoint = Some(checkpoint);
        }

        let config = Arc::new(config);

        let node = Node::new(config.clone())?;
        let node = Arc::new(RwLock::new(node));

        #[cfg(not(target_arch = "wasm32"))]
        let rpc = config
            .rpc_port
            .map(|port| Rpc::new(node.clone(), config.with_http, config.with_ws, port));

        Ok(Client {
            node,
            #[cfg(not(target_arch = "wasm32"))]
            rpc,
            db,
            fallback: config.fallback.clone(),
            load_external_fallback: config.load_external_fallback,
            ws: config.with_ws,
            http: config.with_http,
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(rpc) = &mut self.rpc {
            if self.ws {
                rpc.start_ws().await?;
            }
            if self.http {
                rpc.start_http().await?;
            }
        }

        let sync_res = self.node.write().await.sync().await;

        if let Err(err) = sync_res {
            match err {
                NodeError::ConsensusSyncError(err) => match err.downcast_ref() {
                    Some(ConsensusError::CheckpointTooOld) => {
                        warn!(
                            "failed to sync consensus node with checkpoint: 0x{}",
                            hex::encode(
                                self.node
                                    .read()
                                    .await
                                    .config
                                    .checkpoint
                                    .clone()
                                    .unwrap_or_default()
                            ),
                        );

                        let fallback = self.boot_from_fallback().await;
                        if fallback.is_err() && self.load_external_fallback {
                            self.boot_from_external_fallbacks().await?
                        } else if fallback.is_err() {
                            error!("Invalid checkpoint. Please update your checkpoint too a more recent block. Alternatively, set an explicit checkpoint fallback service url with the `-f` flag or use the configured external fallback services with `-l` (NOT RECOMMENDED). See https://github.com/a16z/helios#additional-options for more information.");
                            return Err(err);
                        }
                    }
                    _ => return Err(err),
                },
                _ => return Err(err.into()),
            }
        }

        self.start_advance_thread();

        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn start_advance_thread(&self) {
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
    }

    #[cfg(target_arch = "wasm32")]
    fn start_advance_thread(&self) {
        let node = self.node.clone();
        Interval::new(12000, move || {
            let node = node.clone();
            spawn_local(async move {
                let res = node.write().await.advance().await;
                if let Err(err) = res {
                    warn!("consensus error: {}", err);
                }
            });
        })
        .forget();
    }

    async fn boot_from_fallback(&self) -> eyre::Result<()> {
        if let Some(fallback) = &self.fallback {
            info!(
                "attempting to load checkpoint from fallback \"{}\"",
                fallback
            );

            let checkpoint = CheckpointFallback::fetch_checkpoint_from_api(fallback)
                .await
                .map_err(|_| {
                    eyre::eyre!("Failed to fetch checkpoint from fallback \"{}\"", fallback)
                })?;

            info!(
                "external fallbacks responded with checkpoint 0x{:?}",
                checkpoint
            );

            // Try to sync again with the new checkpoint by reconstructing the consensus client
            // We fail fast here since the node is unrecoverable at this point
            let config = self.node.read().await.config.clone();
            let consensus =
                ConsensusClient::new(&config.consensus_rpc, checkpoint.as_bytes(), config.clone())?;
            self.node.write().await.consensus = consensus;
            self.node.write().await.sync().await?;

            Ok(())
        } else {
            Err(eyre::eyre!("no explicit fallback specified"))
        }
    }

    async fn boot_from_external_fallbacks(&self) -> eyre::Result<()> {
        info!("attempting to fetch checkpoint from external fallbacks...");
        // Build the list of external checkpoint fallback services
        let list = CheckpointFallback::new()
            .build()
            .await
            .map_err(|_| eyre::eyre!("Failed to construct external checkpoint sync fallbacks"))?;

        let checkpoint = if self.node.read().await.config.chain.chain_id == 5 {
            list.fetch_latest_checkpoint(&Network::GOERLI)
                .await
                .map_err(|_| {
                    eyre::eyre!("Failed to fetch latest goerli checkpoint from external fallbacks")
                })?
        } else {
            list.fetch_latest_checkpoint(&Network::MAINNET)
                .await
                .map_err(|_| {
                    eyre::eyre!("Failed to fetch latest mainnet checkpoint from external fallbacks")
                })?
        };

        info!(
            "external fallbacks responded with checkpoint {:?}",
            checkpoint
        );

        // Try to sync again with the new checkpoint by reconstructing the consensus client
        // We fail fast here since the node is unrecoverable at this point
        let config = self.node.read().await.config.clone();
        let consensus =
            ConsensusClient::new(&config.consensus_rpc, checkpoint.as_bytes(), config.clone())?;
        self.node.write().await.consensus = consensus;
        self.node.write().await.sync().await?;
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

    pub async fn call(&self, opts: &CallOpts, block: BlockTag) -> eyre::Result<Vec<u8>> {
        self.node
            .read()
            .await
            .call(opts, block)
            .await
            .map_err(|err| err.into())
    }

    pub async fn estimate_gas(&self, opts: &CallOpts) -> eyre::Result<u64> {
        self.node
            .read()
            .await
            .estimate_gas(opts)
            .await
            .map_err(|err| err.into())
    }

    pub async fn get_balance(&self, address: &Address, block: BlockTag) -> eyre::Result<U256> {
        self.node.read().await.get_balance(address, block).await
    }

    pub async fn get_nonce(&self, address: &Address, block: BlockTag) -> eyre::Result<u64> {
        self.node.read().await.get_nonce(address, block).await
    }

    pub async fn get_block_transaction_count_by_hash(&self, hash: &Vec<u8>) -> Result<u64> {
        self.node
            .read()
            .await
            .get_block_transaction_count_by_hash(hash)
    }

    pub async fn get_block_transaction_count_by_number(&self, block: BlockTag) -> Result<u64> {
        self.node
            .read()
            .await
            .get_block_transaction_count_by_number(block)
    }

    pub async fn get_code(&self, address: &Address, block: BlockTag) -> Result<Vec<u8>> {
        self.node.read().await.get_code(address, block).await
    }

    pub async fn get_storage_at(
        &self,
        address: &Address,
        slot: H256,
        block: BlockTag,
    ) -> Result<U256> {
        self.node
            .read()
            .await
            .get_storage_at(address, slot, block)
            .await
    }

    pub async fn send_raw_transaction(&self, bytes: &[u8]) -> eyre::Result<H256> {
        self.node.read().await.send_raw_transaction(bytes).await
    }

    pub async fn get_transaction_receipt(
        &self,
        tx_hash: &H256,
    ) -> eyre::Result<Option<TransactionReceipt>> {
        self.node
            .read()
            .await
            .get_transaction_receipt(tx_hash)
            .await
    }

    pub async fn get_transaction_by_hash(
        &self,
        tx_hash: &H256,
    ) -> eyre::Result<Option<Transaction>> {
        self.node
            .read()
            .await
            .get_transaction_by_hash(tx_hash)
            .await
    }

    pub async fn get_logs(&self, filter: &Filter) -> eyre::Result<Vec<Log>> {
        self.node.read().await.get_logs(filter).await
    }

    pub async fn get_gas_price(&self) -> eyre::Result<U256> {
        self.node.read().await.get_gas_price()
    }

    pub async fn get_priority_fee(&self) -> eyre::Result<U256> {
        self.node.read().await.get_priority_fee()
    }

    pub async fn get_block_number(&self) -> eyre::Result<u64> {
        self.node.read().await.get_block_number()
    }

    pub async fn get_fee_history(
        &self,
        block_count: u64,
        last_block: u64,
        reward_percentiles: &[f64],
    ) -> Result<Option<FeeHistory>> {
        self.node
            .read()
            .await
            .get_fee_history(block_count, last_block, reward_percentiles)
            .await
    }

    pub async fn get_block_by_number(
        &self,
        block: BlockTag,
        full_tx: bool,
    ) -> eyre::Result<Option<ExecutionBlock>> {
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
    ) -> eyre::Result<Option<ExecutionBlock>> {
        self.node
            .read()
            .await
            .get_block_by_hash(hash, full_tx)
            .await
    }

    pub async fn get_transaction_by_block_hash_and_index(
        &self,
        block_hash: &Vec<u8>,
        index: usize,
    ) -> Result<Option<Transaction>> {
        self.node
            .read()
            .await
            .get_transaction_by_block_hash_and_index(block_hash, index)
            .await
    }

    pub async fn chain_id(&self) -> u64 {
        self.node.read().await.chain_id()
    }

    pub async fn syncing(&self) -> Result<SyncingStatus> {
        self.node.read().await.syncing()
    }

    pub async fn get_header(&self) -> Result<Header> {
        self.node.read().await.get_header()
    }

    pub async fn get_coinbase(&self) -> Result<Address> {
        self.node.read().await.get_coinbase()
    }
}
