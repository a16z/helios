use std::{sync::Arc, time::Duration};

use clap::Parser;
use common::utils::hex_str_to_bytes;
use dirs::home_dir;
use env_logger::Env;
use eyre::Result;
use tokio::{sync::Mutex, time::sleep};

use client::{rpc::Rpc, Client};
use config::{networks, Config};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();
    let mut config = match cli.network.as_str() {
        "goerli" => networks::goerli(),
        _ => {
            let home = home_dir().unwrap();
            let config_path = home.join(format!(".lightclient/configs/{}.toml", cli.network));
            Config::from_file(&config_path).unwrap()
        }
    };

    if let Some(checkpoint) = cli.checkpoint {
        config.general.checkpoint = hex_str_to_bytes(&checkpoint)?;
    }

    let mut client = Client::new(Arc::new(config)).await?;
    client.sync().await?;

    let client = Arc::new(Mutex::new(client));

    let mut rpc = Rpc::new(client.clone(), cli.port);
    rpc.start().await?;

    loop {
        sleep(Duration::from_secs(10)).await;
        client.lock().await.advance().await?
    }
}

#[derive(Parser)]
struct Cli {
    #[clap(short, long, default_value = "goerli")]
    network: String,
    #[clap(short, long, default_value = "8545")]
    port: u16,
    #[clap(short, long)]
    checkpoint: Option<String>,
}
