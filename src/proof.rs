use ethers::utils::keccak256;
use eyre::Result;
use serde::Deserialize;
use serde::de::Error;
use jsonrpsee::{http_client::HttpClientBuilder, rpc_params, core::client::ClientT};
use ethers::utils::rlp::{RlpStream, decode_list};
use ethers::prelude::{U256, H256, Address};
use super::utils::hex_str_to_bytes;

pub async fn get_proof(address: &str, block: u64) -> Result<Proof> {
    let rpc = "https://eth-mainnet.g.alchemy.com:443/v2/sUiZsY3BSTYXjSHIvPc9rGDipR7lAlT4";
    let client = HttpClientBuilder::default().build(rpc)?;
    let block_hex = format!("0x{:x}", block);
    let params = rpc_params!(address, [""], block_hex);
    Ok(client.request("eth_getProof", params).await?)
}

pub fn verify_proof(proof: &Vec<Vec<u8>>, root: &Vec<u8>, path: &Vec<u8>, value: &Vec<u8>) -> bool {

    // let account_encoded = encode_account(proof);
    // let state_root = hex_str_to_bytes("0x1d006918a3fef7ff7c843f20747c757a38a0a13fe7723f53e349f462c2cfdd71").unwrap();
    // let path = keccak256(proof.address).to_vec();

    let mut expected_hash = root;
    let mut path_offset = 0;

    for (i, node) in proof.iter().enumerate() {
        if expected_hash != &keccak256(node).to_vec() {
            return false;
        }

        let node_list: Vec<Vec<u8>> = decode_list(node);
        
        if node_list.len() == 17 {

            let nibble = get_nibble(&path, path_offset);
            expected_hash = &node_list[nibble as usize].clone();

            path_offset += 1;

        } else if node_list.len() == 2 {
            if i == proof.len() - 1 {
                if &node_list[1] != value {
                    return false;
                }
            } else {
                expected_hash = &node_list[1].clone();
            }
        } else {
            return false;
        }
    }

    true
}

fn get_nibble(path: &Vec<u8>, offset: usize) -> u8 {
    let byte = path[offset / 2];
    if offset % 2 == 0 {
        byte >> 4
    } else {
        byte & 0xF
    }
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

