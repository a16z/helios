use std::str::FromStr;

use ethers::{
    types::{Address, Transaction, H256},
    utils::rlp::{Decodable, Rlp},
};
use serde::de::Error;
use ssz_rs::prelude::*;

use super::{ExecutionPayload, Header};
use common::types::{Block, Transactions};

pub fn u256_deserialize<'de, D>(deserializer: D) -> Result<U256, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let val: String = serde::Deserialize::deserialize(deserializer)?;
    let x = ethers::types::U256::from_dec_str(&val).map_err(D::Error::custom)?;
    let mut x_bytes = [0; 32];
    x.to_little_endian(&mut x_bytes);
    Ok(U256::from_bytes_le(x_bytes))
}

pub fn header_deserialize<'de, D>(deserializer: D) -> Result<Header, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let header: LightClientHeader = serde::Deserialize::deserialize(deserializer)?;

    Ok(match header {
        LightClientHeader::Unwrapped(header) => header,
        LightClientHeader::Wrapped(header) => header.beacon,
    })
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum LightClientHeader {
    Unwrapped(Header),
    Wrapped(Beacon),
}

#[derive(serde::Deserialize)]
struct Beacon {
    beacon: Header,
}

macro_rules! superstruct_ssz {
    ($type:tt) => {
        impl ssz_rs::Merkleized for $type {
            fn hash_tree_root(&mut self) -> Result<Node, MerkleizationError> {
                match self {
                    $type::Bellatrix(inner) => inner.hash_tree_root(),
                    $type::Capella(inner) => inner.hash_tree_root(),
                    $type::Deneb(inner) => inner.hash_tree_root(),
                }
            }
        }

        impl ssz_rs::Sized for $type {
            fn is_variable_size() -> bool {
                true
            }

            fn size_hint() -> usize {
                0
            }
        }

        impl ssz_rs::Serialize for $type {
            fn serialize(&self, buffer: &mut Vec<u8>) -> Result<usize, SerializeError> {
                match self {
                    $type::Bellatrix(inner) => inner.serialize(buffer),
                    $type::Capella(inner) => inner.serialize(buffer),
                    $type::Deneb(inner) => inner.serialize(buffer),
                }
            }
        }

        impl ssz_rs::Deserialize for $type {
            fn deserialize(_encoding: &[u8]) -> Result<Self, DeserializeError>
            where
                Self: Sized,
            {
                panic!("not implemented");
            }
        }

        impl ssz_rs::SimpleSerialize for $type {}
    };
}

/// this has to go after macro definition
pub(crate) use superstruct_ssz;

impl From<ExecutionPayload> for Block {
    fn from(value: ExecutionPayload) -> Block {
        let empty_nonce = "0x0000000000000000".to_string();
        let empty_uncle_hash = "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347";

        let txs = value
            .transactions()
            .iter()
            .enumerate()
            .map(|(i, tx)| {
                let rlp = Rlp::new(tx.as_slice());
                let mut tx = Transaction::decode(&rlp).unwrap();

                tx.block_number = Some(value.block_number().as_u64().into());
                tx.block_hash = Some(H256::from_slice(value.block_hash()));
                tx.from = tx.recover_from().unwrap();
                tx.transaction_index = Some(i.into());

                if let (Some(max_fee), Some(max_priority_fee)) =
                    (tx.max_fee_per_gas, tx.max_priority_fee_per_gas)
                {
                    let base_fee = ethers::types::U256::from_little_endian(
                        &value.base_fee_per_gas().to_bytes_le(),
                    );

                    tx.gas_price = if max_fee >= max_priority_fee + base_fee {
                        Some(base_fee + max_priority_fee)
                    } else {
                        Some(max_fee)
                    };
                }

                tx
            })
            .collect::<Vec<Transaction>>();

        Block {
            number: value.block_number().as_u64().into(),
            base_fee_per_gas: ethers::types::U256::from_little_endian(
                &value.base_fee_per_gas().to_bytes_le(),
            ),
            difficulty: ethers::types::U256::from(0),
            extra_data: value.extra_data().to_vec().into(),
            gas_limit: value.gas_limit().as_u64().into(),
            gas_used: value.gas_used().as_u64().into(),
            hash: H256::from_slice(value.block_hash()),
            logs_bloom: value.logs_bloom().to_vec().into(),
            miner: Address::from_slice(value.fee_recipient()),
            parent_hash: H256::from_slice(value.parent_hash()),
            receipts_root: H256::from_slice(value.receipts_root()),
            state_root: H256::from_slice(value.state_root()),
            timestamp: value.timestamp().as_u64().into(),
            total_difficulty: 0.into(),
            transactions: Transactions::Full(txs),
            mix_hash: H256::from_slice(value.prev_randao()),
            nonce: empty_nonce,
            sha3_uncles: H256::from_str(empty_uncle_hash).unwrap(),
            size: 0.into(),
            transactions_root: H256::default(),
            uncles: vec![],
        }
    }
}
