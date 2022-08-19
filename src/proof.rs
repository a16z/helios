use ethers::utils::keccak256;
use eyre::Result;
use serde::Deserialize;
use serde::de::Error;
use jsonrpsee::{http_client::HttpClientBuilder, rpc_params, core::client::ClientT};
use ethers::utils::rlp::RlpStream;
use ethers::prelude::{U256, H256, Address};
use super::utils::hex_str_to_bytes;

pub async fn get_proof(address: &str, block: u64) -> Result<Proof> {
    let rpc = "https://eth-mainnet.g.alchemy.com:443/v2/sUiZsY3BSTYXjSHIvPc9rGDipR7lAlT4";
    let client = HttpClientBuilder::default().build(rpc)?;
    let block_hex = format!("0x{:x}", block);
    let params = rpc_params!(address, [""], block_hex);
    Ok(client.request("eth_getProof", params).await?)
}

pub fn verify(proof: &Proof) -> Result<()> {

    let account_encoded = encode_account(proof);
    println!("{:?}", account_encoded);

    let state_root = hex_str_to_bytes("0xeb5eb01bd4f503a5698d136c75b37ef2660d6842bf77d0453f2a7fa4a6780d91")?;
    let address_hash = keccak256(proof.address);


    Ok(())
}

fn encode_account(proof: &Proof) -> Vec<u8> {
    let mut stream = RlpStream::new_list(4);
    stream.append(&proof.nonce);
    stream.append(&proof.balance);
    stream.append(&proof.storage_hash);
    stream.append(&proof.code_hash);
    let encoded = stream.out();
    encoded.to_vec()
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Proof {
    address: Address,
    balance: U256,
    code_hash: H256,
    nonce: U256,
    storage_hash: H256,
    #[serde(deserialize_with = "proof_deserialize")]
    account_proof: Vec<Vec<u8>>,
}

fn proof_deserialize<'de, D>(deserializer: D) -> Result<Vec<Vec<u8>>, D::Error> where D: serde::Deserializer<'de> {
    let branch: Vec<String> = serde::Deserialize::deserialize(deserializer)?;
    Ok(branch.iter().map(|elem| {
        hex_str_to_bytes(elem)
    }).collect::<Result<_>>().map_err(D::Error::custom)?)
}

