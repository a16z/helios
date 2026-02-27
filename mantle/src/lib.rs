use alloy::{
    primitives::{keccak256, Address, Bytes, B256},
    signers::Signature,
};
use eyre::Result;
use helios_core::client::HeliosClient;
use serde::{Deserialize, Serialize};
use spec::Mantle;
use ssz::Decode;

use types::ExecutionPayload;

mod builder;
pub mod config;
pub mod consensus;
pub(crate) mod evm;
pub mod spec;
pub mod types;

pub use builder::MantleClientBuilder;
pub type MantleClient = HeliosClient<Mantle>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequencerCommitment {
    data: Bytes,
    signature: Signature,
}

impl SequencerCommitment {
    pub fn new(data: &[u8]) -> Result<Self> {
        let mut decoder = snap::raw::Decoder::new();
        let decompressed = decoder.decompress_vec(data)?;

        let signature = Signature::try_from(&decompressed[..65])?;
        let data = Bytes::from(decompressed[65..].to_vec());

        Ok(SequencerCommitment { data, signature })
    }

    pub fn verify(&self, signer: Address, chain_id: u64) -> Result<()> {
        let msg = signature_msg(&self.data, chain_id);
        let pk = self.signature.recover_from_prehash(&msg)?;
        let recovered_signer = Address::from_public_key(&pk);

        if signer != recovered_signer {
            eyre::bail!("invalid signer");
        }

        Ok(())
    }
}

impl TryFrom<&SequencerCommitment> for ExecutionPayload {
    type Error = eyre::Report;

    fn try_from(value: &SequencerCommitment) -> Result<Self> {
        let payload_bytes = &value.data[32..];
        ExecutionPayload::from_ssz_bytes(payload_bytes).map_err(|_| eyre::eyre!("decode failed"))
    }
}

fn signature_msg(data: &[u8], chain_id: u64) -> B256 {
    let domain = B256::ZERO;
    let chain_id = B256::left_padding_from(&chain_id.to_be_bytes());
    let payload_hash = keccak256(data);

    let signing_data = [
        domain.as_slice(),
        chain_id.as_slice(),
        payload_hash.as_slice(),
    ];

    keccak256(signing_data.concat())
}

#[cfg(test)]
pub(crate) mod test_helpers {
    use super::*;
    use alloy::primitives::{address, U256};
    use alloy::signers::k256::ecdsa::SigningKey;
    use alloy::signers::local::LocalSigner;
    use alloy::signers::SignerSync;
    use ssz::Encode;
    use ssz_types::{FixedVector, VariableList};
    use types::ExtraData;

    pub fn test_withdrawal(
        index: u64,
        validator_index: u64,
        addr: Address,
        amount: u64,
    ) -> crate::types::Withdrawal {
        use ssz::Decode;
        // Withdrawal SSZ layout: index(u64) + validator_index(u64) + address(20) + amount(u64)
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&index.to_le_bytes());
        bytes.extend_from_slice(&validator_index.to_le_bytes());
        bytes.extend_from_slice(addr.as_slice());
        bytes.extend_from_slice(&amount.to_le_bytes());
        crate::types::Withdrawal::from_ssz_bytes(&bytes).unwrap()
    }

    pub fn test_signing_key() -> LocalSigner<SigningKey> {
        let key_bytes = [1u8; 32];
        LocalSigner::from_slice(&key_bytes).unwrap()
    }

    pub fn test_payload() -> ExecutionPayload {
        let logs_bloom = FixedVector::from(vec![0u8; 256]);
        let extra_data: ExtraData = VariableList::from(vec![0u8; 4]);

        ExecutionPayload {
            parent_hash: B256::ZERO,
            fee_recipient: address!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
            state_root: B256::ZERO,
            receipts_root: B256::ZERO,
            logs_bloom,
            prev_randao: B256::ZERO,
            block_number: 100,
            gas_limit: 30_000_000,
            gas_used: 21_000,
            timestamp: 1_700_000_000,
            extra_data,
            base_fee_per_gas: U256::from(1_000_000_000u64),
            block_hash: B256::ZERO,
            transactions: VariableList::from(vec![]),
            withdrawals: VariableList::from(vec![]),
            blob_gas_used: 0,
            excess_blob_gas: 0,
            withdrawals_root: B256::ZERO,
        }
    }

    pub fn create_signed_commitment(
        payload: &ExecutionPayload,
        signer: &LocalSigner<SigningKey>,
        chain_id: u64,
    ) -> Vec<u8> {
        let payload_bytes = payload.as_ssz_bytes();
        // data = 32-byte hash prefix + SSZ-encoded payload
        let payload_hash = keccak256(&payload_bytes);
        let mut data = Vec::with_capacity(32 + payload_bytes.len());
        data.extend_from_slice(payload_hash.as_slice());
        data.extend_from_slice(&payload_bytes);

        let msg = signature_msg(&data, chain_id);
        let sig = signer.sign_hash_sync(&msg).unwrap();

        // Format: signature (65 bytes) + data
        let mut uncompressed = Vec::new();
        let sig_bytes: [u8; 65] = {
            let mut buf = [0u8; 65];
            buf[..32].copy_from_slice(&sig.r().to_be_bytes::<32>());
            buf[32..64].copy_from_slice(&sig.s().to_be_bytes::<32>());
            buf[64] = sig.v() as u8;
            buf
        };
        uncompressed.extend_from_slice(&sig_bytes);
        uncompressed.extend_from_slice(&data);

        // Snap-compress
        let mut encoder = snap::raw::Encoder::new();
        encoder.compress_vec(&uncompressed).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_helpers::*;

    #[test]
    fn test_signature_msg_deterministic() {
        let data = b"hello world";
        let chain_id = 5000u64;
        let msg1 = signature_msg(data, chain_id);
        let msg2 = signature_msg(data, chain_id);
        assert_eq!(msg1, msg2);
    }

    #[test]
    fn test_signature_msg_different_chain_ids() {
        let data = b"hello world";
        let msg1 = signature_msg(data, 5000);
        let msg2 = signature_msg(data, 5003);
        assert_ne!(msg1, msg2);
    }

    #[test]
    fn test_signature_msg_different_data() {
        let msg1 = signature_msg(b"data_a", 5000);
        let msg2 = signature_msg(b"data_b", 5000);
        assert_ne!(msg1, msg2);
    }

    #[test]
    fn test_signature_msg_includes_domain_chainid_payload() {
        let data = b"test";
        let chain_id = 1u64;
        let msg = signature_msg(data, chain_id);
        // Verify it produces a 32-byte B256 hash
        assert_eq!(msg.len(), 32);
        // Verify it's not zero (the hash of the concatenation should not be zero)
        assert_ne!(msg, B256::ZERO);
    }

    #[test]
    fn test_sequencer_commitment_new_valid() {
        let signer = test_signing_key();
        let payload = test_payload();
        let compressed = create_signed_commitment(&payload, &signer, 5000);

        let commitment = SequencerCommitment::new(&compressed).unwrap();
        assert!(!commitment.data.is_empty());
    }

    #[test]
    fn test_sequencer_commitment_new_invalid_data() {
        // Not valid snap-compressed data
        let result = SequencerCommitment::new(&[0u8; 10]);
        assert!(result.is_err());
    }

    #[test]
    fn test_sequencer_commitment_new_empty() {
        let result = SequencerCommitment::new(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_sequencer_commitment_verify_valid() {
        let signer = test_signing_key();
        let signer_address = signer.address();
        let payload = test_payload();
        let compressed = create_signed_commitment(&payload, &signer, 5000);

        let commitment = SequencerCommitment::new(&compressed).unwrap();
        assert!(commitment.verify(signer_address, 5000).is_ok());
    }

    #[test]
    fn test_sequencer_commitment_verify_wrong_signer() {
        let signer = test_signing_key();
        let payload = test_payload();
        let compressed = create_signed_commitment(&payload, &signer, 5000);

        let commitment = SequencerCommitment::new(&compressed).unwrap();
        let wrong_address = Address::ZERO;
        assert!(commitment.verify(wrong_address, 5000).is_err());
    }

    #[test]
    fn test_sequencer_commitment_verify_wrong_chain_id() {
        let signer = test_signing_key();
        let signer_address = signer.address();
        let payload = test_payload();
        let compressed = create_signed_commitment(&payload, &signer, 5000);

        let commitment = SequencerCommitment::new(&compressed).unwrap();
        // Verify with wrong chain_id should fail (different message hash)
        assert!(commitment.verify(signer_address, 9999).is_err());
    }

    #[test]
    fn test_sequencer_commitment_to_payload() {
        let signer = test_signing_key();
        let payload = test_payload();
        let compressed = create_signed_commitment(&payload, &signer, 5000);

        let commitment = SequencerCommitment::new(&compressed).unwrap();
        let decoded_payload = ExecutionPayload::try_from(&commitment).unwrap();

        assert_eq!(decoded_payload.block_number, payload.block_number);
        assert_eq!(decoded_payload.gas_limit, payload.gas_limit);
        assert_eq!(decoded_payload.gas_used, payload.gas_used);
        assert_eq!(decoded_payload.timestamp, payload.timestamp);
        assert_eq!(decoded_payload.fee_recipient, payload.fee_recipient);
    }

    #[test]
    fn test_sequencer_commitment_roundtrip_full() {
        let signer = test_signing_key();
        let signer_address = signer.address();
        let payload = test_payload();
        let chain_id = 5000u64;

        let compressed = create_signed_commitment(&payload, &signer, chain_id);
        let commitment = SequencerCommitment::new(&compressed).unwrap();

        // Verify signature
        assert!(commitment.verify(signer_address, chain_id).is_ok());

        // Extract payload
        let decoded = ExecutionPayload::try_from(&commitment).unwrap();
        assert_eq!(decoded.block_number, 100);
        assert_eq!(decoded.timestamp, 1_700_000_000);
    }

    #[test]
    fn test_sequencer_commitment_serialization() {
        let signer = test_signing_key();
        let payload = test_payload();
        let compressed = create_signed_commitment(&payload, &signer, 5000);
        let commitment = SequencerCommitment::new(&compressed).unwrap();

        // Test JSON serialization roundtrip
        let json = serde_json::to_string(&commitment).unwrap();
        let deserialized: SequencerCommitment = serde_json::from_str(&json).unwrap();
        assert_eq!(commitment.data, deserialized.data);
    }
}
