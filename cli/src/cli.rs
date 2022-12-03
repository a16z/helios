use std::{fs, path::PathBuf, str::FromStr};

use clap::Parser;
use common::utils::hex_str_to_bytes;
use dirs::home_dir;

use config::{CliConfig, Config};

#[derive(Parser)]
pub struct Cli {
    #[clap(short, long, default_value = "mainnet")]
    network: String,
    #[clap(short = 'p', long, env)]
    rpc_port: Option<u16>,
    #[clap(short = 'w', long, env)]
    checkpoint: Option<String>,
    #[clap(short, long, env)]
    execution_rpc: Option<String>,
    #[clap(short, long, env)]
    consensus_rpc: Option<String>,
    #[clap(short, long, env)]
    data_dir: Option<String>,
    #[clap(short = 'f', long, env)]
    fallback: Option<String>,
    #[clap(short = 'l', long, env)]
    load_external_fallback: bool,
    #[clap(short = 's', long, env)]
    with_ws: bool,
    #[clap(short = 'h', long, env)]
    with_http: bool,
}

impl Cli {
    pub fn to_config() -> Config {
        let cli = Cli::parse();
        let config_path = home_dir().unwrap().join(".helios/helios.toml");
        let cli_config = cli.as_cli_config();
        Config::from_file(&config_path, &cli.network, &cli_config)
    }

    fn as_cli_config(&self) -> CliConfig {
        let checkpoint = match &self.checkpoint {
            Some(checkpoint) => Some(hex_str_to_bytes(checkpoint).expect("invalid checkpoint")),
            None => self.get_cached_checkpoint(),
        };

        CliConfig {
            checkpoint,
            execution_rpc: self.execution_rpc.clone(),
            consensus_rpc: self.consensus_rpc.clone(),
            data_dir: self.get_data_dir(),
            rpc_port: self.rpc_port,
            fallback: self.fallback.clone(),
            load_external_fallback: self.load_external_fallback,
            with_ws: self.with_ws,
            with_http: self.with_http,
        }
    }

    fn get_cached_checkpoint(&self) -> Option<Vec<u8>> {
        let data_dir = self.get_data_dir();
        let checkpoint_file = data_dir.join("checkpoint");

        if checkpoint_file.exists() {
            let checkpoint_res = fs::read(checkpoint_file);
            match checkpoint_res {
                Ok(checkpoint) => Some(checkpoint),
                Err(_) => None,
            }
        } else {
            None
        }
    }

    fn get_data_dir(&self) -> PathBuf {
        if let Some(dir) = &self.data_dir {
            PathBuf::from_str(dir).expect("cannot find data dir")
        } else {
            home_dir()
                .unwrap()
                .join(format!(".helios/data/{}", self.network))
        }
    }
}
