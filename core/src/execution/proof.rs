use alloy::consensus::Account;
use alloy::primitives::{b256, keccak256, Bytes, U256};
use alloy::rlp::{encode, Decodable};
use alloy::rpc::types::EIP1186AccountProofResponse;

pub fn verify_proof(proof: &[Bytes], root: &[u8], path: &[u8], value: &[u8]) -> bool {
    let mut expected_hash = root.to_vec();
    let mut path_offset = 0;

    for (i, node) in proof.iter().enumerate() {
        if expected_hash != keccak256(node).to_vec() {
            return false;
        }

        let mut node = &node[..];
        let node_list: Vec<Bytes> = Vec::decode(&mut node).unwrap();

        if node_list.len() == 17 {
            if i == proof.len() - 1 {
                // exclusion proof
                let nibble = get_nibble(path, path_offset);
                let node = &node_list[nibble as usize];

                if node.is_empty() && is_empty_value(value) {
                    return true;
                }
            } else {
                let nibble = get_nibble(path, path_offset);
                expected_hash.clone_from(&node_list[nibble as usize].to_vec());

                path_offset += 1;
            }
        } else if node_list.len() == 2 {
            if i == proof.len() - 1 {
                // exclusion proof
                if !paths_match(&node_list[0], skip_length(&node_list[0]), path, path_offset)
                    && is_empty_value(value)
                {
                    return true;
                }

                // inclusion proof
                if &node_list[1] == value {
                    return paths_match(
                        &node_list[0],
                        skip_length(&node_list[0]),
                        path,
                        path_offset,
                    );
                }
            } else {
                let node_path = &node_list[0];
                let prefix_length = shared_prefix_length(path, path_offset, node_path);
                if prefix_length < node_path.len() * 2 - skip_length(node_path) {
                    // The proof shows a divergent path, but we're not
                    // at the end of the proof, so something's wrong.
                    return false;
                }
                path_offset += prefix_length;
                expected_hash.clone_from(&node_list[1].to_vec());
            }
        } else {
            return false;
        }
    }

    false
}

fn paths_match(p1: &[u8], s1: usize, p2: &[u8], s2: usize) -> bool {
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
fn get_rest_path(p: &[u8], s: usize) -> String {
    let mut ret = String::new();
    for i in s..p.len() * 2 {
        let n = get_nibble(p, i);
        ret += &format!("{n:01x}");
    }
    ret
}

fn is_empty_value(value: &[u8]) -> bool {
    let empty_account = Account {
        nonce: 0,
        balance: U256::ZERO,
        storage_root: b256!("56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421"),
        code_hash: b256!("c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"),
    };

    let empty_account = encode(empty_account);

    let is_empty_slot = value.len() == 1 && value[0] == 0x80;
    let is_empty_account = value == empty_account;
    is_empty_slot || is_empty_account
}

fn shared_prefix_length(path: &[u8], path_offset: usize, node_path: &[u8]) -> usize {
    let skip_length = skip_length(node_path);

    let len = std::cmp::min(
        node_path.len() * 2 - skip_length,
        path.len() * 2 - path_offset,
    );
    let mut prefix_len = 0;

    for i in 0..len {
        let path_nibble = get_nibble(path, i + path_offset);
        let node_path_nibble = get_nibble(node_path, i + skip_length);

        if path_nibble == node_path_nibble {
            prefix_len += 1;
        } else {
            break;
        }
    }

    prefix_len
}

fn skip_length(node: &[u8]) -> usize {
    if node.is_empty() {
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

fn get_nibble(path: &[u8], offset: usize) -> u8 {
    let byte = path[offset / 2];
    if offset % 2 == 0 {
        byte >> 4
    } else {
        byte & 0xF
    }
}

pub fn encode_account(proof: &EIP1186AccountProofResponse) -> Vec<u8> {
    let account = Account {
        nonce: proof.nonce,
        balance: proof.balance,
        storage_root: proof.storage_hash,
        code_hash: proof.code_hash,
    };

    encode(account)
}

#[cfg(test)]
mod tests {
    use crate::execution::proof::shared_prefix_length;

    #[tokio::test]
    async fn test_shared_prefix_length() {
        // We compare the path starting from the 6th nibble i.e. the 6 in 0x6f
        let path: Vec<u8> = vec![0x12, 0x13, 0x14, 0x6f, 0x6c, 0x64, 0x21];
        let path_offset = 6;
        // Our node path matches only the first 5 nibbles of the path
        let node_path: Vec<u8> = vec![0x6f, 0x6c, 0x63, 0x21];
        let shared_len = shared_prefix_length(&path, path_offset, &node_path);
        assert_eq!(shared_len, 5);

        // Now we compare the path starting from the 5th nibble i.e. the 4 in 0x14
        let path: Vec<u8> = vec![0x12, 0x13, 0x14, 0x6f, 0x6c, 0x64, 0x21];
        let path_offset = 5;
        // Our node path matches only the first 7 nibbles of the path
        // Note the first nibble is 1, so we skip 1 nibble
        let node_path: Vec<u8> = vec![0x14, 0x6f, 0x6c, 0x64, 0x11];
        let shared_len = shared_prefix_length(&path, path_offset, &node_path);
        assert_eq!(shared_len, 7);
    }
}
