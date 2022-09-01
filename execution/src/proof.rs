use ethers::utils::keccak256;
use ethers::utils::rlp::{decode_list, RlpStream};

use super::types::Proof;

pub fn verify_proof(proof: &Vec<Vec<u8>>, root: &Vec<u8>, path: &Vec<u8>, value: &Vec<u8>) -> bool {
    let mut expected_hash = root.clone();
    let mut path_offset = 0;

    for (i, node) in proof.iter().enumerate() {
        if expected_hash != keccak256(node).to_vec() {
            return false;
        }

        let node_list: Vec<Vec<u8>> = decode_list(node);

        if node_list.len() == 17 {
            if i == proof.len() - 1 {
                // exclusion proof
                let nibble = get_nibble(&path, path_offset);
                let node = &node_list[nibble as usize];

                if node.len() == 0 && is_empty_value(value) {
                    return true;
                }
            } else {
                // inclusion proof
                let nibble = get_nibble(&path, path_offset);
                expected_hash = node_list[nibble as usize].clone();

                path_offset += 1;
            }
        } else if node_list.len() == 2 {
            if i == proof.len() - 1 {
                // exclusion proof
                if &node_list[0][skip_length(&node_list[0])..] != &path[path_offset..]
                    && is_empty_value(value)
                {
                    return true;
                }

                // inclusion proof
                if &node_list[1] == value {
                    return true;
                }
            } else {
                let node_path = &node_list[0];
                let prefix_length = shared_prefix_length(path, path_offset, node_path);
                path_offset += prefix_length;
                expected_hash = node_list[1].clone();
            }
        } else {
            return false;
        }
    }

    false
}

fn is_empty_value(value: &Vec<u8>) -> bool {
    let mut stream = RlpStream::new();
    stream.begin_list(4);
    stream.append_empty_data();
    stream.append_empty_data();
    let empty_storage_hash = "56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421";
    stream.append(&hex::decode(empty_storage_hash).unwrap());
    let empty_code_hash = "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470";
    stream.append(&hex::decode(empty_code_hash).unwrap());
    let empty_account = stream.out();

    let is_empty_slot = value.len() == 1 && value[0] == 0x80;
    let is_empty_account = value == &empty_account;
    is_empty_slot || is_empty_account
}

fn shared_prefix_length(path: &Vec<u8>, path_offset: usize, node_path: &Vec<u8>) -> usize {
    let skip_length = skip_length(node_path);
    let mut node_decoded = vec![];
    for i in skip_length..node_path.len() * 2 {
        let decoded_nibble_offset = i - skip_length;
        if decoded_nibble_offset % 2 == 0 {
            let shifted = get_nibble(node_path, i) << 4;
            node_decoded.push(shifted);
        } else {
            let byte = &node_decoded.get(decoded_nibble_offset / 2).unwrap().clone();
            let right = get_nibble(node_path, i);
            node_decoded.pop();
            node_decoded.push(byte | right);
        }
    }

    let len = node_decoded.len() * 2;
    let mut prefix_len = 0;

    for i in 0..len {
        let path_nibble = get_nibble(path, i + path_offset);
        let node_path_nibble = get_nibble(&node_decoded, i);

        if path_nibble == node_path_nibble {
            prefix_len += 1;
        }
    }

    prefix_len
}

fn skip_length(node: &Vec<u8>) -> usize {
    let nibble = get_nibble(node, 0);
    match nibble {
        0 => 2,
        1 => 1,
        2 => 2,
        3 => 1,
        _ => 0,
    }
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
