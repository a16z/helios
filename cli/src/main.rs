use clap::Parser;
use common::utils::hex_str_to_bytes;
use dirs::home_dir;
use env_logger::Env;
use eyre::Result;

use client::Client;
use config::{networks, Config};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let config = get_config()?;
    let mut client = Client::new(config).await?;

    client.sync().await?;
    client.start_rpc().await?;
    client.track()?;

    std::future::pending().await
}

fn get_config() -> Result<Config> {
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

    if let Some(port) = cli.port {
        config.general.rpc_port = Some(port);
    }

    Ok(config)
}

#[derive(Parser)]
struct Cli {
    #[clap(short, long, default_value = "goerli")]
    network: String,
    #[clap(short, long)]
    port: Option<u16>,
    #[clap(short, long)]
    checkpoint: Option<String>,
}
