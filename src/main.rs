use std::{sync::Arc, time::Duration};

use eyre::Result;

use client::{rpc::Rpc, Client};
use tokio::time::sleep;

pub mod client;
pub mod common;
pub mod consensus;
pub mod execution;

#[tokio::main]
async fn main() -> Result<()> {
    let consensus_rpc = "http://testing.prater.beacon-api.nimbus.team";
    let execution_rpc = "https://eth-goerli.g.alchemy.com:443/v2/o_8Qa9kgwDPf9G8sroyQ-uQtyhyWa3ao";
    let checkpoint = "0x172128eadf1da46467f4d6a822206698e2d3f957af117dd650954780d680dc99";

    let mut client = Client::new(consensus_rpc, execution_rpc, checkpoint).await?;
    client.sync().await?;

    let mut rpc = Rpc::new(Arc::new(client));
    let addr = rpc.start().await?;
    println!("{}", addr);

    sleep(Duration::from_secs(300)).await;

    Ok(())
}
