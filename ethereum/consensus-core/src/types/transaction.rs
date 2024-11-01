use alloy::primitives::{Address, B256, U256};
use serde::{Deserialize, Serialize};
use ssz_derive::{Decode, Encode};
use ssz_types::{BitVector, VariableList};
use tree_hash_derive::TreeHash;
use typenum::Unsigned;

use super::{
    bytes::{ByteList, ByteVector},
    serde_utils,
};

#[derive(Clone, PartialEq, Debug, Encode, Decode, TreeHash, Serialize, Deserialize)]
pub struct Transaction {
    pub payload: TransactionPayload,
    pub signature: ExecutionSignature,
}

#[derive(Clone, PartialEq, Debug, Encode, Decode, TreeHash, Serialize, Deserialize, Default)]
#[ssz(struct_behaviour = "stable_container")]
#[ssz(max_fields = "typenum::U32")]
#[tree_hash(struct_behaviour = "stable_container")]
#[tree_hash(max_fields = "typenum::U32")]
#[serde(default)]
pub struct TransactionPayload {
    // EIP-2718
    #[serde(rename = "type")]
    #[serde(with = "serde_utils::u8_opt")]
    pub tx_type: Option<u8>,

    // EIP-155
    #[serde(with = "serde_utils::u64_opt")]
    pub chain_id: Option<u64>,

    #[serde(with = "serde_utils::u64_opt")]
    pub nonce: Option<u64>,
    pub max_fees_per_gas: Option<FeesPerGas>,
    #[serde(with = "serde_utils::u64_opt")]
    pub gas: Option<u64>,
    pub to: Option<Address>,
    #[serde(with = "serde_utils::u256_opt")]
    pub value: Option<U256>,
    pub input: Option<ByteList<typenum::U16777216>>,

    // EIP-2930
    pub access_list: Option<VariableList<AccessTuple, typenum::U524288>>,

    // EIP-1559
    pub max_priority_fees_per_gas: Option<FeesPerGas>,

    // EIP-4844
    pub blob_versioned_hashes: Option<VariableList<B256, typenum::U4096>>,

    // EIP-7702
    pub authorization_list: Option<VariableList<Authorization, typenum::U65536>>,
}

#[derive(Clone, PartialEq, Debug, Encode, Decode, TreeHash, Serialize, Deserialize, Default)]
#[ssz(struct_behaviour = "stable_container")]
#[ssz(max_fields = "typenum::U16")]
#[tree_hash(struct_behaviour = "stable_container")]
#[tree_hash(max_fields = "typenum::U16")]
#[serde(default)]
pub struct FeesPerGas {
    #[serde(with = "serde_utils::u256_opt")]
    pub regular: Option<U256>,
    #[serde(with = "serde_utils::u256_opt")]
    pub blob: Option<U256>,
}

impl FeesPerGas {
    pub fn amount(&self) -> U256 {
        if let Some(amount) = self.regular {
            return amount;
        }

        if let Some(amount) = self.blob {
            return amount;
        }

        U256::ZERO
    }
}

#[derive(Clone, PartialEq, Debug, Encode, Decode, TreeHash, Serialize, Deserialize, Default)]
pub struct AccessTuple {
    pub address: Address,
    pub storage_keys: VariableList<B256, typenum::U524288>,
}

#[derive(Clone, PartialEq, Debug, Encode, Decode, TreeHash, Serialize, Deserialize, Default)]
pub struct Authorization {
    payload: AuthorizationPayload,
    signature: ExecutionSignature,
}

#[derive(Clone, PartialEq, Debug, Encode, Decode, TreeHash, Serialize, Deserialize, Default)]
#[ssz(struct_behaviour = "stable_container")]
#[ssz(max_fields = "typenum::U16")]
#[tree_hash(struct_behaviour = "stable_container")]
#[tree_hash(max_fields = "typenum::U16")]
pub struct AuthorizationPayload {
    #[serde(with = "serde_utils::u8_opt")]
    magic: Option<u8>,
    #[serde(with = "serde_utils::u64_opt")]
    chain_id: Option<u64>,
    address: Option<Address>,
    #[serde(with = "serde_utils::u64_opt")]
    nonce: Option<u64>,
}

#[derive(Clone, PartialEq, Debug, Encode, Decode, TreeHash, Serialize, Deserialize, Default)]
#[ssz(struct_behaviour = "stable_container")]
#[ssz(max_fields = "typenum::U8")]
#[tree_hash(struct_behaviour = "stable_container")]
#[tree_hash(max_fields = "typenum::U8")]
pub struct ExecutionSignature {
    pub secp256k1: Option<ByteVector<typenum::U65>>,
}

#[test]
fn test_deserialize() {
    use tree_hash::TreeHash;

    let data = "{\"payload\":{\"type\":\"2\",\"chain_id\":\"7061395750\",\"nonce\":\"0\",\"max_fees_per_gas\":{\"regular\":\"100000000000\"},\"gas\":\"100000\",\"to\":\"0x6eef24367483af2fdc5026d985e2827f19a7a140\",\"value\":\"10000000000000000000\",\"input\":\"0x\",\"access_list\":[],\"max_priority_fees_per_gas\":{\"regular\":\"2000000000\"}},\"signature\":{\"secp256k1\":\"0x6adf0ca6dcbed482a2b1e537a0ffa5a421f8fb1a0106d3fbffda6bf1f5b378013bbf182d5e8c70d91461153ad5c8f4de7f29c9335fa43dccdb022dfb7aefff7801\"}}";

    let tx: Transaction = serde_json::from_str(data).unwrap();
    let hash = tx.tree_hash_root();
    println!("{:#?}", hash);
}
