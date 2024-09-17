use std::net::IpAddr;
use std::{
    path::PathBuf,
    process::exit,
    str::FromStr,
    sync::{Arc, Mutex},
};

use alloy::primitives::B256;
use anyhow::Result;
use clap::Parser;
use dirs::home_dir;
use futures::executor::block_on;
use tracing::{error, info};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::FmtSubscriber;

use client::{Client, ClientBuilder};
use config::{CliConfig, Config};
use consensus::database::FileDB;

#[tokio::main]
async fn main() -> Result<()> {
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()
        .expect("invalid env filter");

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(env_filter)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("subscriber set failed");

    let config = get_config();
    let mut client = match ClientBuilder::new().config(config).build::<FileDB>() {
        Ok(client) => client,
        Err(err) => {
            error!(target: "helios::runner", error = %err);
            exit(1);
        }
    };

    if let Err(err) = client.start().await {
        error!(target: "helios::runner", error = %err);
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
            info!(target: "helios::runner", "forced shutdown");
            exit(0);
        }

        info!(
            target: "helios::runner",
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
#[clap(version, about)]
/// Helios is a fast, secure, and portable light client for Ethereum
struct Cli {
    #[clap(short, long, default_value = "mainnet")]
    network: String,
    #[clap(short = 'b', long, env)]
    rpc_bind_ip: Option<IpAddr>,
    #[clap(short = 'p', long, env)]
    rpc_port: Option<u16>,
    #[clap(short = 'w', long, env)]
    checkpoint: Option<B256>,
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
        CliConfig {
            checkpoint: self.checkpoint,
            execution_rpc: self.execution_rpc.clone(),
            consensus_rpc: self.consensus_rpc.clone(),
            data_dir: self
                .data_dir
                .as_ref()
                .map(|s| PathBuf::from_str(s).expect("cannot find data dir")),
            rpc_bind_ip: self.rpc_bind_ip,
            rpc_port: self.rpc_port,
            fallback: self.fallback.clone(),
            load_external_fallback: true_or_none(self.load_external_fallback),
            strict_checkpoint_age: true_or_none(self.strict_checkpoint_age),
        }
    }
}

fn true_or_none(b: bool) -> Option<bool> {
    if b {
        Some(b)
    } else {
        None
    }
}
