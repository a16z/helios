use ethers::types::{Bytes, EIP1186ProofResponse};
use ethers::utils::keccak256;
use ethers::utils::rlp::{decode_list, RlpStream};

pub fn verify_proof(proof: &Vec<Bytes>, root: &Vec<u8>, path: &Vec<u8>, value: &Vec<u8>) -> bool {
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
                let nibble = get_nibble(&path, path_offset);
                expected_hash = node_list[nibble as usize].clone();

                path_offset += 1;
            }
        } else if node_list.len() == 2 {
            if i == proof.len() - 1 {
                // exclusion proof
                if !paths_match(
                    &node_list[0],
                    skip_length(&node_list[0]),
                    &path,
                    path_offset,
                ) && is_empty_value(value)
                {
                    return true;
                }

                // inclusion proof
                if &node_list[1] == value {
                    return paths_match(
                        &node_list[0],
                        skip_length(&node_list[0]),
                        &path,
                        path_offset,
                    );
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

fn paths_match(p1: &Vec<u8>, s1: usize, p2: &Vec<u8>, s2: usize) -> bool {
    let len1 = p1.len() * 2 - s1;
    let len2 = p2.len() * 2 - s2;

    if len1 != len2 {
        return false;
    }

    for offset in 0..len1 {
        let n1 = get_nibble(p1, s1 + offset);
        let n2 = get_nibble(p2, s2 + offset);

        if n1 != n2 {
            return false;
        }
    }

    true
}

#[allow(dead_code)]
fn get_rest_path(p: &Vec<u8>, s: usize) -> String {
    let mut ret = String::new();
    for i in s..p.len() * 2 {
        let n = get_nibble(p, i);
        ret += &format!("{:01x}", n);
    }
    ret
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
        } else {
            break;
        }
    }

    prefix_len
}

fn skip_length(node: &Vec<u8>) -> usize {
    if node.len() == 0 {
        return 0;
    }

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

pub fn encode_account(proof: &EIP1186ProofResponse) -> Vec<u8> {
    let mut stream = RlpStream::new_list(4);
    stream.append(&proof.nonce);
    stream.append(&proof.balance);
    stream.append(&proof.storage_hash);
    stream.append(&proof.code_hash);
    let encoded = stream.out();
    encoded.to_vec()
}

#[cfg(test)]
mod tests {
    use crate::proof::shared_prefix_length;

    #[tokio::test]
    fn test_shared_prefix_length() {
        // We compare the path starting from the 6th nibble i.e. the 6 in 0x6f
        let path: Vec<u8> = vec![0x12, 0x13, 0x14, 0x6f, 0x6c, 0x64, 0x21];
        let path_offset = 6;
        // Our node path matches only the first 5 nibbles of the path
        let node_path: Vec<u8> = vec![0x6f, 0x6c, 0x63, 0x21];
        let shared_len = shared_prefix_length(&path, path_offset, &node_path);
        assert_eq!(shared_len, 5);

        // Now we compare the path starting from the 5th nibble i.e. the 4 in 0x14
        let path: Vec<u8> = vec![0x12, 0x13, 0x14, 0x6f, 0x6c, 0x64, 0x21];
        let path_offset = 6;
        // Our node path matches only the first 7 nibbles of the path
        // Note the first nibble is 1, so we skip 1 nibble
        let node_path: Vec<u8> = vec![0x14, 0x6f, 0x6c, 0x64, 0x11];
        let shared_len = shared_prefix_length(&path, path_offset, &node_path);
        assert_eq!(shared_len, 7);
    }
}
