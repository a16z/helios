use std::path::PathBuf;

extern crate console_error_panic_hook;
extern crate web_sys;

use client::database::FileDB;
use config::{networks, Config};
use wasm_bindgen::prelude::*;

#[allow(unused_macros)]
macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

#[wasm_bindgen]
pub struct Client {
    node: client::Client<FileDB>,
}

#[wasm_bindgen]
impl Client {
    #[wasm_bindgen(constructor)]
    pub fn new(consensus_rpc: &str, execution_rpc: &str) -> Self {
        console_error_panic_hook::set_once();

        let base = networks::mainnet();
        let config = Config {
            checkpoint: base.checkpoint.clone(),
            execution_rpc: execution_rpc.to_string(),
            consensus_rpc: consensus_rpc.to_string(),

            rpc_port: None,
            fallback: None,
            load_external_fallback: false,
            strict_checkpoint_age: false,
            data_dir: Some(PathBuf::new()),
            max_checkpoint_age: u64::MAX,
            chain: base.chain,
            forks: base.forks,
        };

        let node: client::Client<FileDB> =
            client::ClientBuilder::new().config(config).build().unwrap();

        Self { node }
    }

    #[wasm_bindgen]
    pub async fn sync(&mut self) {
        self.node.start().await.unwrap()
    }

    #[wasm_bindgen]
    pub async fn advance(&mut self) {
        // self.node.advance().await.unwrap()
    }

    #[wasm_bindgen]
    pub async fn request(&self, req: &JsValue) {
        log!("{:?}", req);
    }

    #[wasm_bindgen]
    pub async fn chain_id(&self) -> u32 {
        self.node.chain_id().await as u32
    }

    #[wasm_bindgen]
    pub async fn get_block_number(&self) -> u32 {
        self.node.get_block_number().await.unwrap() as u32
    }
}

// #[wasm_bindgen]
// pub struct Node {
//     consensus: ConsensusClient<NimbusRpc>,
//     execution: ExecutionClient<HttpRpc>,
//     payloads: BTreeMap<u64, ExecutionPayload>,
//     config: Arc<Config>,
// }
//
// #[wasm_bindgen]
// impl Node {
//     #[wasm_bindgen(constructor)]
//     pub fn new(consensus_rpc: &str, execution_rpc: &str) -> Self {
//         console_error_panic_hook::set_once();
//
//         let base = networks::mainnet();
//         let config = Arc::new(Config {
//             checkpoint: base.checkpoint.clone(),
//             consensus_rpc: consensus_rpc.to_string(),
//             rpc_port: None,
//
//             fallback: None,
//             load_external_fallback: false,
//             strict_checkpoint_age: false,
//             data_dir: None,
//             execution_rpc: "".to_string(),
//             max_checkpoint_age: u64::MAX,
//             chain: base.chain,
//             forks: base.forks,
//         });
//
//         let consensus =
//             ConsensusClient::<NimbusRpc>::new(&consensus_rpc, &base.checkpoint, config.clone())
//                 .unwrap();
//
//         let execution = ExecutionClient::<HttpRpc>::new(execution_rpc).unwrap();
//
//         Self {
//             consensus,
//             execution,
//             payloads: BTreeMap::new(),
//             config,
//         }
//     }
//
//     #[wasm_bindgen]
//     pub async fn sync(&mut self) {
//         self.consensus.sync().await.unwrap();
//         self.update_payloads().await;
//     }
//
//     #[wasm_bindgen]
//     pub async fn advance(&mut self) {
//         self.consensus.advance().await.unwrap();
//         self.update_payloads().await;
//     }
//
//     async fn update_payloads(&mut self) {
//         let header = self.consensus.get_header();
//         let payload = self
//             .consensus
//             .get_execution_payload(&Some(header.slot))
//             .await
//             .unwrap();
//
//         self.payloads.insert(payload.block_number, payload);
//
//         while self.payloads.len() > 64 {
//             self.payloads.pop_first();
//         }
//     }
//
//     #[wasm_bindgen]
//     pub async fn chain_id(&self) -> u32 {
//         self.config.chain.chain_id as u32
//     }
//
//     #[wasm_bindgen]
//     pub async fn get_block_number(&self) -> u32 {
//         let payload = self.payloads.last_key_value().unwrap().1;
//         payload.block_number as u32
//     }
//
//     #[wasm_bindgen]
//     pub async fn get_balance(&self, addr: &str, block: &str) -> String {
//         let payload = self.get_payload(block);
//
//         let addr = Address::from_str(addr).unwrap();
//         let account = self
//             .execution
//             .get_account(&addr, None, &payload)
//             .await
//             .unwrap();
//
//         account.balance.to_string()
//     }
//
//     #[wasm_bindgen]
//     pub async fn get_code(&self, addr: &str, block: &str) -> String {
//         let payload = self.get_payload(block);
//
//         let addr = Address::from_str(addr).unwrap();
//         let code = self
//             .execution
//             .get_account(&addr, None, &payload)
//             .await
//             .unwrap()
//             .code;
//
//         format!("0x{}", hex::encode(code))
//     }
//
//     #[wasm_bindgen]
//     pub async fn get_nonce(&self, addr: &str, block: &str) -> u32 {
//         let payload = self.get_payload(block);
//
//         let addr = Address::from_str(addr).unwrap();
//         let nonce = self
//             .execution
//             .get_account(&addr, None, &payload)
//             .await
//             .unwrap()
//             .nonce;
//
//         nonce as u32
//     }
//
//     #[wasm_bindgen]
//     pub async fn get_transaction_by_hash(&self, hash: &str) -> Option<Transaction> {
//         let hash = H256::from_str(hash).unwrap();
//         self.execution
//             .get_transaction(&hash, &self.payloads)
//             .await
//             .unwrap()
//             .map(|tx| Transaction::from_ethers(tx))
//     }
//
//     fn get_payload(&self, block: &str) -> ExecutionPayload {
//         if block == "latest" {
//             self.payloads.last_key_value().unwrap().1.clone()
//         } else {
//             let num = block.parse().unwrap();
//             self.payloads.get(&num).unwrap().clone()
//         }
//     }
// }
//
// #[wasm_bindgen]
// pub struct Transaction {
//     hash: String,
//     to: String,
//     from: String,
//     value: String,
//     pub nonce: u32,
//     gas_limit: String,
//     gas_price: Option<String>,
//     max_fee_per_gas: Option<String>,
//     max_priority_fee_per_gas: Option<String>,
//     data: String,
//     r: String,
//     s: String,
//     v: String,
//     pub chain_id: u32,
// }
//
// #[wasm_bindgen]
// impl Transaction {
//     #[wasm_bindgen(getter)]
//     pub fn hash(&self) -> String {
//         self.hash.clone()
//     }
//
//     #[wasm_bindgen(getter)]
//     pub fn to(&self) -> String {
//         self.to.clone()
//     }
//
//     #[wasm_bindgen(getter)]
//     pub fn from(&self) -> String {
//         self.from.clone()
//     }
//
//     #[wasm_bindgen(getter)]
//     pub fn value(&self) -> String {
//         self.value.clone()
//     }
//
//     #[wasm_bindgen(getter)]
//     pub fn gas_limit(&self) -> String {
//         self.gas_limit.clone()
//     }
//
//     #[wasm_bindgen(getter)]
//     pub fn gas_price(&self) -> Option<String> {
//         self.gas_price.clone()
//     }
//
//     #[wasm_bindgen(getter)]
//     pub fn max_fee_per_gas(&self) -> Option<String> {
//         self.max_fee_per_gas.clone()
//     }
//
//     #[wasm_bindgen(getter)]
//     pub fn max_priority_fee_per_gas(&self) -> Option<String> {
//         self.max_priority_fee_per_gas.clone()
//     }
//
//     #[wasm_bindgen(getter)]
//     pub fn data(&self) -> String {
//         self.data.clone()
//     }
//
//     #[wasm_bindgen(getter)]
//     pub fn r(&self) -> String {
//         self.r.clone()
//     }
//
//     #[wasm_bindgen(getter)]
//     pub fn s(&self) -> String {
//         self.s.clone()
//     }
//
//     #[wasm_bindgen(getter)]
//     pub fn v(&self) -> String {
//         self.v.clone()
//     }
//
//     fn from_ethers(tx: EthersTransaction) -> Self {
//         Self {
//             hash: tx.hash.to_string(),
//             to: tx.to.unwrap().to_string(),
//             from: tx.from.to_string(),
//             value: tx.value.to_string(),
//             nonce: tx.nonce.as_u32(),
//             gas_limit: tx.gas.to_string(),
//             gas_price: tx.gas_price.map(|g| g.to_string()),
//             max_fee_per_gas: tx.max_fee_per_gas.map(|g| g.to_string()),
//             max_priority_fee_per_gas: tx.max_priority_fee_per_gas.map(|g| g.to_string()),
//             data: tx.input.to_string(),
//             r: tx.r.to_string(),
//             s: tx.s.to_string(),
//             v: tx.v.to_string(),
//             chain_id: tx.chain_id.unwrap().as_u32(),
//         }
//     }
// }
