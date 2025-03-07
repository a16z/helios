use std::fmt::Display;

use alloy::{
    consensus::Account as TrieAccount,
    eips::{BlockId, BlockNumberOrTag},
    primitives::{Bytes, B256, U256},
    rpc::types::EIP1186StorageProof,
};
use eyre::{eyre, Report, Result};
use serde::{de::Error, Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub account: TrieAccount,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<Bytes>,
    pub account_proof: Vec<Bytes>,
    pub storage_proof: Vec<EIP1186StorageProof>,
}

impl Account {
    /// Retrieve the value at the given storage slot.
    pub fn get_storage_value(&self, slot: B256) -> Option<U256> {
        self.storage_proof
            .iter()
            .find_map(|EIP1186StorageProof { key, value, .. }| {
                if key.as_b256() == slot {
                    Some(*value)
                } else {
                    None
                }
            })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum BlockTag {
    Latest,
    Finalized,
    Number(u64),
}

impl From<u64> for BlockTag {
    fn from(num: u64) -> Self {
        BlockTag::Number(num)
    }
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

impl From<BlockTag> for BlockId {
    fn from(block_tag: BlockTag) -> Self {
        match block_tag {
            BlockTag::Latest => BlockId::latest(),
            BlockTag::Finalized => BlockId::finalized(),
            BlockTag::Number(num) => BlockId::Number(num.into()),
        }
    }
}

impl TryFrom<BlockNumberOrTag> for BlockTag {
    type Error = Report;

    fn try_from(tag: BlockNumberOrTag) -> Result<Self, Self::Error> {
        match tag {
            BlockNumberOrTag::Number(num) => Ok(BlockTag::Number(num)),
            BlockNumberOrTag::Latest => Ok(BlockTag::Latest),
            BlockNumberOrTag::Finalized => Ok(BlockTag::Finalized),
            other => Err(eyre!("block tag {other} is not supported")),
        }
    }
}

impl TryFrom<BlockId> for BlockTag {
    type Error = Report;

    fn try_from(block_id: BlockId) -> Result<Self, Self::Error> {
        match block_id {
            BlockId::Number(BlockNumberOrTag::Number(num)) => Ok(BlockTag::Number(num)),
            BlockId::Number(BlockNumberOrTag::Latest) => Ok(BlockTag::Latest),
            BlockId::Number(BlockNumberOrTag::Finalized) => Ok(BlockTag::Finalized),
            BlockId::Number(other) => Err(eyre!("block tag {other} is not supported")),
            BlockId::Hash(_) => Err(eyre!("block hash is not supported")),
        }
    }
}
