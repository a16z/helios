use ethers::{
    abi::AbiEncode,
    types::{Address, Transaction, TransactionReceipt, H256},
};
use eyre::Result;
use log::{debug, info, warn};
use std::{fmt::Display, net::SocketAddr, str::FromStr, sync::Arc};
use tokio::sync::Mutex;

use jsonrpsee::{
    core::{async_trait, server::rpc_module::Methods, Error},
    http_server::{HttpServerBuilder, HttpServerHandle},
    proc_macros::rpc,
};

use crate::node::{BlockTag, Node};

use common::utils::{hex_str_to_bytes, u64_to_hex_string};
use execution::types::{CallOpts, ExecutionBlock};

pub struct Rpc {
    node: Arc<Mutex<Node>>,
    handle: Option<HttpServerHandle>,
    port: u16,
}

impl Rpc {
    pub fn new(node: Arc<Mutex<Node>>, port: u16) -> Self {
        Rpc {
            node,
            handle: None,
            port,
        }
    }

    pub async fn start(&mut self) -> Result<SocketAddr> {
        let rpc_inner = RpcInner {
            node: self.node.clone(),
            port: self.port,
        };
        let (handle, addr) = start(rpc_inner).await?;
        self.handle = Some(handle);

        info!("rpc server strated at {}", addr);

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
    async fn chain_id(&self) -> Result<String, Error>;
    #[method(name = "gasPrice")]
    async fn gas_price(&self) -> Result<String, Error>;
    #[method(name = "maxPriorityFeePerGas")]
    async fn max_priority_fee_per_gas(&self) -> Result<String, Error>;
    #[method(name = "blockNumber")]
    async fn block_number(&self) -> Result<String, Error>;
    #[method(name = "getBlockByNumber")]
    async fn get_block_by_number(&self, num: &str, full_tx: bool) -> Result<ExecutionBlock, Error>;
    #[method(name = "getBlockByHash")]
    async fn get_block_by_hash(&self, hash: &str, full_tx: bool) -> Result<ExecutionBlock, Error>;
    #[method(name = "sendRawTransaction")]
    async fn send_raw_transaction(&self, bytes: &str) -> Result<String, Error>;
    #[method(name = "getTransactionReceipt")]
    async fn get_transaction_receipt(&self, hash: &str) -> Result<TransactionReceipt, Error>;
    #[method(name = "getTransactionByHash")]
    async fn get_transaction_by_hash(&self, hash: &str) -> Result<Transaction, Error>;
}

#[rpc(client, server, namespace = "net")]
trait NetRpc {
    #[method(name = "version")]
    async fn version(&self) -> Result<String, Error>;
}

#[derive(Clone)]
struct RpcInner {
    node: Arc<Mutex<Node>>,
    port: u16,
}

#[async_trait]
impl EthRpcServer for RpcInner {
    async fn get_balance(&self, address: &str, block: &str) -> Result<String, Error> {
        debug!("eth_getBalance");
        let block = convert_err(decode_block(block))?;
        let address = convert_err(Address::from_str(address))?;
        let node = self.node.lock().await;
        let balance = convert_err(node.get_balance(&address, &block).await)?;

        Ok(balance.encode_hex())
    }

    async fn get_transaction_count(&self, address: &str, block: &str) -> Result<String, Error> {
        let block = convert_err(decode_block(block))?;
        let address = convert_err(Address::from_str(address))?;
        let node = self.node.lock().await;
        let nonce = convert_err(node.get_nonce(&address, &block).await)?;

        Ok(nonce.encode_hex())
    }

    async fn get_code(&self, address: &str, block: &str) -> Result<String, Error> {
        let block = convert_err(decode_block(block))?;
        let address = convert_err(Address::from_str(address))?;
        let node = self.node.lock().await;
        let code = convert_err(node.get_code(&address, &block).await)?;

        Ok(hex::encode(code))
    }

    async fn call(&self, opts: CallOpts, block: &str) -> Result<String, Error> {
        debug!("eth_call");
        let block = convert_err(decode_block(block))?;
        let node = self.node.lock().await;
        let res = convert_err(node.call(&opts, &block))?;

        Ok(format!("0x{}", hex::encode(res)))
    }

    async fn estimate_gas(&self, opts: CallOpts) -> Result<String, Error> {
        debug!("eth_estimateGas");
        let node = self.node.lock().await;
        let gas = convert_err(node.estimate_gas(&opts))?;

        Ok(u64_to_hex_string(gas))
    }

    async fn chain_id(&self) -> Result<String, Error> {
        let node = self.node.lock().await;
        let id = node.chain_id();
        Ok(u64_to_hex_string(id))
    }

    async fn gas_price(&self) -> Result<String, Error> {
        let node = self.node.lock().await;
        let gas_price = convert_err(node.get_gas_price())?;
        Ok(gas_price.encode_hex())
    }

    async fn max_priority_fee_per_gas(&self) -> Result<String, Error> {
        let node = self.node.lock().await;
        let tip = convert_err(node.get_priority_fee())?;
        Ok(tip.encode_hex())
    }

    async fn block_number(&self) -> Result<String, Error> {
        let node = self.node.lock().await;
        let num = convert_err(node.get_block_number())?;
        Ok(u64_to_hex_string(num))
    }

    async fn get_block_by_number(
        &self,
        block: &str,
        _full_tx: bool,
    ) -> Result<ExecutionBlock, Error> {
        let block = convert_err(decode_block(block))?;
        let node = self.node.lock().await;
        let block = convert_err(node.get_block_by_number(&block))?;

        Ok(block)
    }

    async fn get_block_by_hash(&self, hash: &str, _full_tx: bool) -> Result<ExecutionBlock, Error> {
        let hash = convert_err(hex_str_to_bytes(hash))?;
        let node = self.node.lock().await;
        let block = convert_err(node.get_block_by_hash(&hash))?;

        Ok(block)
    }

    async fn send_raw_transaction(&self, bytes: &str) -> Result<String, Error> {
        let node = self.node.lock().await;
        let bytes = convert_err(hex_str_to_bytes(bytes))?;
        let tx_hash = convert_err(node.send_raw_transaction(&bytes).await)?;
        Ok(hex::encode(tx_hash))
    }

    async fn get_transaction_receipt(&self, hash: &str) -> Result<TransactionReceipt, Error> {
        let node = self.node.lock().await;
        let hash = H256::from_slice(&convert_err(hex_str_to_bytes(hash))?);
        let receipt = convert_err(node.get_transaction_receipt(&hash).await)?;

        match receipt {
            Some(receipt) => Ok(receipt),
            None => Err(Error::Custom("Receipt Not Found".to_string())),
        }
    }

    async fn get_transaction_by_hash(&self, hash: &str) -> Result<Transaction, Error> {
        let node = self.node.lock().await;
        let hash = H256::from_slice(&convert_err(hex_str_to_bytes(hash))?);
        let tx = convert_err(node.get_transaction_by_hash(&hash).await)?;

        match tx {
            Some(tx) => Ok(tx),
            None => Err(Error::Custom("Transaction Not Found".to_string())),
        }
    }
}

#[async_trait]
impl NetRpcServer for RpcInner {
    async fn version(&self) -> Result<String, Error> {
        let node = self.node.lock().await;
        Ok(node.chain_id().to_string())
    }
}

async fn start(rpc: RpcInner) -> Result<(HttpServerHandle, SocketAddr)> {
    let addr = format!("127.0.0.1:{}", rpc.port);
    let server = HttpServerBuilder::default().build(addr).await?;

    let addr = server.local_addr()?;

    let mut methods = Methods::new();
    let eth_methods: Methods = EthRpcServer::into_rpc(rpc.clone()).into();
    let net_methods: Methods = NetRpcServer::into_rpc(rpc).into();

    methods.merge(eth_methods)?;
    methods.merge(net_methods)?;

    let handle = server.start(methods)?;

    Ok((handle, addr))
}

fn convert_err<T, E: Display>(res: Result<T, E>) -> Result<T, Error> {
    res.map_err(|err| {
        warn!("{}", err);
        Error::Custom(err.to_string())
    })
}

fn decode_block(block: &str) -> Result<BlockTag> {
    match block {
        "latest" => Ok(BlockTag::Latest),
        "finalized" => Ok(BlockTag::Finalized),
        _ => {
            if block.starts_with("0x") {
                Ok(BlockTag::Number(u64::from_str_radix(
                    block.strip_prefix("0x").unwrap(),
                    16,
                )?))
            } else {
                Ok(BlockTag::Number(block.parse()?))
            }
        }
    }
}
