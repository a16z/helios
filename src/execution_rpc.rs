use ethers::prelude::{Address, H256, U256};
use eyre::Result;
use jsonrpsee::{core::client::ClientT, http_client::HttpClientBuilder, rpc_params};
use serde::de::Error;
use serde::Deserialize;

use crate::utils::hex_str_to_bytes;

pub struct ExecutionRpc {
    rpc: String,
}

impl ExecutionRpc {
    pub fn new(rpc: &str) -> Self {
        ExecutionRpc {
            rpc: rpc.to_string(),
        }
    }

    pub async fn get_proof(&self, address: &Address, block: u64) -> Result<Proof> {
        let client = HttpClientBuilder::default().build(&self.rpc)?;
        let block_hex = format!("0x{:x}", block);
        let addr_hex = format!("0x{}", hex::encode(address.as_bytes()));
        let params = rpc_params!(addr_hex, [""], block_hex);
        Ok(client.request("eth_getProof", params).await?)
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Proof {
    pub address: Address,
    pub balance: U256,
    pub code_hash: H256,
    pub nonce: U256,
    pub storage_hash: H256,
    #[serde(deserialize_with = "proof_deserialize")]
    pub account_proof: Vec<Vec<u8>>,
}

fn proof_deserialize<'de, D>(deserializer: D) -> Result<Vec<Vec<u8>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let branch: Vec<String> = serde::Deserialize::deserialize(deserializer)?;
    Ok(branch
        .iter()
        .map(|elem| hex_str_to_bytes(elem))
        .collect::<Result<_>>()
        .map_err(D::Error::custom)?)
}
