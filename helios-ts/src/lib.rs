extern crate console_error_panic_hook;
extern crate web_sys;

use std::str::FromStr;

use common::types::BlockTag;
use ethers::types::{Address, Filter, H256};
use execution::types::CallOpts;
use wasm_bindgen::prelude::*;

use config::{networks, Config};

#[allow(unused_macros)]
macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

#[wasm_bindgen]
pub struct Client {
    inner: client::Client,
    chain_id: u64,
}

#[wasm_bindgen]
impl Client {
    #[wasm_bindgen(constructor)]
    pub fn new(
        execution_rpc: String,
        consensus_rpc: Option<String>,
        network: String,
        checkpoint: Option<String>,
    ) -> Self {
        console_error_panic_hook::set_once();

        let base = match network.as_str() {
            "mainnet" => networks::mainnet(),
            "goerli" => networks::goerli(),
            _ => panic!("invalid network"),
        };

        let chain_id = base.chain.chain_id;

        let checkpoint = Some(
            checkpoint
                .as_ref()
                .map(|c| c.strip_prefix("0x").unwrap_or(c.as_str()))
                .map(|c| hex::decode(c).unwrap())
                .unwrap_or(base.default_checkpoint),
        );

        let consensus_rpc = consensus_rpc.unwrap_or(base.consensus_rpc.unwrap());

        let config = Config {
            execution_rpc,
            consensus_rpc,
            checkpoint,

            chain: base.chain,
            forks: base.forks,

            ..Default::default()
        };

        let inner: client::Client = client::ClientBuilder::new().config(config).build().unwrap();

        Self { inner, chain_id }
    }

    #[wasm_bindgen]
    pub async fn sync(&mut self) {
        self.inner.start().await.unwrap()
    }

    #[wasm_bindgen]
    pub fn chain_id(&self) -> u32 {
        self.chain_id as u32
    }

    #[wasm_bindgen]
    pub async fn get_block_number(&self) -> u32 {
        self.inner.get_block_number().await.unwrap().as_u32()
    }

    #[wasm_bindgen]
    pub async fn get_balance(&self, addr: JsValue, block: JsValue) -> String {
        let addr: Address = serde_wasm_bindgen::from_value(addr).unwrap();
        let block: BlockTag = serde_wasm_bindgen::from_value(block).unwrap();
        self.inner
            .get_balance(&addr, block)
            .await
            .unwrap()
            .to_string()
    }

    #[wasm_bindgen]
    pub async fn get_transaction_by_hash(&self, hash: String) -> JsValue {
        let hash = H256::from_str(&hash).unwrap();
        let tx = self.inner.get_transaction_by_hash(&hash).await.unwrap();
        serde_wasm_bindgen::to_value(&tx).unwrap()
    }

    #[wasm_bindgen]
    pub async fn get_transaction_count(&self, addr: JsValue, block: JsValue) -> u32 {
        let addr: Address = serde_wasm_bindgen::from_value(addr).unwrap();
        let block: BlockTag = serde_wasm_bindgen::from_value(block).unwrap();
        self.inner.get_nonce(&addr, block).await.unwrap() as u32
    }

    #[wasm_bindgen]
    pub async fn get_block_transaction_count_by_hash(&self, hash: JsValue) -> u32 {
        let hash: H256 = serde_wasm_bindgen::from_value(hash).unwrap();
        self.inner
            .get_block_transaction_count_by_hash(&hash)
            .await
            .unwrap() as u32
    }

    #[wasm_bindgen]
    pub async fn get_block_transaction_count_by_number(&self, block: JsValue) -> u32 {
        let block: BlockTag = serde_wasm_bindgen::from_value(block).unwrap();
        self.inner
            .get_block_transaction_count_by_number(block)
            .await
            .unwrap() as u32
    }

    #[wasm_bindgen]
    pub async fn get_code(&self, addr: JsValue, block: JsValue) -> String {
        let addr: Address = serde_wasm_bindgen::from_value(addr).unwrap();
        let block: BlockTag = serde_wasm_bindgen::from_value(block).unwrap();
        let code = self.inner.get_code(&addr, block).await.unwrap();
        format!("0x{}", hex::encode(code))
    }

    #[wasm_bindgen]
    pub async fn call(&self, opts: JsValue, block: JsValue) -> String {
        let opts: CallOpts = serde_wasm_bindgen::from_value(opts).unwrap();
        let block: BlockTag = serde_wasm_bindgen::from_value(block).unwrap();
        let res = self.inner.call(&opts, block).await.unwrap();
        format!("0x{}", hex::encode(res))
    }

    #[wasm_bindgen]
    pub async fn estimate_gas(&self, opts: JsValue) -> u32 {
        let opts: CallOpts = serde_wasm_bindgen::from_value(opts).unwrap();
        self.inner.estimate_gas(&opts).await.unwrap() as u32
    }

    #[wasm_bindgen]
    pub async fn gas_price(&self) -> JsValue {
        let price = self.inner.get_gas_price().await.unwrap();
        serde_wasm_bindgen::to_value(&price).unwrap()
    }

    #[wasm_bindgen]
    pub async fn max_priority_fee_per_gas(&self) -> JsValue {
        let price = self.inner.get_priority_fee().await.unwrap();
        serde_wasm_bindgen::to_value(&price).unwrap()
    }

    #[wasm_bindgen]
    pub async fn send_raw_transaction(&self, tx: String) -> JsValue {
        let tx = hex::decode(tx).unwrap();
        let hash = self.inner.send_raw_transaction(&tx).await.unwrap();
        serde_wasm_bindgen::to_value(&hash).unwrap()
    }

    #[wasm_bindgen]
    pub async fn get_transaction_receipt(&self, tx: JsValue) -> JsValue {
        let tx: H256 = serde_wasm_bindgen::from_value(tx).unwrap();
        let receipt = self.inner.get_transaction_receipt(&tx).await.unwrap();
        serde_wasm_bindgen::to_value(&receipt).unwrap()
    }

    #[wasm_bindgen]
    pub async fn get_logs(&self, filter: JsValue) -> JsValue {
        let filter: Filter = serde_wasm_bindgen::from_value(filter).unwrap();
        let logs = self.inner.get_logs(&filter).await.unwrap();
        serde_wasm_bindgen::to_value(&logs).unwrap()
    }
}
