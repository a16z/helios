use eyre::Result;

use consensus::*;
use execution_rpc::*;
use proof::*;
use utils::hex_str_to_bytes;

pub mod consensus;
pub mod consensus_rpc;
pub mod execution_rpc;
pub mod utils;
pub mod proof;

#[tokio::main]
async fn main() -> Result<()> {

    let rpc = "http://testing.prater.beacon-api.nimbus.team";
    let checkpoint = "0x172128eadf1da46467f4d6a822206698e2d3f957af117dd650954780d680dc99";
    let mut client = ConsensusClient::new(rpc, checkpoint).await?;    
    
    let rpc = "https://eth-goerli.g.alchemy.com:443/v2/o_8Qa9kgwDPf9G8sroyQ-uQtyhyWa3ao";
    let execution = ExecutionRpc::new(rpc);
    
    client.sync().await?;

    let payload = client.get_execution_payload().await?;
    println!("verified execution block hash: {}", hex::encode(payload.block_hash));

    let addr = "0x25c4a76E7d118705e7Ea2e9b7d8C59930d8aCD3b";
    let proof = execution.get_proof(addr, payload.block_number).await?;

    let account_path = get_account_path(&hex_str_to_bytes(addr)?);
    let account_encoded = encode_account(&proof);

    let is_valid = verify_proof(&proof.account_proof, &payload.state_root, &account_path, &account_encoded);

    println!("is account proof valid: {}", is_valid);

    Ok(())
}

