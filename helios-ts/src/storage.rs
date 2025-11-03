extern crate console_error_panic_hook;
extern crate web_sys;

use alloy::{hex, hex::FromHex, primitives::B256};
use eyre::Result;
use wasm_bindgen::prelude::*;

use helios_ethereum::{config::Config, database::Database};

#[derive(Clone)]
pub struct LocalStorageDB;

impl Database for LocalStorageDB {
    fn new(_config: &Config) -> Result<Self> {
        console_error_panic_hook::set_once();
        let window = web_sys::window().unwrap();
        if let Ok(Some(_local_storage)) = window.local_storage() {
            return Ok(Self {});
        }

        eyre::bail!("local_storage not available")
    }

    fn load_checkpoint(&self) -> Result<B256> {
        let window = web_sys::window().unwrap();
        if let Ok(Some(local_storage)) = window.local_storage() {
            let checkpoint = local_storage.get_item("checkpoint");
            if let Ok(Some(checkpoint)) = checkpoint {
                let checkpoint = checkpoint.strip_prefix("0x").unwrap_or(&checkpoint);
                return B256::from_hex(checkpoint)
                    .map_err(|_| eyre::eyre!("Failed to decode checkpoint"));
            }
            eyre::bail!("checkpoint not found")
        }

        eyre::bail!("local_storage not available")
    }

    fn save_checkpoint(&self, checkpoint: B256) -> Result<()> {
        let window = web_sys::window().unwrap();
        if let Ok(Some(local_storage)) = window.local_storage() {
            local_storage
                .set_item("checkpoint", &hex::encode(checkpoint))
                .unwrap_throw();
            return Ok(());
        }

        eyre::bail!("local_storage not available")
    }
}
