use ethers::{
    abi::AbiEncode,
    types::{Address, Transaction, TransactionReceipt, H256},
};
use eyre::Result;
use log::{debug, info, warn};
use std::{fmt::Display, net::SocketAddr, str::FromStr, sync::Arc};
use tokio::sync::RwLock;

use jsonrpsee::{
    core::{async_trait, server::rpc_module::Methods, Error},
    http_server::{HttpServerBuilder, HttpServerHandle},
    proc_macros::rpc,
};

use crate::node::Node;

use common::{
    types::BlockTag,
    utils::{hex_str_to_bytes, u64_to_hex_string},
};
use execution::types::{CallOpts, ExecutionBlock};

pub struct Rpc {
    node: Arc<RwLock<Node>>,
    handle: Option<HttpServerHandle>,
    port: u16,
}

impl Rpc {
    pub fn new(node: Arc<RwLock<Node>>, port: u16) -> Self {
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

        info!("rpc server started at {}", addr);

        Ok(addr)
    }
}

#[rpc(server, namespace = "eth")]
trait EthRpc {
    #[method(name = "getBalance")]
    async fn get_balance(&self, address: &str, block: BlockTag) -> Result<String, Error>;
    #[method(name = "getTransactionCount")]
    async fn get_transaction_count(&self, address: &str, block: BlockTag) -> Result<String, Error>;
    #[method(name = "getCode")]
    async fn get_code(&self, address: &str, block: BlockTag) -> Result<String, Error>;
    #[method(name = "call")]
    async fn call(&self, opts: CallOpts, block: BlockTag) -> Result<String, Error>;
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
    async fn get_block_by_number(
        &self,
        block: BlockTag,
        full_tx: bool,
    ) -> Result<Option<ExecutionBlock>, Error>;
    #[method(name = "getBlockByHash")]
    async fn get_block_by_hash(
        &self,
        hash: &str,
        full_tx: bool,
    ) -> Result<Option<ExecutionBlock>, Error>;
    #[method(name = "sendRawTransaction")]
    async fn send_raw_transaction(&self, bytes: &str) -> Result<String, Error>;
    #[method(name = "getTransactionReceipt")]
    async fn get_transaction_receipt(
        &self,
        hash: &str,
    ) -> Result<Option<TransactionReceipt>, Error>;
    #[method(name = "getTransactionByHash")]
    async fn get_transaction_by_hash(&self, hash: &str) -> Result<Option<Transaction>, Error>;
}

#[rpc(client, server, namespace = "net")]
trait NetRpc {
    #[method(name = "version")]
    async fn version(&self) -> Result<String, Error>;
}

#[derive(Clone)]
struct RpcInner {
    node: Arc<RwLock<Node>>,
    port: u16,
}

#[async_trait]
impl EthRpcServer for RpcInner {
    async fn get_balance(&self, address: &str, block: BlockTag) -> Result<String, Error> {
        debug!("eth_getBalance");
        let address = convert_err(Address::from_str(address))?;
        let node = self.node.read().await;
        let balance = convert_err(node.get_balance(&address, &block).await)?;

        Ok(balance.encode_hex())
    }

    async fn get_transaction_count(&self, address: &str, block: BlockTag) -> Result<String, Error> {
        let address = convert_err(Address::from_str(address))?;
        let node = self.node.read().await;
        let nonce = convert_err(node.get_nonce(&address, &block).await)?;

        Ok(nonce.encode_hex())
    }

    async fn get_code(&self, address: &str, block: BlockTag) -> Result<String, Error> {
        let address = convert_err(Address::from_str(address))?;
        let node = self.node.read().await;
        let code = convert_err(node.get_code(&address, &block).await)?;

        Ok(hex::encode(code))
    }

    async fn call(&self, opts: CallOpts, block: BlockTag) -> Result<String, Error> {
        debug!("eth_call");
        let node = self.node.read().await;
        let res = convert_err(node.call(&opts, &block))?;

        Ok(format!("0x{}", hex::encode(res)))
    }

    async fn estimate_gas(&self, opts: CallOpts) -> Result<String, Error> {
        debug!("eth_estimateGas");
        let node = self.node.read().await;
        let gas = convert_err(node.estimate_gas(&opts))?;

        Ok(u64_to_hex_string(gas))
    }

    async fn chain_id(&self) -> Result<String, Error> {
        let node = self.node.read().await;
        let id = node.chain_id();
        Ok(u64_to_hex_string(id))
    }

    async fn gas_price(&self) -> Result<String, Error> {
        let node = self.node.read().await;
        let gas_price = convert_err(node.get_gas_price())?;
        Ok(gas_price.encode_hex())
    }

    async fn max_priority_fee_per_gas(&self) -> Result<String, Error> {
        let node = self.node.read().await;
        let tip = convert_err(node.get_priority_fee())?;
        Ok(tip.encode_hex())
    }

    async fn block_number(&self) -> Result<String, Error> {
        let node = self.node.read().await;
        let num = convert_err(node.get_block_number())?;
        Ok(u64_to_hex_string(num))
    }

    async fn get_block_by_number(
        &self,
        block: BlockTag,
        _full_tx: bool,
    ) -> Result<Option<ExecutionBlock>, Error> {
        let node = self.node.read().await;
        let block = convert_err(node.get_block_by_number(&block))?;
        Ok(block)
    }

    async fn get_block_by_hash(
        &self,
        hash: &str,
        _full_tx: bool,
    ) -> Result<Option<ExecutionBlock>, Error> {
        let hash = convert_err(hex_str_to_bytes(hash))?;
        let node = self.node.read().await;
        let block = convert_err(node.get_block_by_hash(&hash))?;
        Ok(block)
    }

    async fn send_raw_transaction(&self, bytes: &str) -> Result<String, Error> {
        let node = self.node.read().await;
        let bytes = convert_err(hex_str_to_bytes(bytes))?;
        let tx_hash = convert_err(node.send_raw_transaction(&bytes).await)?;
        Ok(hex::encode(tx_hash))
    }

    async fn get_transaction_receipt(
        &self,
        hash: &str,
    ) -> Result<Option<TransactionReceipt>, Error> {
        let node = self.node.read().await;
        let hash = H256::from_slice(&convert_err(hex_str_to_bytes(hash))?);
        let receipt = convert_err(node.get_transaction_receipt(&hash).await)?;
        Ok(receipt)
    }

    async fn get_transaction_by_hash(&self, hash: &str) -> Result<Option<Transaction>, Error> {
        let node = self.node.read().await;
        let hash = H256::from_slice(&convert_err(hex_str_to_bytes(hash))?);
        convert_err(node.get_transaction_by_hash(&hash).await)
    }
}

#[async_trait]
impl NetRpcServer for RpcInner {
    async fn version(&self) -> Result<String, Error> {
        let node = self.node.read().await;
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
