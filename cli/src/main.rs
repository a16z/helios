use std::{path::Path, sync::Arc, time::Duration};

use eyre::Result;
use tokio::time::sleep;

use client::{rpc::Rpc, Client};
use common::config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::from_file(Path::new("./configs/goerli.toml"))?;

    let mut client = Client::new(Arc::new(config)).await?;
    client.sync().await?;

    let mut rpc = Rpc::new(Arc::new(client));
    let addr = rpc.start().await?;
    println!("{}", addr);

    sleep(Duration::from_secs(300)).await;

    Ok(())
}
