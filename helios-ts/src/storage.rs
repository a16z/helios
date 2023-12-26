extern crate console_error_panic_hook;
extern crate web_sys;

use config::Config;
use consensus::database::Database;
use eyre::Result;

#[derive(Clone)]
pub struct StorageDB {
    checkpoint: Vec<u8>,
}

impl Database for StorageDB {
    fn new(config: &Config) -> Result<Self> {
        console_error_panic_hook::set_once();
        let window = web_sys::window().unwrap();
        if let Ok(Some(local_storage)) = window.local_storage() {
            let checkpoint = local_storage.get_item("checkpoint").unwrap();
            let checkpoint = checkpoint
                .as_ref()
                .map(|c| c.strip_prefix("0x").unwrap_or(c.as_str()))
                .map(|c| hex::decode(c).unwrap())
                .unwrap_or(config.default_checkpoint.clone());
            return Ok(Self { checkpoint });
        }

        eyre::bail!("local_storage not available")
    }

    fn load_checkpoint(&self) -> Result<Vec<u8>> {
        Ok(self.checkpoint.clone())
    }

    fn save_checkpoint(&self, checkpoint: &[u8]) -> Result<()> {
        let window = web_sys::window().unwrap();
        if let Ok(Some(local_storage)) = window.local_storage() {
            local_storage
                .set_item("checkpoint", &hex::encode(checkpoint))
                .unwrap();
            return Ok(());
        }

        eyre::bail!("local_storage not available")
    }
}
