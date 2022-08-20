use eyre::Result;
use ssz_rs::{Node, Vector};

use crate::consensus_rpc::Bytes32;

pub fn hex_str_to_bytes(s: &str) -> Result<Vec<u8>> {
    let stripped = s.strip_prefix("0x").unwrap_or(s);
    Ok(hex::decode(stripped)?)
}

pub fn bytes32_to_node(bytes: &Bytes32) -> Result<Node> {
    Ok(Node::from_bytes(bytes.as_slice().try_into()?))
}

pub fn bytes_to_bytes32(bytes: &[u8]) -> Bytes32 {
    Vector::from_iter(bytes.to_vec())
}
