use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};

use config::Config;
use eyre::Result;

pub trait Database {
    fn new(config: &Config) -> Result<Self> where Self: Sized;
    fn save_checkpoint(&self, checkpoint: Vec<u8>) -> Result<()>;
    fn load_checkpoint(&self) -> Result<Vec<u8>>;
}

pub struct FileDB {
    data_dir: PathBuf,
}

impl Database for FileDB {
    fn new(config: &Config) -> Result<Self> {
        if let Some(data_dir) = &config.data_dir {
            return Ok(FileDB { data_dir: data_dir.to_path_buf() })
        }

        eyre::bail!("data dir not in config")
    }

    fn save_checkpoint(&self, checkpoint: Vec<u8>) -> Result<()> {
        fs::create_dir_all(&self.data_dir)?;

        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(self.data_dir.join("checkpoint"))?;

        f.write_all(checkpoint.as_slice())?;

        Ok(())
    }

    fn load_checkpoint(&self) -> Result<Vec<u8>> {
        let mut f = fs::OpenOptions::new()
            .read(true)
            .open(self.data_dir.join("checkpoint"))?;

        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;

        Ok(buf)
    }
}
