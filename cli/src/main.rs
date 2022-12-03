use env_logger::Env;
use eyre::Result;

use client::{Client, ClientBuilder};

mod cli;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let config = cli::Cli::to_config();
    let mut client = ClientBuilder::new().config(config).build()?;

    client.start().await?;

    Client::register_shutdown_handler(client);
    std::future::pending().await
}
