use std::{
    fs,
    path::PathBuf,
    process::exit,
    sync::{Arc, Mutex},
};

use clap::Parser;
use common::utils::hex_str_to_bytes;
use dirs::home_dir;
use env_logger::Env;
use eyre::Result;

use client::{database::FileDB, Client};
use config::Config;
use futures::executor::block_on;
use log::info;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let config = get_config().expect("could not parse configuration");
    let mut client = Client::new(config).await?;

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

fn get_config() -> Result<Config> {
    let cli = Cli::parse();

    let data_dir = get_data_dir(&cli);
    let config_path = home_dir().unwrap().join(".lightclient/lightclient.toml");

    let checkpoint = match cli.checkpoint {
        Some(checkpoint) => Some(hex_str_to_bytes(&checkpoint).expect("invalid checkpoint")),
        None => get_cached_checkpoint(&data_dir),
    };

    let config = Config::from_file(
        &config_path,
        &cli.network,
        &cli.execution_rpc,
        &cli.consensus_rpc,
        checkpoint,
        cli.port,
        &data_dir,
    )?;

    Ok(config)
}

fn get_data_dir(cli: &Cli) -> PathBuf {
    home_dir()
        .unwrap()
        .join(format!(".lightclient/data/{}", cli.network))
}

fn get_cached_checkpoint(data_dir: &PathBuf) -> Option<Vec<u8>> {
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

#[derive(Parser)]
struct Cli {
    #[clap(short, long, default_value = "mainnet")]
    network: String,
    #[clap(short, long)]
    port: Option<u16>,
    #[clap(short = 'w', long)]
    checkpoint: Option<String>,
    #[clap(short, long)]
    execution_rpc: Option<String>,
    #[clap(short, long)]
    consensus_rpc: Option<String>,
}
