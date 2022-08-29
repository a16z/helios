use std::{sync::Arc, time::Duration};

use clap::Parser;
use dirs::home_dir;
use eyre::Result;
use tokio::time::sleep;

use client::{rpc::Rpc, Client};
use config::{networks, Config};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = match cli.network.as_str() {
        "goerli" => networks::goerli(),
        _ => {
            let home = home_dir().unwrap();
            let config_path = home.join(format!(".lightclient/configs/{}.toml", cli.network));
            Config::from_file(&config_path).unwrap()
        }
    };

    let mut client = Client::new(Arc::new(config)).await?;
    client.sync().await?;

    let mut rpc = Rpc::new(Arc::new(client), cli.port.unwrap_or(8545));
    let addr = rpc.start().await?;
    println!("started rpc at: {}", addr);

    sleep(Duration::from_secs(300)).await;

    Ok(())
}

#[derive(Parser)]
struct Cli {
    #[clap(long)]
    network: String,
    #[clap(long)]
    port: Option<u16>,
}
