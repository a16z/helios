use std::{panic, str::FromStr};
use std::sync::Arc;

extern crate console_error_panic_hook;
extern crate web_sys;

use config::{networks, Config};
use consensus::{rpc::nimbus_rpc::NimbusRpc, ConsensusClient};
use ethers::types::Address;
use execution::{ExecutionClient, rpc::http_rpc::HttpRpc};
use wasm_bindgen::prelude::*;

macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

#[wasm_bindgen]
pub async fn get_balance(addr: &str) -> String {
    panic::set_hook(Box::new(console_error_panic_hook::hook));
    log!("here");

    let rpc = "https://www.lightclientdata.org".to_string();

    let base = networks::mainnet();
    let config = Config {
        checkpoint: base.checkpoint.clone(),
        consensus_rpc: rpc.clone(),
        rpc_port: None,

        data_dir: None,
        execution_rpc: "".to_string(),
        max_checkpoint_age: u64::MAX,
        chain: base.chain,
        forks: base.forks,
    };

    let mut client = ConsensusClient::<NimbusRpc>::new(&rpc, &base.checkpoint, Arc::new(config)).unwrap();
    client.sync().await.unwrap();
    let header = client.get_header();
    let payload = client.get_execution_payload(&Some(header.slot)).await.unwrap();

    let execution = ExecutionClient::<HttpRpc>::new("https://eth-mainnet.g.alchemy.com/v2/23IavJytUwkTtBMpzt_TZKwgwAarocdT").unwrap();

    let addr = Address::from_str(addr).unwrap();
    let account = execution.get_account(&addr, None, &payload).await.unwrap();

    account.balance.to_string()
}
