extern crate console_error_panic_hook;
extern crate web_sys;

use std::str::FromStr;

use alloy::primitives::{Address, B256};
use alloy::rpc::types::{Filter, TransactionRequest};
use wasm_bindgen::prelude::*;

use helios_core::types::BlockTag;
use helios_opstack::config::{Config, Network, NetworkConfig};
use helios_opstack::OpStackClientBuilder;

use crate::map_err;

#[wasm_bindgen]
pub struct OpStackClient {
    inner: helios_opstack::OpStackClient,
    chain_id: u64,
}

#[wasm_bindgen]
impl OpStackClient {
    #[wasm_bindgen(constructor)]
    pub fn new(execution_rpc: String, network: String) -> Result<OpStackClient, JsError> {
        console_error_panic_hook::set_once();

        let network_config = match network.as_str() {
            "optimism" => NetworkConfig::from(Network::Optimism),
            "base" => NetworkConfig::from(Network::Base),
            other => Err(JsError::new(&format!("invalid network: {}", other)))?,
        };

        let chain_id = network_config.chain.chain_id;
        let consensus_rpc = network_config
            .consensus_rpc
            .ok_or(JsError::new("consensus rpc not found"))?;

        let config = Config {
            execution_rpc: execution_rpc.parse()?,
            consensus_rpc,
            chain: network_config.chain,
            rpc_socket: None,
        };

        let inner = map_err(OpStackClientBuilder::new().config(config).build())?;

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
}
