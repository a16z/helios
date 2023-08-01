use ethers::prelude::Address;
use eyre::Result;
use ssz_rs::{Node, Vector};

use super::types::Bytes32;

pub fn hex_str_to_bytes(s: &str) -> Result<Vec<u8>> {
    let stripped = s.strip_prefix("0x").unwrap_or(s);
    Ok(hex::decode(stripped)?)
}

pub fn bytes32_to_node(bytes: &Bytes32) -> Result<Node> {
    Ok(Node::try_from(bytes.as_slice())?)
}

pub fn bytes_to_bytes32(bytes: &[u8]) -> Bytes32 {
    Vector::try_from(bytes.to_vec()).unwrap()
}

pub fn address_to_hex_string(address: &Address) -> String {
    format!("0x{}", hex::encode(address.as_bytes()))
}

pub fn u64_to_hex_string(val: u64) -> String {
    format!("0x{val:x}")
}
