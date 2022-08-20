use std::str::FromStr;

use ethers::prelude::Address;
use eyre::Result;

use consensus::*;
use execution::*;

pub mod consensus;
pub mod consensus_rpc;
pub mod execution;
pub mod execution_rpc;
pub mod proof;
pub mod utils;

#[tokio::main]
async fn main() -> Result<()> {
    let rpc = "http://testing.prater.beacon-api.nimbus.team";
    let checkpoint = "0x172128eadf1da46467f4d6a822206698e2d3f957af117dd650954780d680dc99";
    let mut client = ConsensusClient::new(rpc, checkpoint).await?;

    let rpc = "https://eth-goerli.g.alchemy.com:443/v2/o_8Qa9kgwDPf9G8sroyQ-uQtyhyWa3ao";
    let execution = ExecutionClient::new(rpc);

    client.sync().await?;

    let payload = client.get_execution_payload().await?;
    println!(
        "verified execution block hash: {}",
        hex::encode(&payload.block_hash)
    );

    let addr = Address::from_str("0x25c4a76E7d118705e7Ea2e9b7d8C59930d8aCD3b")?;
    let balance = execution.get_balance(&addr, &payload).await?;
    println!("verified account balance: {}", balance);

    Ok(())
}
