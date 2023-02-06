extern crate console_error_panic_hook;
extern crate web_sys;

use std::str::FromStr;

use common::types::BlockTag;
use ethers::types::{Address, H256};
use execution::types::CallOpts;
use wasm_bindgen::prelude::*;

use client::database::ConfigDB;
use config::{networks, Config};

#[allow(unused_macros)]
macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

#[wasm_bindgen]
pub struct Client {
    inner: client::Client<ConfigDB>,
    chain_id: u32,
}

#[wasm_bindgen]
impl Client {
    #[wasm_bindgen(constructor)]
    pub fn new(consensus_rpc: &str, execution_rpc: &str) -> Self {
        console_error_panic_hook::set_once();

        // TODO: handle different networks
        let chain_id = 1;
        let base = networks::mainnet();

        let config = Config {
            checkpoint: Some(base.default_checkpoint.clone()),
            execution_rpc: execution_rpc.to_string(),
            consensus_rpc: consensus_rpc.to_string(),
            chain: base.chain,
            forks: base.forks,

            ..Default::default()
        };

        let inner: client::Client<ConfigDB> =
            client::ClientBuilder::new().config(config).build().unwrap();

        Self { inner, chain_id }
    }

    #[wasm_bindgen]
    pub async fn sync(&mut self) {
        self.inner.start().await.unwrap()
    }

    #[wasm_bindgen]
    pub fn chain_id(&self) -> u32 {
        self.chain_id
    }

    #[wasm_bindgen]
    pub async fn get_block_number(&self) -> u32 {
        self.inner.get_block_number().await.unwrap() as u32
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
            .get_block_transaction_count_by_hash(&hash.as_bytes().to_vec())
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
}
