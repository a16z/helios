extern crate console_error_panic_hook;
extern crate web_sys;

use std::str::FromStr;

use alloy::hex::FromHex;
use alloy::primitives::{Address, B256};
use alloy::rpc::types::{Filter, TransactionRequest};
use eyre::Result;
use wasm_bindgen::prelude::*;

use helios_core::types::BlockTag;
use helios_ethereum::config::{networks, Config};
use helios_ethereum::database::{ConfigDB, Database};
use helios_ethereum::EthereumClientBuilder;

use crate::map_err;
use crate::storage::LocalStorageDB;

#[derive(Clone)]
pub enum DatabaseType {
    Memory(ConfigDB),
    LocalStorage(LocalStorageDB),
}

impl Database for DatabaseType {
    fn new(config: &Config) -> Result<Self> {
        // Implement this method based on the behavior of ConfigDB and LocalStorageDB
        match config.database_type.as_deref() {
            Some("config") => Ok(DatabaseType::Memory(ConfigDB::new(config)?)),
            Some("localstorage") => Ok(DatabaseType::LocalStorage(LocalStorageDB::new(config)?)),
            _ => Ok(DatabaseType::Memory(ConfigDB::new(config)?)),
        }
    }

    fn load_checkpoint(&self) -> Result<B256> {
        match self {
            DatabaseType::Memory(db) => db.load_checkpoint(),
            DatabaseType::LocalStorage(db) => db.load_checkpoint(),
        }
    }

    fn save_checkpoint(&self, checkpoint: B256) -> Result<()> {
        match self {
            DatabaseType::Memory(db) => db.save_checkpoint(checkpoint),
            DatabaseType::LocalStorage(db) => db.save_checkpoint(checkpoint),
        }
    }
}

#[wasm_bindgen]
pub struct EthereumClient {
    inner: helios_ethereum::EthereumClient<DatabaseType>,
    chain_id: u64,
}

#[wasm_bindgen]
impl EthereumClient {
    #[wasm_bindgen(constructor)]
    pub fn new(
        execution_rpc: String,
        consensus_rpc: Option<String>,
        network: String,
        checkpoint: Option<String>,
        db_type: String,
    ) -> Result<EthereumClient, JsError> {
        console_error_panic_hook::set_once();

        let base = match network.as_str() {
            "mainnet" => networks::mainnet(),
            "sepolia" => networks::sepolia(),
            "holesky" => networks::holesky(),
            other => Err(JsError::new(&format!("invalid network: {}", other)))?,
        };

        let chain_id = base.chain.chain_id;

        let checkpoint = Some(
            checkpoint
                .as_ref()
                .map(|c| c.strip_prefix("0x").unwrap_or(c.as_str()))
                .and_then(|c| B256::from_hex(c).ok())
                .unwrap_or(base.default_checkpoint),
        );

        let consensus_rpc = if let Some(rpc) = consensus_rpc {
            rpc
        } else {
            base.consensus_rpc
                .ok_or(JsError::new("consensus rpc not found"))?
        };

        let config = Config {
            execution_rpc,
            consensus_rpc,
            checkpoint,

            chain: base.chain,
            forks: base.forks,

            database_type: Some(db_type),
            ..Default::default()
        };

        let inner = map_err(EthereumClientBuilder::new().config(config).build())?;

        Ok(Self { inner, chain_id })
    }

    #[wasm_bindgen]
    pub async fn sync(&mut self) -> Result<(), JsError> {
        map_err(self.inner.start().await)
    }

    #[wasm_bindgen]
    pub async fn wait_synced(&self) {
        self.inner.wait_synced().await;
    }

    #[wasm_bindgen]
    pub fn chain_id(&self) -> u32 {
        self.chain_id as u32
    }

    #[wasm_bindgen]
    pub async fn get_block_number(&self) -> Result<u32, JsError> {
        map_err(self.inner.get_block_number().await).map(|v| v.to())
    }

    #[wasm_bindgen]
    pub async fn get_balance(&self, addr: JsValue, block: JsValue) -> Result<String, JsError> {
        let addr: Address = serde_wasm_bindgen::from_value(addr)?;
        let block: BlockTag = serde_wasm_bindgen::from_value(block)?;
        let res = map_err(self.inner.get_balance(addr, block).await);
        res.map(|v| v.to_string())
    }

    #[wasm_bindgen]
    pub async fn get_transaction_by_hash(&self, hash: String) -> Result<JsValue, JsError> {
        let hash = B256::from_str(&hash)?;
        let tx = self.inner.get_transaction_by_hash(hash).await;
        Ok(serde_wasm_bindgen::to_value(&tx)?)
    }

    #[wasm_bindgen]
    pub async fn get_transaction_by_block_hash_and_index(
        &self,
        hash: JsValue,
        index: JsValue,
    ) -> Result<JsValue, JsError> {
        let hash: B256 = serde_wasm_bindgen::from_value(hash)?;
        let index: u64 = serde_wasm_bindgen::from_value(index)?;
        let tx = self
            .inner
            .get_transaction_by_block_hash_and_index(hash, index)
            .await;
        Ok(serde_wasm_bindgen::to_value(&tx)?)
    }

    #[wasm_bindgen]
    pub async fn get_transaction_count(
        &self,
        addr: JsValue,
        block: JsValue,
    ) -> Result<u32, JsError> {
        let addr: Address = serde_wasm_bindgen::from_value(addr)?;
        let block: BlockTag = serde_wasm_bindgen::from_value(block)?;
        Ok(map_err(self.inner.get_nonce(addr, block).await)? as u32)
    }

    #[wasm_bindgen]
    pub async fn get_block_transaction_count_by_hash(&self, hash: JsValue) -> Result<u32, JsError> {
        let hash: B256 = serde_wasm_bindgen::from_value(hash)?;
        let count = map_err(self.inner.get_block_transaction_count_by_hash(hash).await)?;
        Ok(count as u32)
    }

    #[wasm_bindgen]
    pub async fn get_block_transaction_count_by_number(
        &self,
        block: JsValue,
    ) -> Result<u32, JsError> {
        let block: BlockTag = serde_wasm_bindgen::from_value(block)?;
        let res = self
            .inner
            .get_block_transaction_count_by_number(block)
            .await;
        Ok(map_err(res)? as u32)
    }

    #[wasm_bindgen]
    pub async fn get_block_by_number(
        &self,
        block: JsValue,
        full_tx: bool,
    ) -> Result<JsValue, JsError> {
        let block: BlockTag = serde_wasm_bindgen::from_value(block)?;
        let block = map_err(self.inner.get_block_by_number(block, full_tx).await)?;
        Ok(serde_wasm_bindgen::to_value(&block)?)
    }

    #[wasm_bindgen]
    pub async fn get_code(&self, addr: JsValue, block: JsValue) -> Result<String, JsError> {
        let addr: Address = serde_wasm_bindgen::from_value(addr)?;
        let block: BlockTag = serde_wasm_bindgen::from_value(block)?;
        let code = map_err(self.inner.get_code(addr, block).await)?;
        Ok(format!("0x{}", hex::encode(code)))
    }

    #[wasm_bindgen]
    pub async fn call(&self, opts: JsValue, block: JsValue) -> Result<String, JsError> {
        let opts: TransactionRequest = serde_wasm_bindgen::from_value(opts)?;
        let block: BlockTag = serde_wasm_bindgen::from_value(block)?;
        let res = map_err(self.inner.call(&opts, block).await)?;
        Ok(format!("0x{}", hex::encode(res)))
    }

    #[wasm_bindgen]
    pub async fn estimate_gas(&self, opts: JsValue) -> Result<u32, JsError> {
        let opts: TransactionRequest = serde_wasm_bindgen::from_value(opts)?;
        Ok(map_err(self.inner.estimate_gas(&opts).await)? as u32)
    }

    #[wasm_bindgen]
    pub async fn gas_price(&self) -> Result<JsValue, JsError> {
        let price = map_err(self.inner.get_gas_price().await)?;
        Ok(serde_wasm_bindgen::to_value(&price)?)
    }

    #[wasm_bindgen]
    pub async fn max_priority_fee_per_gas(&self) -> Result<JsValue, JsError> {
        let price = map_err(self.inner.get_priority_fee().await)?;
        Ok(serde_wasm_bindgen::to_value(&price)?)
    }

    #[wasm_bindgen]
    pub async fn send_raw_transaction(&self, tx: String) -> Result<JsValue, JsError> {
        let tx = hex::decode(tx)?;
        let hash = map_err(self.inner.send_raw_transaction(&tx).await)?;
        Ok(serde_wasm_bindgen::to_value(&hash)?)
    }

    #[wasm_bindgen]
    pub async fn get_transaction_receipt(&self, tx: JsValue) -> Result<JsValue, JsError> {
        let tx: B256 = serde_wasm_bindgen::from_value(tx)?;
        let receipt = map_err(self.inner.get_transaction_receipt(tx).await)?;
        Ok(serde_wasm_bindgen::to_value(&receipt)?)
    }

    #[wasm_bindgen]
    pub async fn get_logs(&self, filter: JsValue) -> Result<JsValue, JsError> {
        let filter: Filter = serde_wasm_bindgen::from_value(filter)?;
        let logs = map_err(self.inner.get_logs(&filter).await)?;
        Ok(serde_wasm_bindgen::to_value(&logs)?)
    }

    #[wasm_bindgen]
    pub async fn client_version(&self) -> String {
        self.inner.client_version().await
    }
}
