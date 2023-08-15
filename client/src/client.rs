use std::net::IpAddr;
use std::sync::Arc;

use config::networks::Network;
use ethers::prelude::{Address, U256};
use ethers::types::{
    FeeHistory, Filter, Log, SyncingStatus, Transaction, TransactionReceipt, H256,
};
use eyre::{eyre, Result};

use common::types::BlockTag;
use config::Config;
use execution::types::{CallOpts, ExecutionBlock};
use log::{info, warn};
use tokio::sync::RwLock;

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use tokio::spawn;
#[cfg(not(target_arch = "wasm32"))]
use tokio::time::sleep;

#[cfg(target_arch = "wasm32")]
use gloo_timers::callback::Interval;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local;

use crate::database::Database;
use crate::node::Node;

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
        };

        Client::new(config)
    }
}

pub struct Client<DB: Database> {
    node: Arc<RwLock<Node>>,
    #[cfg(not(target_arch = "wasm32"))]
    rpc: Option<Rpc>,
    db: DB,
    // fallback: Option<String>,
    // load_external_fallback: bool,
}

impl<DB: Database> Client<DB> {
    fn new(mut config: Config) -> Result<Self> {
        let db = DB::new(&config)?;
        if config.checkpoint.is_none() {
            let checkpoint = db.load_checkpoint()?;
            config.checkpoint = Some(checkpoint);
        }

        let config = Arc::new(config);
        let node = Node::new(config.clone())?;
        let node = Arc::new(RwLock::new(node));
        let mut rpc: Option<Rpc> = None;

        #[cfg(not(target_arch = "wasm32"))]
        if config.rpc_bind_ip.is_some() || config.rpc_port.is_some() {
            rpc = Some(Rpc::new(node.clone(), config.rpc_bind_ip, config.rpc_port));
        }

        Ok(Client {
            node,
            #[cfg(not(target_arch = "wasm32"))]
            rpc,
            db,
            // fallback: config.fallback.clone(),
            // load_external_fallback: config.load_external_fallback,
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(rpc) = &mut self.rpc {
            rpc.start().await?;
        }

        // let sync_res = self.node.write().await.sync().await;

        // if let Err(err) = sync_res {
        //     match err {
        //         NodeError::ConsensusSyncError(err) => match err.downcast_ref() {
        //             Some(ConsensusError::CheckpointTooOld) => {
        //                 warn!(
        //                     "failed to sync consensus node with checkpoint: 0x{}",
        //                     hex::encode(
        //                         self.node
        //                             .read()
        //                             .await
        //                             .config
        //                             .checkpoint
        //                             .clone()
        //                             .unwrap_or_default()
        //                     ),
        //                 );

        //                 let fallback = self.boot_from_fallback().await;
        //                 if fallback.is_err() && self.load_external_fallback {
        //                     self.boot_from_external_fallbacks().await?
        //                 } else if fallback.is_err() {
        //                     error!("Invalid checkpoint. Please update your checkpoint too a more recent block. Alternatively, set an explicit checkpoint fallback service url with the `-f` flag or use the configured external fallback services with `-l` (NOT RECOMMENDED). See https://github.com/a16z/helios#additional-options for more information.");
        //                     return Err(err);
        //                 }
        //             }
        //             _ => return Err(err),
        //         },
        //         _ => return Err(err.into()),
        //     }
        // }

        // self.save_last_checkpoint().await;

        self.start_advance_thread();

        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn start_advance_thread(&self) {
        let node = self.node.clone();
        spawn(async move {
            loop {
                node.write().await.advance().await;

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

    // async fn boot_from_fallback(&self) -> eyre::Result<()> {
    //     if let Some(fallback) = &self.fallback {
    //         info!(
    //             "attempting to load checkpoint from fallback \"{}\"",
    //             fallback
    //         );

    //         let checkpoint = CheckpointFallback::fetch_checkpoint_from_api(fallback)
    //             .await
    //             .map_err(|_| {
    //                 eyre::eyre!("Failed to fetch checkpoint from fallback \"{}\"", fallback)
    //             })?;

    //         info!(
    //             "external fallbacks responded with checkpoint 0x{:?}",
    //             checkpoint
    //         );

    //         // Try to sync again with the new checkpoint by reconstructing the consensus client
    //         // We fail fast here since the node is unrecoverable at this point
    //         let config = self.node.read().await.config.clone();
    //         let consensus =
    //             ConsensusClient::new(&config.consensus_rpc, checkpoint.as_bytes(), config.clone())?;
    //         self.node.write().await.consensus = consensus;
    //         self.node.write().await.sync().await?;

    //         Ok(())
    //     } else {
    //         Err(eyre::eyre!("no explicit fallback specified"))
    //     }
    // }

    // async fn boot_from_external_fallbacks(&self) -> eyre::Result<()> {
    //     info!("attempting to fetch checkpoint from external fallbacks...");
    //     // Build the list of external checkpoint fallback services
    //     let list = CheckpointFallback::new()
    //         .build()
    //         .await
    //         .map_err(|_| eyre::eyre!("Failed to construct external checkpoint sync fallbacks"))?;

    //     let checkpoint = if self.node.read().await.config.chain.chain_id == 5 {
    //         list.fetch_latest_checkpoint(&Network::GOERLI)
    //             .await
    //             .map_err(|_| {
    //                 eyre::eyre!("Failed to fetch latest goerli checkpoint from external fallbacks")
    //             })?
    //     } else {
    //         list.fetch_latest_checkpoint(&Network::MAINNET)
    //             .await
    //             .map_err(|_| {
    //                 eyre::eyre!("Failed to fetch latest mainnet checkpoint from external fallbacks")
    //             })?
    //     };

    //     info!(
    //         "external fallbacks responded with checkpoint {:?}",
    //         checkpoint
    //     );

    //     // Try to sync again with the new checkpoint by reconstructing the consensus client
    //     // We fail fast here since the node is unrecoverable at this point
    //     let config = self.node.read().await.config.clone();
    //     let consensus =
    //         ConsensusClient::new(&config.consensus_rpc, checkpoint.as_bytes(), config.clone())?;
    //     self.node.write().await.consensus = consensus;
    //     self.node.write().await.sync().await?;
    //     Ok(())
    // }

    /// Saves last checkpoint of the node.
    async fn save_last_checkpoint(&self) {
        let node = self.node.read().await;
        let checkpoint = node.consensus.checkpoint_recv.borrow().to_owned();

        if let Some(checkpoint) = checkpoint {
            info!("saving last checkpoint hash");
            let res = self.db.save_checkpoint(checkpoint);
            if res.is_err() {
                warn!("checkpoint save failed");
            }
        };
    }

    pub async fn shutdown(&self) {
        self.save_last_checkpoint().await;
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

    pub async fn get_coinbase(&self) -> Result<Address> {
        self.node.read().await.get_coinbase()
    }
}
