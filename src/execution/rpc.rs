use ethers::prelude::Address;
use eyre::Result;
use jsonrpsee::{core::client::ClientT, http_client::HttpClientBuilder, rpc_params};

use super::types::Proof;

pub struct Rpc {
    rpc: String,
}

impl Rpc {
    pub fn new(rpc: &str) -> Self {
        Rpc { rpc: rpc.to_string() }
    }

    pub async fn get_proof(&self, address: &Address, block: u64) -> Result<Proof> {
        let client = HttpClientBuilder::default().build(&self.rpc)?;
        let block_hex = format!("0x{:x}", block);
        let addr_hex = format!("0x{}", hex::encode(address.as_bytes()));
        let params = rpc_params!(addr_hex, [""], block_hex);
        Ok(client.request("eth_getProof", params).await?)
    }
}

