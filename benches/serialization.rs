use criterion::{criterion_group, criterion_main, Criterion};

use ethers::types::{Address, U256};
use serde::{Deserialize, Serialize};
use bincode::{config, Decode, Encode};

mod harness;

criterion_main!(serialization);
criterion_group!(serialization, callopts);

/// CallOpts is hoisted from [CallOpts](types::CallOpts).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
// #[derive(Encode, Decode)]
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallOpts {
    /// The address the transaction is sent from.
    pub from: Option<Address>,
    /// The address the transaction is directed to.
    pub to: Address,
    // #[bincode(with = "harness::u256")]
    /// Amount of gas used for the transaction.
    pub gas: Option<U256>,
    /// Gas price for the transaction.
    pub gas_price: Option<U256>,
    /// Value sent with this transaction.
    pub value: Option<U256>,
    /// Data of the transaction.
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

/// Benchmark callopts serialization.
fn callopts(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization");
    serialize_callopts(&mut group);
    group.finish();
}

/// Benchmark callopts serialization.
pub fn serialize_callopts(c: &mut Criterion) {
    c.bench_function("serialize_callopts", |b| {
        // Construct new callopts.
        let callopts = CallOpts {
            from: Some(Address::from_low_u64_be(0x1234)),
            to: Address::from_low_u64_be(0x5678),
            gas: Some(U256::from(0x9abc)),
            gas_price: Some(U256::from(0xdef0)),
            value: Some(U256::from(0x1234_5678)),
            data: Some(vec![0x12, 0x34, 0x56, 0x78]),
        };

        let config = config::standard();
        b.iter(|| {
            let encoded: Vec<u8> = bincode::encode_to_vec(&callopts, config).unwrap();
            let (decoded, len): (CallOpts, usize) = bincode::decode_from_slice(&encoded[..], config).unwrap();
            assert_eq!(callopts, decoded);
        })
    });
}



