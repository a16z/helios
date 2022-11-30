use ethers::{
    abi::AbiEncode,
    types::{Address, U256},
};
use eyre::Result;
use ssz_rs::{Node, Vector};

use super::types::Bytes32;

pub fn format_hex(num: &U256) -> String {
    let stripped = num
        .encode_hex()
        .strip_prefix("0x")
        .unwrap()
        .trim_start_matches('0')
        .to_string();
    format!("0x{}", stripped)
}

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

pub fn address_to_hex_string(address: &Address) -> String {
    format!("0x{}", hex::encode(address.as_bytes()))
}

pub fn u64_to_hex_string(val: u64) -> String {
    format!("0x{:x}", val)
}
