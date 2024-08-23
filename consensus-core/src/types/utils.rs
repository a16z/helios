use alloy::consensus::{Transaction as TxTrait, TxEnvelope};
use alloy::primitives::{hex::FromHex, B256, U256, U64};
use alloy::rlp::Decodable;
use alloy::rpc::types::{Parity, Signature, Transaction};
// use serde::de::Error;

use super::{ExecutionPayload, Header};
use common::types::{Block, Transactions};

// pub fn u256_deserialize<'de, D>(deserializer: D) -> Result<U256, D::Error>
// where
//     D: serde::Deserializer<'de>,
// {
//     let val: String = serde::Deserialize::deserialize(deserializer)?;
//     let x = AU256::from_str_radix(&val, 10).map_err(D::Error::custom)?;
//     let x_bytes = x.to_le_bytes();
//     Ok(U256::from_bytes_le(x_bytes))
// }

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

impl From<ExecutionPayload> for Block {
    fn from(value: ExecutionPayload) -> Block {
        let empty_nonce = "0x0000000000000000".to_string();
        let empty_uncle_hash = "1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347";

        let txs = value
            .transactions()
            .iter()
            .enumerate()
            .map(|(i, tx_bytes)| {
                let tx_bytes = tx_bytes.to_vec();
                let mut tx_bytes_slice = tx_bytes.as_slice();
                let tx_envelope = TxEnvelope::decode(&mut tx_bytes_slice).unwrap();

                let mut tx = Transaction {
                    hash: *tx_envelope.tx_hash(),
                    nonce: tx_envelope.nonce(),
                    block_hash: Some(*value.block_hash()),
                    block_number: Some(*value.block_number()),
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
                            v: U256::from(inner.signature().v().to_u64()),
                            y_parity: None,
                        });
                    }
                    TxEnvelope::Eip2930(inner) => {
                        tx.from = inner.recover_signer().unwrap();
                        tx.signature = Some(Signature {
                            r: inner.signature().r(),
                            s: inner.signature().s(),
                            v: U256::from(inner.signature().v().to_u64()),
                            y_parity: Some(Parity(inner.signature().v().to_u64() == 1)),
                        });
                        tx.access_list = Some(inner.tx().access_list.clone());
                    }
                    TxEnvelope::Eip1559(inner) => {
                        tx.from = inner.recover_signer().unwrap();
                        tx.signature = Some(Signature {
                            r: inner.signature().r(),
                            s: inner.signature().s(),
                            v: U256::from(inner.signature().v().to_u64()),
                            y_parity: Some(Parity(inner.signature().v().to_u64() == 1)),
                        });

                        let tx_inner = inner.tx();
                        tx.access_list = Some(tx_inner.access_list.clone());
                        tx.max_fee_per_gas = Some(tx_inner.max_fee_per_gas);
                        tx.max_priority_fee_per_gas = Some(tx_inner.max_priority_fee_per_gas);

                        tx.gas_price = Some(gas_price(
                            tx_inner.max_fee_per_gas,
                            tx_inner.max_priority_fee_per_gas,
                            value.base_fee_per_gas().to(),
                        ));
                    }
                    TxEnvelope::Eip4844(inner) => {
                        tx.from = inner.recover_signer().unwrap();
                        tx.signature = Some(Signature {
                            r: inner.signature().r(),
                            s: inner.signature().s(),
                            v: U256::from(inner.signature().v().to_u64()),
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
                            value.base_fee_per_gas().to(),
                        ));
                    }
                    _ => todo!(),
                }

                tx
            })
            .collect::<Vec<Transaction>>();

        Block {
            number: U64::from(*value.block_number()),
            base_fee_per_gas: *value.base_fee_per_gas(),
            difficulty: U256::ZERO,
            extra_data: value.extra_data().to_vec().into(),
            gas_limit: U64::from(*value.gas_limit()),
            gas_used: U64::from(*value.gas_used()),
            hash: *value.block_hash(),
            logs_bloom: value.logs_bloom().to_vec().into(),
            miner: *value.fee_recipient(),
            parent_hash: *value.parent_hash(),
            receipts_root: *value.receipts_root(),
            state_root: *value.state_root(),
            timestamp: U64::from(*value.timestamp()),
            total_difficulty: U64::ZERO,
            transactions: Transactions::Full(txs),
            mix_hash: *value.prev_randao(),
            nonce: empty_nonce,
            sha3_uncles: B256::from_hex(empty_uncle_hash).unwrap(),
            size: U64::ZERO,
            transactions_root: B256::default(),
            uncles: vec![],
            blob_gas_used: value.blob_gas_used().map(|v| U64::from(*v)).ok(),
            excess_blob_gas: value.excess_blob_gas().map(|v| U64::from(*v)).ok(),
        }
    }
}

fn gas_price(max_fee: u128, max_prio_fee: u128, base_fee: u128) -> u128 {
    u128::min(max_fee, max_prio_fee + base_fee)
}
