use std::{
    fs,
    path::PathBuf,
    process::exit,
    str::FromStr,
    sync::{Arc, Mutex},
};

use clap::Parser;
use common::utils::hex_str_to_bytes;
use dirs::home_dir;
use env_logger::Env;
use eyre::Result;

use client::{database::FileDB, Client, ClientBuilder};
use config::{CliConfig, Config};
use futures::executor::block_on;
use log::info;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let config = get_config();
    let mut client = ClientBuilder::new().config(config).build()?;

    client.start().await?;

    register_shutdown_handler(client);
    std::future::pending().await
}

fn register_shutdown_handler(client: Client<FileDB>) {
    let client = Arc::new(client);
    let shutdown_counter = Arc::new(Mutex::new(0));

    ctrlc::set_handler(move || {
        let mut counter = shutdown_counter.lock().unwrap();
        *counter += 1;

        let counter_value = *counter;

        if counter_value == 3 {
            info!("forced shutdown");
            exit(0);
        }

        info!(
            "shutting down... press ctrl-c {} more times to force quit",
            3 - counter_value
        );

        if counter_value == 1 {
            let client = client.clone();
            std::thread::spawn(move || {
                block_on(client.shutdown());
                exit(0);
            });
        }
    })
    .expect("could not register shutdown handler");
}

fn get_config() -> Config {
    let cli = Cli::parse();

    let config_path = home_dir().unwrap().join(".helios/helios.toml");

    let cli_config = cli.as_cli_config();

    Config::from_file(&config_path, &cli.network, &cli_config)
}

#[derive(Parser)]
struct Cli {
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
    checkpoint_fallback: Option<String>,
    #[clap(short = 'z', long, env)]
    load_checkpoint_fallback: bool,
}

impl Cli {
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
            checkpoint_fallback: self.checkpoint_fallback.clone(),
            load_checkpoint_fallback: self.load_checkpoint_fallback,
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
