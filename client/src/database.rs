use std::{fs, io::Write, path::PathBuf};

use eyre::Result;
use log::info;

pub trait Database: Sync + Send {
    fn save_checkpoint(&self, checkpoint: Vec<u8>) -> Result<()>;
}

fn log_checkpoint_save(checkpoint: &Vec<u8>) {
    info!(
        "saving last checkpoint             hash={}",
        hex::encode(&checkpoint)
    );
}

pub struct FileDB {
    data_dir: PathBuf,
}

impl FileDB {
    pub fn new(data_dir: PathBuf) -> Self {
        FileDB { data_dir }
    }
}

impl Database for FileDB {
    fn save_checkpoint(&self, checkpoint: Vec<u8>) -> Result<()> {
        log_checkpoint_save(&checkpoint);

        fs::create_dir_all(&self.data_dir)?;

        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(self.data_dir.join("checkpoint"))?;

        f.write_all(checkpoint.as_slice())?;

        Ok(())
    }
}
