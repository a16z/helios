#[cfg(not(target_arch = "wasm32"))]
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};

use config::Config;
use eyre::Result;

pub trait Database {
    fn new(config: &Config) -> Result<Self>
    where
        Self: Sized;

    fn save_checkpoint(&self, checkpoint: &[u8]) -> Result<()>;
    fn load_checkpoint(&self) -> Result<Vec<u8>>;
}

#[cfg(not(target_arch = "wasm32"))]
pub struct FileDB {
    data_dir: PathBuf,
    default_checkpoint: Vec<u8>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Database for FileDB {
    fn new(config: &Config) -> Result<Self> {
        if let Some(data_dir) = &config.data_dir {
            return Ok(FileDB {
                data_dir: data_dir.to_path_buf(),
                default_checkpoint: config.default_checkpoint.clone(),
            });
        }

        eyre::bail!("data dir not in config")
    }

    fn save_checkpoint(&self, checkpoint: &[u8]) -> Result<()> {
        fs::create_dir_all(&self.data_dir)?;

        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(self.data_dir.join("checkpoint"))?;

        f.write_all(checkpoint)?;

        Ok(())
    }

    fn load_checkpoint(&self) -> Result<Vec<u8>> {
        let mut buf = Vec::new();

        let res = fs::OpenOptions::new()
            .read(true)
            .open(self.data_dir.join("checkpoint"))
            .map(|mut f| f.read_to_end(&mut buf));

        if buf.len() == 32 && res.is_ok() {
            Ok(buf)
        } else {
            Ok(self.default_checkpoint.clone())
        }
    }
}

pub struct ConfigDB {
    checkpoint: Vec<u8>,
}

impl Database for ConfigDB {
    fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            checkpoint: config
                .checkpoint
                .clone()
                .unwrap_or(config.default_checkpoint.clone()),
        })
    }

    fn load_checkpoint(&self) -> Result<Vec<u8>> {
        Ok(self.checkpoint.clone())
    }

    fn save_checkpoint(&self, _checkpoint: &[u8]) -> Result<()> {
        Ok(())
    }
}
