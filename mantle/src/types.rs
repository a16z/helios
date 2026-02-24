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
    pub withdrawals_root: B256,
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

impl From<Withdrawal> for alloy::eips::eip4895::Withdrawal {
    fn from(value: Withdrawal) -> Self {
        alloy::eips::eip4895::Withdrawal {
            index: value.index,
            validator_index: value.validator_index,
            address: value.address,
            amount: value.amount,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{address, b256};
    use ssz::{Decode, Encode};

    fn make_test_withdrawal() -> Withdrawal {
        Withdrawal {
            index: 42,
            validator_index: 100,
            address: address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            amount: 1_000_000,
        }
    }

    #[test]
    fn test_withdrawal_into_alloy() {
        let w = make_test_withdrawal();
        let alloy_w: alloy::eips::eip4895::Withdrawal = w.into();
        assert_eq!(alloy_w.index, 42);
        assert_eq!(alloy_w.validator_index, 100);
        assert_eq!(
            alloy_w.address,
            address!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert_eq!(alloy_w.amount, 1_000_000);
    }

    #[test]
    fn test_withdrawal_ssz_roundtrip() {
        let w = make_test_withdrawal();
        let encoded = w.as_ssz_bytes();
        let decoded = Withdrawal::from_ssz_bytes(&encoded).unwrap();
        assert_eq!(decoded.index, 42);
        assert_eq!(decoded.validator_index, 100);
        assert_eq!(decoded.amount, 1_000_000);
    }

    fn make_test_payload() -> ExecutionPayload {
        let logs_bloom = FixedVector::from(vec![0u8; 256]);
        let extra_data = VariableList::from(vec![1u8, 2, 3]);

        ExecutionPayload {
            parent_hash: b256!("0000000000000000000000000000000000000000000000000000000000000001"),
            fee_recipient: address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            state_root: b256!("0000000000000000000000000000000000000000000000000000000000000002"),
            receipts_root: b256!("0000000000000000000000000000000000000000000000000000000000000003"),
            logs_bloom,
            prev_randao: B256::ZERO,
            block_number: 12345,
            gas_limit: 30_000_000,
            gas_used: 21_000,
            timestamp: 1_700_000_000,
            extra_data,
            base_fee_per_gas: U256::from(1_000_000_000u64),
            block_hash: b256!("0000000000000000000000000000000000000000000000000000000000000099"),
            transactions: VariableList::from(vec![]),
            withdrawals: VariableList::from(vec![]),
            blob_gas_used: 0,
            excess_blob_gas: 0,
            withdrawals_root: B256::ZERO,
        }
    }

    #[test]
    fn test_execution_payload_ssz_roundtrip() {
        let payload = make_test_payload();
        let encoded = payload.as_ssz_bytes();
        assert!(!encoded.is_empty());
        let decoded = ExecutionPayload::from_ssz_bytes(&encoded).unwrap();
        assert_eq!(decoded.block_number, 12345);
        assert_eq!(decoded.gas_limit, 30_000_000);
        assert_eq!(decoded.gas_used, 21_000);
        assert_eq!(decoded.timestamp, 1_700_000_000);
        assert_eq!(decoded.parent_hash, payload.parent_hash);
        assert_eq!(decoded.fee_recipient, payload.fee_recipient);
        assert_eq!(decoded.state_root, payload.state_root);
        assert_eq!(decoded.receipts_root, payload.receipts_root);
        assert_eq!(decoded.block_hash, payload.block_hash);
        assert_eq!(decoded.base_fee_per_gas, U256::from(1_000_000_000u64));
        assert_eq!(decoded.blob_gas_used, 0);
        assert_eq!(decoded.excess_blob_gas, 0);
    }

    #[test]
    fn test_execution_payload_fields() {
        let payload = make_test_payload();
        assert_eq!(payload.block_number, 12345);
        assert_eq!(
            payload.fee_recipient,
            address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")
        );
        assert_eq!(payload.gas_limit, 30_000_000);
        assert_eq!(payload.transactions.len(), 0);
        assert_eq!(payload.withdrawals.len(), 0);
    }

    #[test]
    fn test_execution_payload_with_withdrawals_roundtrip() {
        let mut payload = make_test_payload();
        let w = make_test_withdrawal();
        payload.withdrawals = VariableList::from(vec![w]);

        let encoded = payload.as_ssz_bytes();
        let decoded = ExecutionPayload::from_ssz_bytes(&encoded).unwrap();
        assert_eq!(decoded.withdrawals.len(), 1);
        assert_eq!(decoded.withdrawals[0].index, 42);
    }

    #[test]
    fn test_execution_payload_decode_invalid_bytes() {
        let result = ExecutionPayload::from_ssz_bytes(&[0u8; 10]);
        assert!(result.is_err());
    }
}
