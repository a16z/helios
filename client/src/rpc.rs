use consensus::database::Database;
use ethers::{
    abi::AbiEncode,
    types::{Address, Filter, Log, SyncingStatus, Transaction, TransactionReceipt, H256, U256},
};
use eyre::Result;
use std::net::{IpAddr, Ipv4Addr};
use std::{fmt::Display, net::SocketAddr, str::FromStr, sync::Arc};
use tracing::info;

use jsonrpsee::{
    core::{async_trait, server::Methods, Error},
    proc_macros::rpc,
    server::{ServerBuilder, ServerHandle},
};

use crate::{errors::NodeError, node::Node};

use common::{
    types::{Block, BlockTag},
    utils::{hex_str_to_bytes, u64_to_hex_string},
};
use execution::types::CallOpts;

pub struct Rpc<DB: Database> {
    node: Arc<Node<DB>>,
    handle: Option<ServerHandle>,
    address: SocketAddr,
}

impl<DB: Database> Rpc<DB> {
    pub fn new(node: Arc<Node<DB>>, ip: Option<IpAddr>, port: Option<u16>) -> Self {
        let address = SocketAddr::new(
            ip.unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST)),
            port.unwrap_or(0),
        );
        Rpc {
            node,
            handle: None,
            address,
        }
    }

    pub async fn start(&mut self) -> Result<SocketAddr> {
        let rpc_inner = RpcInner {
            node: self.node.clone(),
            address: self.address,
        };

        let (handle, addr) = start(rpc_inner).await?;
        self.handle = Some(handle);

        info!(target: "helios::rpc", "rpc server started at {}", addr);

        Ok(addr)
    }
}

#[rpc(server, namespace = "eth")]
trait EthRpc {
    #[method(name = "getBalance")]
    async fn get_balance(&self, address: &str, block: BlockTag) -> Result<String, Error>;
    #[method(name = "getTransactionCount")]
    async fn get_transaction_count(&self, address: &str, block: BlockTag) -> Result<String, Error>;
    #[method(name = "getBlockTransactionCountByHash")]
    async fn get_block_transaction_count_by_hash(&self, hash: H256) -> Result<String, Error>;
    #[method(name = "getBlockTransactionCountByNumber")]
    async fn get_block_transaction_count_by_number(&self, block: BlockTag)
        -> Result<String, Error>;
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
    ) -> Result<Option<Block>, Error>;
    #[method(name = "getBlockByHash")]
    async fn get_block_by_hash(&self, hash: H256, full_tx: bool) -> Result<Option<Block>, Error>;
    #[method(name = "sendRawTransaction")]
    async fn send_raw_transaction(&self, bytes: &str) -> Result<String, Error>;
    #[method(name = "getTransactionReceipt")]
    async fn get_transaction_receipt(
        &self,
        hash: H256,
    ) -> Result<Option<TransactionReceipt>, Error>;
    #[method(name = "getTransactionByHash")]
    async fn get_transaction_by_hash(&self, hash: H256) -> Result<Option<Transaction>, Error>;
    #[method(name = "getTransactionByBlockHashAndIndex")]
    async fn get_transaction_by_block_hash_and_index(
        &self,
        hash: H256,
        index: u64,
    ) -> Result<Option<Transaction>, Error>;
    #[method(name = "getLogs")]
    async fn get_logs(&self, filter: Filter) -> Result<Vec<Log>, Error>;
    #[method(name = "getFilterChanges")]
    async fn get_filter_changes(&self, filter_id: U256) -> Result<Vec<Log>, Error>;
    #[method(name = "uninstallFilter")]
    async fn uninstall_filter(&self, filter_id: U256) -> Result<bool, Error>;
    #[method(name = "getNewFilter")]
    async fn get_new_filter(&self, filter: Filter) -> Result<U256, Error>;
    #[method(name = "getNewBlockFilter")]
    async fn get_new_block_filter(&self) -> Result<U256, Error>;
    #[method(name = "getNewPendingTransactionFilter")]
    async fn get_new_pending_transaction_filter(&self) -> Result<U256, Error>;
    #[method(name = "getStorageAt")]
    async fn get_storage_at(
        &self,
        address: &str,
        slot: H256,
        block: BlockTag,
    ) -> Result<String, Error>;
    #[method(name = "coinbase")]
    async fn coinbase(&self) -> Result<Address, Error>;
    #[method(name = "syncing")]
    async fn syncing(&self) -> Result<SyncingStatus, Error>;
}

#[rpc(client, server, namespace = "net")]
trait NetRpc {
    #[method(name = "version")]
    async fn version(&self) -> Result<String, Error>;
}

#[derive(Clone)]
struct RpcInner<DB: Database> {
    node: Arc<Node<DB>>,
    address: SocketAddr,
}

#[async_trait]
impl<DB: Database> EthRpcServer for RpcInner<DB> {
    async fn get_balance(&self, address: &str, block: BlockTag) -> Result<String, Error> {
        let address = convert_err(Address::from_str(address))?;
        let balance = convert_err(self.node.get_balance(&address, block).await)?;

        Ok(format_hex(&balance))
    }

    async fn get_transaction_count(&self, address: &str, block: BlockTag) -> Result<String, Error> {
        let address = convert_err(Address::from_str(address))?;
        let nonce = convert_err(self.node.get_nonce(&address, block).await)?;

        Ok(format!("0x{nonce:x}"))
    }

    async fn get_block_transaction_count_by_hash(&self, hash: H256) -> Result<String, Error> {
        let transaction_count =
            convert_err(self.node.get_block_transaction_count_by_hash(&hash).await)?;
        Ok(u64_to_hex_string(transaction_count))
    }

    async fn get_block_transaction_count_by_number(
        &self,
        block: BlockTag,
    ) -> Result<String, Error> {
        let transaction_count =
            convert_err(self.node.get_block_transaction_count_by_number(block).await)?;
        Ok(u64_to_hex_string(transaction_count))
    }

    async fn get_code(&self, address: &str, block: BlockTag) -> Result<String, Error> {
        let address = convert_err(Address::from_str(address))?;
        let code = convert_err(self.node.get_code(&address, block).await)?;

        Ok(format!("0x{:}", hex::encode(code)))
    }

    async fn call(&self, opts: CallOpts, block: BlockTag) -> Result<String, Error> {
        let res = self
            .node
            .call(&opts, block)
            .await
            .map_err(NodeError::to_json_rpsee_error)?;

        Ok(format!("0x{}", hex::encode(res)))
    }

    async fn estimate_gas(&self, opts: CallOpts) -> Result<String, Error> {
        let gas = self
            .node
            .estimate_gas(&opts)
            .await
            .map_err(NodeError::to_json_rpsee_error)?;

        Ok(u64_to_hex_string(gas))
    }

    async fn chain_id(&self) -> Result<String, Error> {
        let id = self.node.chain_id();
        Ok(u64_to_hex_string(id))
    }

    async fn gas_price(&self) -> Result<String, Error> {
        let gas_price = convert_err(self.node.get_gas_price().await)?;
        Ok(format_hex(&gas_price))
    }

    async fn max_priority_fee_per_gas(&self) -> Result<String, Error> {
        let tip = convert_err(self.node.get_priority_fee())?;
        Ok(format_hex(&tip))
    }

    async fn block_number(&self) -> Result<String, Error> {
        let num = convert_err(self.node.get_block_number().await)?;
        Ok(u64_to_hex_string(num.as_u64()))
    }

    async fn get_block_by_number(
        &self,
        block: BlockTag,
        full_tx: bool,
    ) -> Result<Option<Block>, Error> {
        let block = convert_err(self.node.get_block_by_number(block, full_tx).await)?;
        Ok(block)
    }

    async fn get_block_by_hash(&self, hash: H256, full_tx: bool) -> Result<Option<Block>, Error> {
        let block = convert_err(self.node.get_block_by_hash(&hash, full_tx).await)?;
        Ok(block)
    }

    async fn send_raw_transaction(&self, bytes: &str) -> Result<String, Error> {
        let bytes = convert_err(hex_str_to_bytes(bytes))?;
        let tx_hash = convert_err(self.node.send_raw_transaction(&bytes).await)?;
        Ok(hex::encode(tx_hash))
    }

    async fn get_transaction_receipt(
        &self,
        hash: H256,
    ) -> Result<Option<TransactionReceipt>, Error> {
        let receipt = convert_err(self.node.get_transaction_receipt(&hash).await)?;
        Ok(receipt)
    }

    async fn get_transaction_by_hash(&self, hash: H256) -> Result<Option<Transaction>, Error> {
        Ok(self.node.get_transaction_by_hash(&hash).await)
    }

    async fn get_transaction_by_block_hash_and_index(
        &self,
        hash: H256,
        index: u64,
    ) -> Result<Option<Transaction>, Error> {
        Ok(self
            .node
            .get_transaction_by_block_hash_and_index(&hash, index)
            .await)
    }

    async fn coinbase(&self) -> Result<Address, Error> {
        convert_err(self.node.get_coinbase().await)
    }

    async fn syncing(&self) -> Result<SyncingStatus, Error> {
        convert_err(self.node.syncing().await)
    }

    async fn get_logs(&self, filter: Filter) -> Result<Vec<Log>, Error> {
        convert_err(self.node.get_logs(&filter).await)
    }

    async fn get_filter_changes(&self, filter_id: U256) -> Result<Vec<Log>, Error> {
        convert_err(self.node.get_filter_changes(&filter_id).await)
    }

    async fn uninstall_filter(&self, filter_id: U256) -> Result<bool, Error> {
        convert_err(self.node.uninstall_filter(&filter_id).await)
    }

    async fn get_new_filter(&self, filter: Filter) -> Result<U256, Error> {
        convert_err(self.node.get_new_filter(&filter).await)
    }

    async fn get_new_block_filter(&self) -> Result<U256, Error> {
        convert_err(self.node.get_new_block_filter().await)
    }

    async fn get_new_pending_transaction_filter(&self) -> Result<U256, Error> {
        convert_err(self.node.get_new_pending_transaction_filter().await)
    }

    async fn get_storage_at(
        &self,
        address: &str,
        slot: H256,
        block: BlockTag,
    ) -> Result<String, Error> {
        let address = convert_err(Address::from_str(address))?;
        let storage = convert_err(self.node.get_storage_at(&address, slot, block).await)?;

        Ok(format_hex(&storage))
    }
}

#[async_trait]
impl<DB: Database> NetRpcServer for RpcInner<DB> {
    async fn version(&self) -> Result<String, Error> {
        Ok(self.node.chain_id().to_string())
    }
}

async fn start<DB: Database>(rpc: RpcInner<DB>) -> Result<(ServerHandle, SocketAddr)> {
    let server = ServerBuilder::default().build(rpc.address).await?;
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
    res.map_err(|err| Error::Custom(err.to_string()))
}

fn format_hex(num: &U256) -> String {
    let stripped = num
        .encode_hex()
        .strip_prefix("0x")
        .unwrap()
        .trim_start_matches('0')
        .to_string();

    let stripped = if stripped.is_empty() {
        "0".to_string()
    } else {
        stripped
    };

    format!("0x{stripped}")
}
