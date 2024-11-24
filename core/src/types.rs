use std::fmt::Display;

use alloy::consensus::{Header, EMPTY_OMMER_ROOT_HASH};
use alloy::network::TransactionResponse;
use alloy::primitives::{Address, Bloom, Bytes, FixedBytes, B256, U256, U64};
use serde::{de::Error, ser::SerializeSeq, Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Block<T: TransactionResponse> {
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
    pub nonce: FixedBytes<8>,
    pub parent_hash: B256,
    pub receipts_root: B256,
    pub sha3_uncles: B256,
    pub size: U64,
    pub state_root: B256,
    pub timestamp: U64,
    #[serde(default)]
    pub total_difficulty: U64,
    pub transactions: Transactions<T>,
    pub transactions_root: B256,
    pub uncles: Vec<B256>,
    pub withdrawals_root: B256,
    pub blob_gas_used: Option<U64>,
    pub excess_blob_gas: Option<U64>,
    pub parent_beacon_block_root: Option<B256>,
}

impl<T: TransactionResponse> Block<T> {
    pub fn is_hash_valid(&self) -> bool {
        let header = Header {
            parent_hash: self.parent_hash,
            ommers_hash: EMPTY_OMMER_ROOT_HASH,
            beneficiary: self.miner,
            state_root: self.state_root,
            transactions_root: self.transactions_root,
            receipts_root: self.receipts_root,
            withdrawals_root: Some(self.withdrawals_root),
            logs_bloom: Bloom::from_slice(&self.logs_bloom),
            difficulty: self.difficulty,
            number: self.number.to(),
            gas_limit: self.gas_limit.to(),
            gas_used: self.gas_used.to(),
            timestamp: self.timestamp.to(),
            mix_hash: self.mix_hash,
            nonce: self.nonce,
            base_fee_per_gas: Some(self.base_fee_per_gas.to()),
            blob_gas_used: self.blob_gas_used.map(|v| v.to()),
            excess_blob_gas: self.excess_blob_gas.map(|v| v.to()),
            parent_beacon_block_root: self.parent_beacon_block_root,
            requests_root: None,
            extra_data: self.extra_data.clone(),
        };

        header.hash_slow() == self.hash
    }
}

#[derive(Debug, Clone)]
pub enum Transactions<T: TransactionResponse> {
    Hashes(Vec<B256>),
    Full(Vec<T>),
}

impl<T: TransactionResponse> Default for Transactions<T> {
    fn default() -> Self {
        Self::Full(Vec::new())
    }
}

impl<T: TransactionResponse> Transactions<T> {
    pub fn hashes(&self) -> Vec<B256> {
        match self {
            Self::Hashes(hashes) => hashes.clone(),
            Self::Full(txs) => txs.iter().map(|tx| tx.tx_hash()).collect(),
        }
    }
}

impl<T: TransactionResponse + Serialize> Serialize for Transactions<T> {
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

impl<'de, T: TransactionResponse + Deserialize<'de>> Deserialize<'de> for Transactions<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let txs: Vec<T> = serde::Deserialize::deserialize(deserializer)?;
        Ok(Transactions::Full(txs))
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
