use ethers::prelude::Address;
use eyre::Result;

pub fn hex_str_to_bytes(s: &str) -> Result<Vec<u8>> {
    let stripped = s.strip_prefix("0x").unwrap_or(s);
    Ok(hex::decode(stripped)?)
}

pub fn address_to_hex_string(address: &Address) -> String {
    format!("0x{}", hex::encode(address.as_bytes()))
}

pub fn u64_to_hex_string(val: u64) -> String {
    format!("0x{val:x}")
}
