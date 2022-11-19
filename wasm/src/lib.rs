use std::str::FromStr;
use std::sync::Arc;

extern crate console_error_panic_hook;
extern crate web_sys;

use config::{networks, Config};
use consensus::{rpc::nimbus_rpc::NimbusRpc, ConsensusClient};
use ethers::types::Address;
use execution::{rpc::http_rpc::HttpRpc, ExecutionClient};
use wasm_bindgen::prelude::*;

#[allow(unused_macros)]
macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

#[wasm_bindgen]
pub fn ret_five() -> u64 {
    5
}

#[wasm_bindgen]
pub struct Node {
    consensus: ConsensusClient<NimbusRpc>,
    execution: ExecutionClient<HttpRpc>,
}

#[wasm_bindgen]
impl Node {
    #[wasm_bindgen(constructor)]
    pub fn new(consensus_rpc: &str, execution_rpc: &str) -> Self {
        console_error_panic_hook::set_once();

        let base = networks::mainnet();
        let config = Config {
            checkpoint: base.checkpoint.clone(),
            consensus_rpc: consensus_rpc.to_string(),
            rpc_port: None,

            data_dir: None,
            execution_rpc: "".to_string(),
            max_checkpoint_age: u64::MAX,
            chain: base.chain,
            forks: base.forks,
        };

        let consensus =
            ConsensusClient::<NimbusRpc>::new(&consensus_rpc, &base.checkpoint, Arc::new(config))
                .unwrap();

        let execution = ExecutionClient::<HttpRpc>::new(execution_rpc).unwrap();

        Self {
            consensus,
            execution,
        }
    }

    #[wasm_bindgen]
    pub async fn sync(&mut self) {
        self.consensus.sync().await.unwrap();
    }

    #[wasm_bindgen]
    pub async fn get_balance(&self, addr: &str) -> String {
        let header = self.consensus.get_header();
        let payload = self
            .consensus
            .get_execution_payload(&Some(header.slot))
            .await
            .unwrap();

        let addr = Address::from_str(addr).unwrap();
        let account = self
            .execution
            .get_account(&addr, None, &payload)
            .await
            .unwrap();

        account.balance.to_string()
    }
}
