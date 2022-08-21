use ethers::abi::AbiEncode;
use ethers::prelude::{Address, U256};
use eyre::Result;
use jsonrpsee::{
    core::client::ClientT,
    http_client::{HttpClient, HttpClientBuilder},
    rpc_params,
};

use crate::common::utils::{address_to_hex_string, hex_str_to_bytes, u64_to_hex_string};

use super::types::Proof;

pub struct Rpc {
    rpc: String,
}

impl Rpc {
    pub fn new(rpc: &str) -> Self {
        Rpc {
            rpc: rpc.to_string(),
        }
    }

    pub async fn get_proof(&self, address: &Address, slots: &[U256], block: u64) -> Result<Proof> {
        let client = self.client()?;
        let block_hex = u64_to_hex_string(block);
        let addr_hex = address_to_hex_string(address);
        let slots = slots
            .iter()
            .map(|slot| slot.encode_hex())
            .collect::<Vec<String>>();
        let params = rpc_params!(addr_hex, slots.as_slice(), block_hex);
        Ok(client.request("eth_getProof", params).await?)
    }

    pub async fn get_code(&self, address: &Address, block: u64) -> Result<Vec<u8>> {
        let client = self.client()?;
        let block_hex = u64_to_hex_string(block);
        let addr_hex = address_to_hex_string(address);
        let params = rpc_params!(addr_hex, block_hex);
        let code: String = client.request("eth_getCode", params).await?;
        hex_str_to_bytes(&code)
    }

    fn client(&self) -> Result<HttpClient> {
        Ok(HttpClientBuilder::default().build(&self.rpc)?)
    }
}
