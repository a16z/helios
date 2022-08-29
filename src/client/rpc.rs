use ethers::{
    abi::AbiEncode,
    types::{Address, U256},
};
use eyre::Result;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, net::SocketAddr, str::FromStr, sync::Arc};

use jsonrpsee::{
    core::{async_trait, Error},
    http_server::{HttpServerBuilder, HttpServerHandle},
    proc_macros::rpc,
};

use crate::common::utils::{hex_str_to_bytes, u64_to_hex_string};

use super::Client;

pub struct Rpc {
    client: Arc<Client>,
    handle: Option<HttpServerHandle>,
}

impl Rpc {
    pub fn new(client: Arc<Client>) -> Self {
        Rpc {
            client,
            handle: None,
        }
    }

    pub async fn start(&mut self) -> Result<SocketAddr> {
        let rpc_inner = RpcInner {
            client: self.client.clone(),
        };
        let (handle, addr) = start(rpc_inner).await?;
        self.handle = Some(handle);
        Ok(addr)
    }
}

#[rpc(client, server, namespace = "eth")]
trait EthRpc {
    #[method(name = "getBalance")]
    async fn get_balance(&self, address: &str, block: &str) -> Result<String, Error>;
    #[method(name = "getTransactionCount")]
    async fn get_transaction_count(&self, address: &str, block: &str) -> Result<String, Error>;
    #[method(name = "getCode")]
    async fn get_code(&self, address: &str, block: &str) -> Result<String, Error>;
    #[method(name = "call")]
    async fn call(&self, opts: CallOpts, block: &str) -> Result<String, Error>;
    #[method(name = "estimateGas")]
    async fn estimate_gas(&self, opts: CallOpts) -> Result<String, Error>;
    #[method(name = "chainId")]
    fn chain_id(&self) -> Result<String, Error>;
    #[method(name = "gasPrice")]
    async fn gas_price(&self) -> Result<String, Error>;
    #[method(name = "maxPriorityFeePerGas")]
    async fn max_priority_fee_per_gas(&self) -> Result<String, Error>;
}

struct RpcInner {
    pub client: Arc<Client>,
}

#[async_trait]
impl EthRpcServer for RpcInner {
    async fn get_balance(&self, address: &str, block: &str) -> Result<String, Error> {
        match block {
            "latest" => {
                let address = convert_err(Address::from_str(address))?;
                let balance = convert_err(self.client.get_balance(&address).await)?;

                Ok(balance.encode_hex())
            }
            _ => Err(Error::Custom("Invalid Block Number".to_string())),
        }
    }

    async fn get_transaction_count(&self, address: &str, block: &str) -> Result<String, Error> {
        match block {
            "latest" => {
                let address = convert_err(Address::from_str(address))?;
                let nonce = convert_err(self.client.get_nonce(&address).await)?;

                Ok(nonce.encode_hex())
            }
            _ => Err(Error::Custom("Invalid Block Number".to_string())),
        }
    }

    async fn get_code(&self, address: &str, block: &str) -> Result<String, Error> {
        match block {
            "latest" => {
                let address = convert_err(Address::from_str(address))?;
                let code = convert_err(self.client.get_code(&address).await)?;

                Ok(hex::encode(code))
            }
            _ => Err(Error::Custom("Invalid Block Number".to_string())),
        }
    }

    async fn call(&self, opts: CallOpts, block: &str) -> Result<String, Error> {
        match block {
            "latest" => {
                let to = convert_err(Address::from_str(&opts.to))?;
                let data = convert_err(hex_str_to_bytes(&opts.data.unwrap_or("0x".to_string())))?;
                let value = convert_err(U256::from_str_radix(
                    &opts.value.unwrap_or("0x0".to_string()),
                    16,
                ))?;

                let res = convert_err(self.client.call(&to, &data, value).await)?;
                Ok(hex::encode(res))
            }
            _ => Err(Error::Custom("Invalid Block Number".to_string())),
        }
    }

    async fn estimate_gas(&self, opts: CallOpts) -> Result<String, Error> {
        let to = convert_err(Address::from_str(&opts.to))?;
        let data = convert_err(hex_str_to_bytes(&opts.data.unwrap_or("0x".to_string())))?;
        let value = convert_err(U256::from_str_radix(
            &opts.value.unwrap_or("0x0".to_string()),
            16,
        ))?;

        let gas = convert_err(self.client.estimate_gas(&to, &data, value).await)?;
        Ok(u64_to_hex_string(gas))
    }

    fn chain_id(&self) -> Result<String, Error> {
        let id = self.client.chain_id();
        Ok(u64_to_hex_string(id))
    }

    async fn gas_price(&self) -> Result<String, Error> {
        let gas_price = convert_err(self.client.get_gas_price().await)?;
        Ok(gas_price.encode_hex())
    }

    async fn max_priority_fee_per_gas(&self) -> Result<String, Error> {
        let tip = convert_err(self.client.get_priority_fee().await)?;
        Ok(tip.encode_hex())
    }
}

async fn start(rpc: RpcInner) -> Result<(HttpServerHandle, SocketAddr)> {
    let server = HttpServerBuilder::default().build("127.0.0.1:8545").await?;

    let addr = server.local_addr()?;
    let handle = server.start(rpc.into_rpc())?;

    Ok((handle, addr))
}

fn convert_err<T, E: Display>(res: Result<T, E>) -> Result<T, Error> {
    res.map_err(|err| Error::Custom(err.to_string()))
}

#[derive(Deserialize, Serialize)]
pub struct CallOpts {
    from: Option<String>,
    to: String,
    gas: Option<String>,
    value: Option<String>,
    data: Option<String>,
}
