use std::{sync::Arc, time::Duration};

use clap::Parser;
use dirs::home_dir;
use eyre::Result;
use tokio::{sync::Mutex, time::sleep};

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

    let client = Arc::new(Mutex::new(client));

    let mut rpc = Rpc::new(client.clone(), cli.port.unwrap_or(8545));
    let addr = rpc.start().await?;
    println!("started rpc at: {}", addr);

    loop {
        sleep(Duration::from_secs(10)).await;
        client.lock().await.advance().await?
    }
}

#[derive(Parser)]
struct Cli {
    #[clap(long)]
    network: String,
    #[clap(long)]
    port: Option<u16>,
}
