use alloy::{
    primitives::{keccak256, Address, Bytes, B256},
    signers::Signature,
};
use eyre::Result;
use helios_core::client::HeliosClient;
use serde::{Deserialize, Serialize};
use spec::OpStack;
use ssz::Decode;

use types::ExecutionPayload;

mod builder;
pub mod config;
pub mod consensus;
pub(crate) mod evm;
#[cfg(not(target_arch = "wasm32"))]
pub mod server;
pub mod spec;
pub mod types;

pub use builder::OpStackClientBuilder;
pub type OpStackClient = HeliosClient<OpStack>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequencerCommitment {
    data: Bytes,
    signature: Signature,
}

impl SequencerCommitment {
    pub fn new(data: &[u8]) -> Result<Self> {
        let mut decoder = snap::raw::Decoder::new();
        let decompressed = decoder.decompress_vec(data)?;
        if decompressed.len() < 65 {
            eyre::bail!("sequencer commitment shorter than a signature");
        }

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
mod tests {
    use super::*;

    fn compress(data: &[u8]) -> Vec<u8> {
        snap::raw::Encoder::new().compress_vec(data).unwrap()
    }

    #[test]
    fn rejects_messages_shorter_than_a_signature() {
        for len in [0, 10, 64] {
            assert!(SequencerCommitment::new(&compress(&vec![0u8; len])).is_err());
        }
    }

    #[test]
    fn splits_signature_and_data() {
        let mut msg = vec![0u8; 65];
        msg[0] = 1;
        msg[32] = 1;
        msg.extend_from_slice(&[0xaa, 0xbb]);

        let commitment = SequencerCommitment::new(&compress(&msg)).unwrap();

        assert_eq!(commitment.data, Bytes::from(vec![0xaa, 0xbb]));
        assert_eq!(commitment.signature.as_bytes()[..64], msg[..64]);
    }
}
