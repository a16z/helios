use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::exit;
use std::sync::Arc;
use std::time::Duration;

use ethers::prelude::{Address, U256};
use ethers::types::{Transaction, TransactionReceipt, H256};
use eyre::Result;

use config::Config;
use consensus::types::Header;
use execution::types::{CallOpts, ExecutionBlock};
use futures::executor::block_on;
use log::{info, warn};
use tokio::spawn;
use tokio::sync::Mutex;
use tokio::time::sleep;

use crate::node::{BlockTag, Node};
use crate::rpc::Rpc;

pub struct Client {
    node: Arc<Mutex<Node>>,
    rpc: Option<Rpc>,
}

impl Client {
    pub async fn new(config: Config) -> Result<Self> {
        let config = Arc::new(config);
        let node = Node::new(config.clone()).await?;
        let node = Arc::new(Mutex::new(node));

        let node_moved = node.clone();
        let name = config.machine.data_dir.clone();
        ctrlc::set_handler(move || {
            println!(""); // avoid ctrl-c showing up to the left of logs
            info!("shutting down");

            let res = save_last_checkpoint(node_moved.clone(), name.clone());
            if res.is_err() {
                warn!("could not save checkpoint")
            }

            exit(0);
        })?;

        let rpc = if let Some(port) = config.general.rpc_port {
            Some(Rpc::new(node.clone(), port))
        } else {
            None
        };

        Ok(Client { node, rpc })
    }

    pub async fn start(&mut self) -> Result<()> {
        self.rpc.as_mut().unwrap().start().await?;
        self.node.lock().await.sync().await?;

        let node = self.node.clone();
        spawn(async move {
            loop {
                let res = node.lock().await.advance().await;
                if let Err(err) = res {
                    warn!("{}", err);
                }

                sleep(Duration::from_secs(10)).await;
            }
        });

        Ok(())
    }

    pub async fn call(&self, opts: &CallOpts, block: &BlockTag) -> Result<Vec<u8>> {
        self.node.lock().await.call(opts, block)
    }

    pub async fn estimate_gas(&self, opts: &CallOpts) -> Result<u64> {
        self.node.lock().await.estimate_gas(opts)
    }

    pub async fn get_balance(&self, address: &Address, block: &BlockTag) -> Result<U256> {
        self.node.lock().await.get_balance(address, block).await
    }

    pub async fn get_nonce(&self, address: &Address, block: &BlockTag) -> Result<u64> {
        self.node.lock().await.get_nonce(address, block).await
    }

    pub async fn get_code(&self, address: &Address, block: &BlockTag) -> Result<Vec<u8>> {
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

    pub async fn get_block_by_number(&self, block: &BlockTag) -> Result<ExecutionBlock> {
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

fn save_last_checkpoint(node: Arc<Mutex<Node>>, data_dir: Option<PathBuf>) -> Result<()> {
    let node = block_on(node.lock());
    let checkpoint = node.get_last_checkpoint();
    let checkpoint = if let Some(checkpoint) = checkpoint {
        checkpoint
    } else {
        return Ok(());
    };

    let data_dir = if let Some(data_dir) = data_dir {
        data_dir
    } else {
        return Ok(());
    };

    info!(
        "saving last checkpoint               hash={}",
        hex::encode(&checkpoint)
    );

    fs::create_dir_all(&data_dir)?;

    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(data_dir.join("checkpoint"))?;

    f.write_all(checkpoint.as_slice())?;

    Ok(())
}
