use alloy::consensus::{Transaction as TxTrait, TxEnvelope};
use alloy::primitives::{hex::FromHex, Address, B256, U256 as AU256, U64};
use alloy::rlp::Decodable;
use alloy::rpc::types::{Parity, Signature, Transaction};
use serde::de::Error;
use ssz_rs::prelude::*;

use super::{ExecutionPayload, Header};
use common::types::{Block, Transactions};

pub fn u256_deserialize<'de, D>(deserializer: D) -> Result<U256, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let val: String = serde::Deserialize::deserialize(deserializer)?;
    let x = AU256::from_str_radix(&val, 10).map_err(D::Error::custom)?;
    let x_bytes = x.to_le_bytes();
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

// this has to go after macro definition
pub(crate) use superstruct_ssz;

impl From<ExecutionPayload> for Block {
    fn from(value: ExecutionPayload) -> Block {
        let empty_nonce = "0x0000000000000000".to_string();
        let empty_uncle_hash = "1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347";
        let base_fee_per_gas = AU256::from_le_slice(&value.base_fee_per_gas().to_bytes_le());

        let txs = value
            .transactions()
            .iter()
            .enumerate()
            .map(|(i, tx_bytes)| {
                let mut tx_bytes = tx_bytes.as_slice();
                let tx_envelope = TxEnvelope::decode(&mut tx_bytes).unwrap();

                let mut tx = Transaction {
                    hash: *tx_envelope.tx_hash(),
                    nonce: tx_envelope.nonce(),
                    block_hash: Some(B256::from_slice(value.block_hash())),
                    block_number: Some(value.block_number().as_u64()),
                    transaction_index: Some(i as u64),
                    to: tx_envelope.to().to().cloned(),
                    value: tx_envelope.value(),
                    gas_price: tx_envelope.gas_price(),
                    gas: tx_envelope.gas_limit(),
                    input: tx_envelope.input().to_vec().into(),
                    chain_id: tx_envelope.chain_id(),
                    transaction_type: Some(tx_envelope.tx_type().into()),
                    ..Default::default()
                };

                match tx_envelope {
                    TxEnvelope::Legacy(inner) => {
                        tx.from = inner.recover_signer().unwrap();
                        tx.signature = Some(Signature {
                            r: inner.signature().r(),
                            s: inner.signature().s(),
                            v: AU256::from(inner.signature().v().to_u64()),
                            y_parity: None,
                        });
                    }
                    TxEnvelope::Eip2930(inner) => {
                        tx.from = inner.recover_signer().unwrap();
                        tx.signature = Some(Signature {
                            r: inner.signature().r(),
                            s: inner.signature().s(),
                            v: AU256::from(inner.signature().v().to_u64()),
                            y_parity: Some(Parity(inner.signature().v().to_u64() == 1)),
                        });
                        tx.access_list = Some(inner.tx().access_list.clone());
                    }
                    TxEnvelope::Eip1559(inner) => {
                        tx.from = inner.recover_signer().unwrap();
                        tx.signature = Some(Signature {
                            r: inner.signature().r(),
                            s: inner.signature().s(),
                            v: AU256::from(inner.signature().v().to_u64()),
                            y_parity: Some(Parity(inner.signature().v().to_u64() == 1)),
                        });

                        let tx_inner = inner.tx();
                        tx.access_list = Some(tx_inner.access_list.clone());
                        tx.max_fee_per_gas = Some(tx_inner.max_fee_per_gas);
                        tx.max_priority_fee_per_gas = Some(tx_inner.max_priority_fee_per_gas);

                        tx.gas_price = Some(gas_price(
                            tx_inner.max_fee_per_gas,
                            tx_inner.max_priority_fee_per_gas,
                            base_fee_per_gas.to(),
                        ));
                    }
                    TxEnvelope::Eip4844(inner) => {
                        tx.from = inner.recover_signer().unwrap();
                        tx.signature = Some(Signature {
                            r: inner.signature().r(),
                            s: inner.signature().s(),
                            v: AU256::from(inner.signature().v().to_u64()),
                            y_parity: Some(Parity(inner.signature().v().to_u64() == 1)),
                        });

                        let tx_inner = inner.tx().tx();
                        tx.access_list = Some(tx_inner.access_list.clone());
                        tx.max_fee_per_gas = Some(tx_inner.max_fee_per_gas);
                        tx.max_priority_fee_per_gas = Some(tx_inner.max_priority_fee_per_gas);
                        tx.max_fee_per_blob_gas = Some(tx_inner.max_fee_per_blob_gas);
                        tx.gas_price = Some(tx_inner.max_fee_per_gas);
                        tx.blob_versioned_hashes = Some(tx_inner.blob_versioned_hashes.clone());

                        tx.gas_price = Some(gas_price(
                            tx_inner.max_fee_per_gas,
                            tx_inner.max_priority_fee_per_gas,
                            base_fee_per_gas.to(),
                        ));
                    }
                    _ => todo!(),
                }

                tx
            })
            .collect::<Vec<Transaction>>();

        Block {
            number: U64::from(value.block_number().as_u64()),
            base_fee_per_gas,
            difficulty: AU256::ZERO,
            extra_data: value.extra_data().to_vec().into(),
            gas_limit: U64::from(value.gas_limit().as_u64()),
            gas_used: U64::from(value.gas_used().as_u64()),
            hash: B256::from_slice(value.block_hash()),
            logs_bloom: value.logs_bloom().to_vec().into(),
            miner: Address::from_slice(value.fee_recipient()),
            parent_hash: B256::from_slice(value.parent_hash()),
            receipts_root: B256::from_slice(value.receipts_root()),
            state_root: B256::from_slice(value.state_root()),
            timestamp: U64::from(value.timestamp().as_u64()),
            total_difficulty: U64::ZERO,
            transactions: Transactions::Full(txs),
            mix_hash: B256::from_slice(value.prev_randao()),
            nonce: empty_nonce,
            sha3_uncles: B256::from_hex(empty_uncle_hash).unwrap(),
            size: U64::ZERO,
            transactions_root: B256::default(),
            uncles: vec![],
        }
    }
}

fn gas_price(max_fee: u128, max_prio_fee: u128, base_fee: u128) -> u128 {
    u128::min(max_fee, max_prio_fee + base_fee)
}
