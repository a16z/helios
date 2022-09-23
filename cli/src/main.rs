use std::{fs, path::PathBuf, process::exit};

use clap::Parser;
use common::utils::hex_str_to_bytes;
use dirs::home_dir;
use env_logger::Env;
use eyre::Result;

use client::Client;
use config::{networks, Config};
use futures::executor::block_on;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let config = get_config();
    let mut client = Client::new(config).await?;

    client.start().await?;

    ctrlc::set_handler(move || {
        block_on(client.shutdown());
        exit(0);
    })?;

    std::future::pending().await
}

fn get_config() -> Config {
    let cli = Cli::parse();
    let mut config = match cli.network.as_str() {
        "mainnet" => networks::mainnet(),
        "goerli" => networks::goerli(),
        _ => {
            let home = home_dir().unwrap();
            let config_path = home.join(format!(".lightclient/configs/{}.toml", cli.network));
            Config::from_file(&config_path).expect("could not read network config")
        }
    };

    let data_dir = get_data_dir(&cli);

    config.general.checkpoint = match cli.checkpoint {
        Some(checkpoint) => hex_str_to_bytes(&checkpoint).expect("invalid checkpoint"),
        None => get_cached_checkpoint(&data_dir).unwrap_or(config.general.checkpoint),
    };

    config.general.execution_rpc = Some(cli.execution_rpc);

    if let Some(port) = cli.port {
        config.general.rpc_port = Some(port);
    }

    if let Some(consensus_rpc) = cli.consensus_rpc {
        config.general.consensus_rpc = consensus_rpc;
    }

    config.machine.data_dir = Some(data_dir);

    config
}

fn get_data_dir(cli: &Cli) -> PathBuf {
    match &cli.data_dir {
        Some(dir) => PathBuf::from(dir),
        None => home_dir()
            .unwrap()
            .join(format!(".lightclient/data/{}", cli.network)),
    }
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
    execution_rpc: String,
    #[clap(short, long)]
    consensus_rpc: Option<String>,
    #[clap(long)]
    data_dir: Option<String>,
}
