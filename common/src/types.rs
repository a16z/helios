use std::fmt::Display;

use alloy::primitives::{Address, Bytes, B256, U256, U64};
use alloy::rpc::types::Transaction;
use serde::{de::Error, ser::SerializeSeq, Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    pub number: U64,
    pub base_fee_per_gas: U256,
    pub difficulty: U256,
    pub extra_data: Bytes,
    pub gas_limit: U64,
    pub gas_used: U64,
    pub hash: B256,
    pub logs_bloom: Bytes,
    pub miner: Address,
    pub mix_hash: B256,
    pub nonce: String,
    pub parent_hash: B256,
    pub receipts_root: B256,
    pub sha3_uncles: B256,
    pub size: U64,
    pub state_root: B256,
    pub timestamp: U64,
    pub total_difficulty: U64,
    pub transactions: Transactions,
    pub transactions_root: B256,
    pub uncles: Vec<B256>,
    pub blob_gas_used: Option<U64>,
    pub excess_blob_gas: Option<U64>,
}

#[derive(Deserialize, Debug, Clone)]
pub enum Transactions {
    Hashes(Vec<B256>),
    Full(Vec<Transaction>),
}

impl Default for Transactions {
    fn default() -> Self {
        Self::Full(Vec::new())
    }
}

impl Transactions {
    pub fn hashes(&self) -> Vec<B256> {
        match self {
            Self::Hashes(hashes) => hashes.clone(),
            Self::Full(txs) => txs.iter().map(|tx| tx.hash).collect(),
        }
    }
}

impl Serialize for Transactions {
    fn serialize<S>(&self, s: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Transactions::Hashes(hashes) => {
                let mut seq = s.serialize_seq(Some(hashes.len()))?;
                for hash in hashes {
                    seq.serialize_element(&hash)?;
                }

                seq.end()
            }
            Transactions::Full(txs) => {
                let mut seq = s.serialize_seq(Some(txs.len()))?;
                for tx in txs {
                    seq.serialize_element(&tx)?;
                }

                seq.end()
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum BlockTag {
    Latest,
    Finalized,
    Number(u64),
}

impl Display for BlockTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let formatted = match self {
            Self::Latest => "latest".to_string(),
            Self::Finalized => "finalized".to_string(),
            Self::Number(num) => num.to_string(),
        };

        write!(f, "{formatted}")
    }
}

impl<'de> Deserialize<'de> for BlockTag {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let block: String = serde::Deserialize::deserialize(deserializer)?;
        let parse_error = D::Error::custom("could not parse block tag");

        let block_tag = match block.as_str() {
            "latest" => BlockTag::Latest,
            "finalized" => BlockTag::Finalized,
            _ => match block.strip_prefix("0x") {
                Some(hex_block) => {
                    let num = u64::from_str_radix(hex_block, 16).map_err(|_| parse_error)?;

                    BlockTag::Number(num)
                }
                None => {
                    let num = block.parse().map_err(|_| parse_error)?;

                    BlockTag::Number(num)
                }
            },
        };

        Ok(block_tag)
    }
}
