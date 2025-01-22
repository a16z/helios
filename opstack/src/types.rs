use alloy::primitives::{Address, B256, U256};
use alloy_rlp::RlpEncodable;
use ssz_derive::{Decode, Encode};
use ssz_types::{FixedVector, VariableList};

#[derive(Debug, Clone, Encode, Decode)]
pub struct ExecutionPayload {
    pub parent_hash: B256,
    pub fee_recipient: Address,
    pub state_root: B256,
    pub receipts_root: B256,
    pub logs_bloom: LogsBloom,
    pub prev_randao: B256,
    pub block_number: u64,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub timestamp: u64,
    pub extra_data: ExtraData,
    pub base_fee_per_gas: U256,
    pub block_hash: B256,
    pub transactions: VariableList<Transaction, typenum::U1048576>,
    pub withdrawals: VariableList<Withdrawal, typenum::U16>,
    pub blob_gas_used: u64,
    pub excess_blob_gas: u64,
}

pub type Transaction = VariableList<u8, typenum::U1073741824>;
pub type LogsBloom = FixedVector<u8, typenum::U256>;
pub type ExtraData = VariableList<u8, typenum::U32>;

#[derive(Clone, Debug, Encode, Decode, RlpEncodable)]
pub struct Withdrawal {
    index: u64,
    validator_index: u64,
    address: Address,
    amount: u64,
}

impl Into<alloy::eips::eip4895::Withdrawal> for Withdrawal {
    fn into(self) -> alloy::eips::eip4895::Withdrawal {
        alloy::eips::eip4895::Withdrawal {
            index: self.index,
            validator_index: self.validator_index,
            address: self.address,
            amount: self.amount,
        }
    }
}
