use std::{
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
use log::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let config = get_config();
    let mut client = match ClientBuilder::new().config(config).build() {
        Ok(client) => client,
        Err(err) => {
            error!("{}", err);
            exit(1);
        }
    };

    if let Err(err) = client.start().await {
        error!("{}", err);
        exit(1);
    }

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
    fallback: Option<String>,
    #[clap(short = 'l', long, env)]
    load_external_fallback: bool,
    #[clap(short = 's', long, env)]
    strict_checkpoint_age: bool,
}

impl Cli {
    fn as_cli_config(&self) -> CliConfig {
        let checkpoint = self
            .checkpoint
            .as_ref()
            .map(|c| hex_str_to_bytes(c).expect("invalid checkpoint"));

        CliConfig {
            checkpoint,
            execution_rpc: self.execution_rpc.clone(),
            consensus_rpc: self.consensus_rpc.clone(),
            data_dir: self.get_data_dir(),
            rpc_port: self.rpc_port,
            fallback: self.fallback.clone(),
            load_external_fallback: self.load_external_fallback,
            strict_checkpoint_age: self.strict_checkpoint_age,
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
