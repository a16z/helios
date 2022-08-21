use std::str::FromStr;

use ethers::prelude::{Address, U256};
use eyre::Result;

use client::Client;

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

    let header = client.get_header();
    println!("synced up to slot: {}", header.slot);

    let address = Address::from_str("0x14f9D4aF749609c1438528C0Cce1cC3f6D411c47")?;
    let balance = client.get_balance(&address).await?;
    let nonce = client.get_nonce(&address).await?;
    let code = client.get_code(&address).await?;
    let storage_value = client.get_storage_at(&address, U256::from(0)).await?;

    println!("balance: {}", balance);
    println!("nonce: {}", nonce);
    println!("code: 0x{}...", hex::encode(code[..5].to_vec()));
    println!("value at slot 0: 0x{:x}", storage_value);

    Ok(())
}
