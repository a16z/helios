use ethers::prelude::Address;
use eyre::Result;
use ssz_rs::{Node, Vector};

use super::types::Bytes32;

pub fn hex_str_to_bytes(s: &str) -> Result<Vec<u8>> {
    let stripped = s.strip_prefix("0x").unwrap_or(s);
    Ok(hex::decode(stripped)?)
}

pub fn bytes_to_hex_str(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

pub fn bytes_vector_to_hex_str(bytes: &[Vector<u8, 48>]) -> Vec<String> {
    bytes.iter().map(|b| bytes_to_hex_str(b)).collect()
}

pub fn bytes32_to_node(bytes: &Bytes32) -> Result<Node> {
    Ok(Node::from_bytes(bytes.as_slice().try_into()?))
}

pub fn bytes_to_bytes32(bytes: &[u8]) -> Bytes32 {
    Vector::from_iter(bytes.to_vec())
}

pub fn address_to_hex_string(address: &Address) -> String {
    format!("0x{}", hex::encode(address.as_bytes()))
}

pub fn u64_to_hex_string(val: u64) -> String {
    format!("0x{val:x}")
}
