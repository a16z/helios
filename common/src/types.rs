use std::fmt::Display;

use serde::{de::Error, Deserialize};
use ssz_rs::Vector;

pub type Bytes32 = Vector<u8, 32>;

#[derive(Debug)]
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

        write!(f, "{}", formatted)
    }
}

impl<'de> Deserialize<'de> for BlockTag {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let block: String = serde::Deserialize::deserialize(deserializer)?;

        let block_tag = match block.as_str() {
            "latest" => BlockTag::Latest,
            "finalized" => BlockTag::Finalized,
            _ => match block.strip_prefix("0x") {
                Some(hex_block) => {
                    let num = u64::from_str_radix(hex_block, 16)
                        .map_err(|_| D::Error::custom("could not parse block tag"))?;
                    BlockTag::Number(num)
                }
                None => {
                    let num = block
                        .parse()
                        .map_err(|_| D::Error::custom("could not parse block tag"))?;
                    BlockTag::Number(num)
                }
            },
        };

        Ok(block_tag)
    }
}
