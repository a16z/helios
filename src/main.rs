use std::str::FromStr;

use ethers::prelude::Address;
use eyre::Result;

use client::Client;

pub mod client;
pub mod consensus;
pub mod execution;
pub mod common;

#[tokio::main]
async fn main() -> Result<()> {
    let consensus_rpc = "http://testing.prater.beacon-api.nimbus.team";
    let execution_rpc = "https://eth-goerli.g.alchemy.com:443/v2/o_8Qa9kgwDPf9G8sroyQ-uQtyhyWa3ao";
    let checkpoint = "0x172128eadf1da46467f4d6a822206698e2d3f957af117dd650954780d680dc99";

    let mut client = Client::new(consensus_rpc, execution_rpc, checkpoint).await?;
    client.sync().await?;

    let header = client.get_header();
    println!("synced up to slot: {}", header.slot);

    let address = Address::from_str("0xe0Fa62CD8543473627D337fAe1212d4E639EE932")?;
    let balance = client.get_balance(address).await?;
    let nonce = client.get_nonce(address).await?;
    println!("balance: {}", balance);
    println!("nonce: {}", nonce);

    Ok(())
}
