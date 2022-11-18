use std::panic;
use std::sync::Arc;

extern crate console_error_panic_hook;
extern crate web_sys;

use config::{networks, Config};
use consensus::{rpc::nimbus_rpc::NimbusRpc, ConsensusClient};
use wasm_bindgen::prelude::*;

macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

#[wasm_bindgen]
pub async fn current_slot() -> u64 {
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

    let mut client =
        ConsensusClient::<NimbusRpc>::new(&rpc, &base.checkpoint, Arc::new(config)).unwrap();
    client.sync().await.unwrap();

    let header = client.get_header();

    header.slot
}
