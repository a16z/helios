use alloy::{
    primitives::{keccak256, Address, Bytes, B256},
    signers::Signature,
};
use eyre::Result;
use serde::{Deserialize, Serialize};
use spec::OpStack;
use ssz::Decode;

use helios_core::client::Client;

use consensus::ConsensusClient;
use types::ExecutionPayload;

mod builder;
pub mod config;
pub mod consensus;
#[cfg(not(target_arch = "wasm32"))]
pub mod server;
pub mod spec;
pub mod types;

pub use builder::OpStackClientBuilder;
pub type OpStackClient = Client<OpStack, ConsensusClient>;

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
