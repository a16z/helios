use eyre::Result;

use consensus::*;

pub mod consensus;
pub mod consensus_rpc;
pub mod utils;
pub mod proof;

#[tokio::main]
async fn main() -> Result<()> {

    let rpc = "http://testing.prater.beacon-api.nimbus.team";
    let checkpoint = "0x172128eadf1da46467f4d6a822206698e2d3f957af117dd650954780d680dc99";
    let mut client = ConsensusClient::new(rpc, checkpoint).await?;    
    
    client.sync().await?;

    let payload = client.get_execution_payload().await?;
    println!("verified execution block hash: {}", hex::encode(payload.block_hash));

    Ok(())
}

