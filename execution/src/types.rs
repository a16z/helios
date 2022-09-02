use std::collections::HashMap;

use ethers::prelude::{Address, H256, U256};
use eyre::Result;
use serde::de::Error;
use serde::{Deserialize, Serialize};

use common::utils::{hex_str_to_bytes, u64_to_hex_string};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Proof {
    pub address: Address,
    pub balance: U256,
    pub code_hash: H256,
    pub nonce: U256,
    pub storage_hash: H256,
    #[serde(deserialize_with = "proof_deserialize")]
    pub account_proof: Vec<Vec<u8>>,
    pub storage_proof: Vec<StorageProof>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct StorageProof {
    pub key: U256,
    pub value: U256,
    #[serde(deserialize_with = "proof_deserialize")]
    pub proof: Vec<Vec<u8>>,
}

#[derive(Default, Debug)]
pub struct Account {
    pub balance: U256,
    pub nonce: U256,
    pub code_hash: H256,
    pub storage_hash: H256,
    pub slots: HashMap<U256, U256>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionBlock {
    #[serde(serialize_with = "serialize_u64_string")]
    pub number: u64,
    pub base_fee_per_gas: U256,
    pub difficulty: U256,
    #[serde(serialize_with = "serialize_bytes")]
    pub extra_data: Vec<u8>,
    #[serde(serialize_with = "serialize_u64_string")]
    pub gas_limit: u64,
    #[serde(serialize_with = "serialize_u64_string")]
    pub gas_used: u64,
    pub hash: H256,
    #[serde(serialize_with = "serialize_bytes")]
    pub logs_bloom: Vec<u8>,
    pub miner: Address,
    pub mix_hash: H256,
    pub nonce: String,
    pub parent_hash: H256,
    pub receipts_root: H256,
    pub sha3_uncles: H256,
    #[serde(serialize_with = "serialize_u64_string")]
    pub size: u64,
    pub state_root: H256,
    #[serde(serialize_with = "serialize_u64_string")]
    pub timestamp: u64,
    #[serde(serialize_with = "serialize_u64_string")]
    pub total_difficulty: u64,
    pub transactions: Vec<H256>,
    pub transactions_root: H256,
    pub uncles: Vec<H256>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CallOpts {
    pub from: Option<Address>,
    pub to: Address,
    pub gas: Option<U256>,
    pub gas_price: Option<U256>,
    pub value: Option<U256>,
    #[serde(default, deserialize_with = "bytes_deserialize")]
    pub data: Option<Vec<u8>>,
}

fn bytes_deserialize<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let bytes: Option<String> = serde::Deserialize::deserialize(deserializer)?;
    match bytes {
        Some(bytes) => {
            let bytes = hex::decode(bytes.strip_prefix("0x").unwrap()).unwrap();
            Ok(Some(bytes.to_vec()))
        }
        None => Ok(None),
    }
}

fn serialize_bytes<S>(bytes: &Vec<u8>, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let bytes_str = format!("0x{}", hex::encode(bytes));
    s.serialize_str(&bytes_str)
}

fn serialize_u64_string<S>(x: &u64, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let num_string = u64_to_hex_string(*x);
    s.serialize_str(&num_string)
}

fn proof_deserialize<'de, D>(deserializer: D) -> Result<Vec<Vec<u8>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let branch: Vec<String> = serde::Deserialize::deserialize(deserializer)?;
    Ok(branch
        .iter()
        .map(|elem| hex_str_to_bytes(elem))
        .collect::<Result<_>>()
        .map_err(D::Error::custom)?)
}
