use discv5::enr::{self, CombinedKey};
use ethers::prelude::Address;
use eyre::Result;
use libp2p_core::identity::Keypair;
use ssz_rs::{Node, Vector};

use super::types::Bytes32;

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
    format!("0x{val:x}")
}

/// Transforms a [Keypair](libp2p_core::identity::Keypair) into a [CombinedKey].
pub fn from_libp2p(key: &libp2p_core::identity::Keypair) -> Result<CombinedKey, &'static str> {
    match key {
        Keypair::Secp256k1(key) => {
            let secret = enr::k256::ecdsa::SigningKey::from_bytes(&key.secret().to_bytes())
                .expect("libp2p key must be valid");
            Ok(CombinedKey::Secp256k1(secret))
        }
        Keypair::Ed25519(key) => {
            let ed_keypair = enr::ed25519_dalek::SecretKey::from_bytes(&key.encode()[..32])
                .expect("libp2p key must be valid");
            Ok(CombinedKey::from(ed_keypair))
        }
        _ => Err("ENR: Unsupported libp2p key type"),
    }
}
