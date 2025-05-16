#[cfg(not(target_arch = "wasm32"))]
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};

use alloy::primitives::B256;
use eyre::Result;

use crate::config::Config;

pub trait Database: Default + Clone + Sync + Send + 'static {
    fn new(config: &Config) -> Result<Self>
    where
        Self: Sized;

    fn save_checkpoint(&self, checkpoint: B256) -> Result<()>;
    fn load_checkpoint(&self) -> Result<B256>;
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
pub struct FileDB {
    data_dir: PathBuf,
    default_checkpoint: B256,
}

#[cfg(not(target_arch = "wasm32"))]
impl Database for FileDB {
    fn new(config: &Config) -> Result<Self> {
        if let Some(data_dir) = &config.data_dir {
            return Ok(FileDB {
                data_dir: data_dir.to_path_buf(),
                default_checkpoint: config.default_checkpoint,
            });
        }

        eyre::bail!("data dir not in config")
    }

    fn save_checkpoint(&self, checkpoint: B256) -> Result<()> {
        fs::create_dir_all(&self.data_dir)?;

        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(self.data_dir.join("checkpoint"))?;

        f.write_all(checkpoint.as_slice())?;

        Ok(())
    }

    fn load_checkpoint(&self) -> Result<B256> {
        let mut buf = Vec::new();

        let res = fs::OpenOptions::new()
            .read(true)
            .open(self.data_dir.join("checkpoint"))
            .map(|mut f| f.read_to_end(&mut buf));

        if buf.len() == 32 && res.is_ok() {
            Ok(B256::from_slice(&buf))
        } else {
            Ok(self.default_checkpoint)
        }
    }
}

impl Default for FileDB {
    fn default() -> Self {
         panic!("not default for db construction");
     } 
}

#[derive(Clone)]
pub struct ConfigDB {
    checkpoint: B256,
}

impl Database for ConfigDB {
    fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            checkpoint: config.checkpoint.unwrap_or(config.default_checkpoint),
        })
    }

    fn load_checkpoint(&self) -> Result<B256> {
        Ok(self.checkpoint)
    }

    fn save_checkpoint(&self, _checkpoint: B256) -> Result<()> {
        Ok(())
    }
}

impl Default for ConfigDB {
    fn default() -> Self {
         panic!("not default for db construction");
     } 
}
