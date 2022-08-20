use ethers::utils::keccak256;
use ethers::utils::rlp::{decode_list, RlpStream};

use crate::execution_rpc::Proof;

pub fn verify_proof(proof: &Vec<Vec<u8>>, root: &Vec<u8>, path: &Vec<u8>, value: &Vec<u8>) -> bool {
    let mut expected_hash = root.clone();
    let mut path_offset = 0;

    for (i, node) in proof.iter().enumerate() {
        if expected_hash != keccak256(node).to_vec() {
            return false;
        }

        let node_list: Vec<Vec<u8>> = decode_list(node);

        if node_list.len() == 17 {
            let nibble = get_nibble(&path, path_offset);
            expected_hash = node_list[nibble as usize].clone();

            path_offset += 1;
        } else if node_list.len() == 2 {
            if i == proof.len() - 1 {
                if &node_list[1] != value {
                    return false;
                }
            } else {
                expected_hash = node_list[1].clone();
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

pub fn encode_account(proof: &Proof) -> Vec<u8> {
    let mut stream = RlpStream::new_list(4);
    stream.append(&proof.nonce);
    stream.append(&proof.balance);
    stream.append(&proof.storage_hash);
    stream.append(&proof.code_hash);
    let encoded = stream.out();
    encoded.to_vec()
}

pub fn get_account_path(addr: &Vec<u8>) -> Vec<u8> {
    keccak256(addr).to_vec()
}
