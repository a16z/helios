use std::{collections::HashMap, fmt};

use consensus::types::ExecutionPayload;
use ethers::{
    prelude::{Address, H256, U256},
    types::Transaction,
    utils::rlp::{Decodable, Rlp},
};
use eyre::Result;
use serde::{ser::SerializeSeq, Deserialize, Serialize};

use common::{types::Block, utils::u64_to_hex_string};

#[derive(Default, Debug, Clone)]
pub struct Account {
    pub balance: U256,
    pub nonce: u64,
    pub code_hash: H256,
    pub code: Vec<u8>,
    pub storage_hash: H256,
    pub slots: HashMap<H256, U256>,
}

#[derive(Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CallOpts {
    pub from: Option<Address>,
    pub to: Option<Address>,
    pub gas: Option<U256>,
    pub gas_price: Option<U256>,
    pub value: Option<U256>,
    #[serde(default, deserialize_with = "bytes_deserialize")]
    pub data: Option<Vec<u8>>,
}

impl fmt::Debug for CallOpts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CallOpts")
            .field("from", &self.from)
            .field("to", &self.to)
            .field("value", &self.value)
            .field("data", &hex::encode(self.data.clone().unwrap_or_default()))
            .finish()
    }
}

// impl From<ExecutionPayload> for Block<Transaction> {
//     fn from(payload: ExecutionPayload) -> Self {
//         let empty_nonce = "0x0000000000000000".to_string();
//         let empty_uncle_hash = "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347";
//
//         let txs = payload.transactions().into_iter().map(|tx| {
//             Transaction::decode(&Rlp::new(tx.as_slice())).unwrap()
//         }).collect();
//
//         Block<Transaction> {
//             number: payload.block_number().as_u64(),
//             base_fee_per_gas: U256::from_little_endian(&payload.base_fee_per_gas().to_bytes_le()),
//             difficulty: U256::from(0),
//             extra_data: payload.extra_data().to_vec(),
//             gas_limit: payload.gas_limit().as_u64(),
//             gas_used: payload.gas_used().as_u64(),
//             hash: H256::from_slice(payload.block_hash()),
//             logs_bloom: payload.logs_bloom().to_vec(),
//             miner: Address::from_slice(payload.fee_recipient()),
//             parent_hash: H256::from_slice(payload.parent_hash()),
//             receipts_root: H256::from_slice(payload.receipts_root()),
//             state_root: H256::from_slice(payload.state_root()),
//             timestamp: payload.timestamp().as_u64(),
//             total_difficulty: 0,
//             transactions: txs,
//             mix_hash: H256::from_slice(payload.prev_randao()),
//             nonce: empty_nonce,
//             sha3_uncles: H256::from_str(empty_uncle_hash)?,
//             size: 0,
//             transactions_root: H256::default(),
//             uncles: vec![],
//         }
//     }
// }

fn bytes_deserialize<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let bytes: Option<String> = serde::Deserialize::deserialize(deserializer)?;
    match bytes {
        Some(bytes) => {
            let bytes = hex::decode(bytes.strip_prefix("0x").unwrap_or("")).unwrap_or_default();
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
