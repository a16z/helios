use std::marker::PhantomData;
use std::sync::Arc;
use std::{fmt::Display, net::SocketAddr};

use alloy::eips::{BlockId, BlockNumberOrTag};
use alloy::network::{BlockResponse, ReceiptResponse, TransactionResponse};
use alloy::primitives::{Address, Bytes, B256, U256, U64};
use alloy::rpc::json_rpc::RpcObject;
use alloy::rpc::types::{
    AccessListResult, EIP1186AccountProofResponse, Filter, FilterChanges, Log, SyncStatus,
};
use eyre::{eyre, Result};
use jsonrpsee::{
    core::{async_trait, server::Methods, SubscriptionResult},
    proc_macros::rpc,
    server::{PendingSubscriptionSink, ServerBuilder, ServerHandle, SubscriptionMessage},
    types::error::{ErrorObject, ErrorObjectOwned},
};

use helios_common::{
    network_spec::NetworkSpec,
    types::{SubEventRx, SubscriptionType},
};

use crate::client::api::HeliosApi;

pub type Handle = ServerHandle;

pub async fn start<N: NetworkSpec>(
    client: Arc<dyn HeliosApi<N>>,
    addr: SocketAddr,
) -> Result<ServerHandle> {
    let server = ServerBuilder::default().build(addr).await?;
    let rpc = JsonRpc {
        client,
        phantom: PhantomData,
    };

    let mut methods = Methods::new();
    let eth_methods: Methods = EthRpcServer::into_rpc(rpc.clone()).into();
    let net_methods: Methods = NetRpcServer::into_rpc(rpc.clone()).into();
    let web3_methods: Methods = Web3RpcServer::into_rpc(rpc).into();

    methods.merge(eth_methods)?;
    methods.merge(net_methods)?;
    methods.merge(web3_methods)?;

    Ok(server.start(methods))
}

#[derive(Clone)]
struct JsonRpc<N: NetworkSpec> {
    client: Arc<dyn HeliosApi<N>>,
    phantom: PhantomData<N>,
}

#[rpc(server, namespace = "eth")]
trait EthRpc<
    TX: TransactionResponse + RpcObject,
    TXR: RpcObject,
    R: ReceiptResponse + RpcObject,
    B: BlockResponse + RpcObject,
>
{
    #[method(name = "getBalance")]
    async fn get_balance(&self, address: Address, block: BlockId)
        -> Result<U256, ErrorObjectOwned>;
    #[method(name = "getTransactionCount")]
    async fn get_transaction_count(
        &self,
        address: Address,
        block: BlockId,
    ) -> Result<U64, ErrorObjectOwned>;
    #[method(name = "getBlockTransactionCountByHash")]
    async fn get_block_transaction_count_by_hash(
        &self,
        hash: B256,
    ) -> Result<Option<U64>, ErrorObjectOwned>;
    #[method(name = "getBlockTransactionCountByNumber")]
    async fn get_block_transaction_count_by_number(
        &self,
        block: BlockNumberOrTag,
    ) -> Result<Option<U64>, ErrorObjectOwned>;
    #[method(name = "getCode")]
    async fn get_code(&self, address: Address, block: BlockId) -> Result<Bytes, ErrorObjectOwned>;
    #[method(name = "call")]
    async fn call(&self, tx: TXR, block: BlockId) -> Result<Bytes, ErrorObjectOwned>;
    #[method(name = "estimateGas")]
    async fn estimate_gas(&self, tx: TXR, block: BlockId) -> Result<U64, ErrorObjectOwned>;
    #[method(name = "createAccessList")]
    async fn create_access_list(
        &self,
        tx: TXR,
        block: BlockId,
    ) -> Result<AccessListResult, ErrorObjectOwned>;
    #[method(name = "chainId")]
    async fn chain_id(&self) -> Result<U64, ErrorObjectOwned>;
    #[method(name = "gasPrice")]
    async fn gas_price(&self) -> Result<U256, ErrorObjectOwned>;
    #[method(name = "maxPriorityFeePerGas")]
    async fn max_priority_fee_per_gas(&self) -> Result<U256, ErrorObjectOwned>;
    #[method(name = "blobBaseFee")]
    async fn blob_base_fee(&self) -> Result<U256, ErrorObjectOwned>;
    #[method(name = "blockNumber")]
    async fn block_number(&self) -> Result<U64, ErrorObjectOwned>;
    #[method(name = "getBlockByNumber")]
    async fn get_block_by_number(
        &self,
        block: BlockNumberOrTag,
        full_tx: bool,
    ) -> Result<Option<B>, ErrorObjectOwned>;
    #[method(name = "getBlockByHash")]
    async fn get_block_by_hash(
        &self,
        hash: B256,
        full_tx: bool,
    ) -> Result<Option<B>, ErrorObjectOwned>;
    #[method(name = "sendRawTransaction")]
    async fn send_raw_transaction(&self, bytes: Bytes) -> Result<B256, ErrorObjectOwned>;
    #[method(name = "getTransactionReceipt")]
    async fn get_transaction_receipt(&self, hash: B256) -> Result<Option<R>, ErrorObjectOwned>;
    #[method(name = "getBlockReceipts")]
    async fn get_block_receipts(&self, block: BlockId) -> Result<Option<Vec<R>>, ErrorObjectOwned>;
    #[method(name = "getTransactionByHash")]
    async fn get_transaction_by_hash(&self, hash: B256) -> Result<Option<TX>, ErrorObjectOwned>;
    #[method(name = "getTransactionByBlockHashAndIndex")]
    async fn get_transaction_by_block_hash_and_index(
        &self,
        hash: B256,
        index: U64,
    ) -> Result<Option<TX>, ErrorObjectOwned>;
    #[method(name = "getTransactionByBlockNumberAndIndex")]
    async fn get_transaction_by_block_number_and_index(
        &self,
        block: BlockNumberOrTag,
        index: U64,
    ) -> Result<Option<TX>, ErrorObjectOwned>;
    #[method(name = "getLogs")]
    async fn get_logs(&self, filter: Filter) -> Result<Vec<Log>, ErrorObjectOwned>;
    #[method(name = "getFilterChanges")]
    async fn get_filter_changes(&self, filter_id: U256) -> Result<FilterChanges, ErrorObjectOwned>;
    #[method(name = "getFilterLogs")]
    async fn get_filter_logs(&self, filter_id: U256) -> Result<Vec<Log>, ErrorObjectOwned>;
    #[method(name = "uninstallFilter")]
    async fn uninstall_filter(&self, filter_id: U256) -> Result<bool, ErrorObjectOwned>;
    #[method(name = "newFilter")]
    async fn new_filter(&self, filter: Filter) -> Result<U256, ErrorObjectOwned>;
    #[method(name = "newBlockFilter")]
    async fn new_block_filter(&self) -> Result<U256, ErrorObjectOwned>;
    #[method(name = "getStorageAt")]
    async fn get_storage_at(
        &self,
        address: Address,
        slot: U256,
        block: BlockId,
    ) -> Result<B256, ErrorObjectOwned>;
    #[method(name = "getProof")]
    async fn get_proof(
        &self,
        address: Address,
        storage_keys: Vec<U256>,
        block: BlockId,
    ) -> Result<EIP1186AccountProofResponse, ErrorObjectOwned>;
    #[method(name = "coinbase")]
    async fn coinbase(&self) -> Result<Address, ErrorObjectOwned>;
    #[method(name = "syncing")]
    async fn syncing(&self) -> Result<SyncStatus, ErrorObjectOwned>;
    #[subscription(name = "subscribe", unsubscribe = "unsubscribe", item = String)]
    async fn subscribe(&self, event_type: SubscriptionType) -> SubscriptionResult;
}

#[rpc(client, server, namespace = "net")]
trait NetRpc {
    #[method(name = "version")]
    async fn version(&self) -> Result<u64, ErrorObjectOwned>;
}

#[rpc(client, server, namespace = "web3")]
trait Web3Rpc {
    #[method(name = "clientVersion")]
    async fn client_version(&self) -> Result<String, ErrorObjectOwned>;
}

#[async_trait]
impl<N: NetworkSpec>
    EthRpcServer<
        N::TransactionResponse,
        N::TransactionRequest,
        N::ReceiptResponse,
        N::BlockResponse,
    > for JsonRpc<N>
{
    async fn get_balance(
        &self,
        address: Address,
        block: BlockId,
    ) -> Result<U256, ErrorObjectOwned> {
        convert_err(self.client.get_balance(address, block).await)
    }

    async fn get_transaction_count(
        &self,
        address: Address,
        block: BlockId,
    ) -> Result<U64, ErrorObjectOwned> {
        convert_err(self.client.get_nonce(address, block).await).map(U64::from)
    }

    async fn get_block_transaction_count_by_hash(
        &self,
        hash: B256,
    ) -> Result<Option<U64>, ErrorObjectOwned> {
        convert_err(
            self.client
                .get_block_transaction_count(hash.into())
                .await
                .map(|opt| opt.map(U64::from)),
        )
    }

    async fn get_block_transaction_count_by_number(
        &self,
        block: BlockNumberOrTag,
    ) -> Result<Option<U64>, ErrorObjectOwned> {
        convert_err(
            self.client
                .get_block_transaction_count(block.into())
                .await
                .map(|opt| opt.map(U64::from)),
        )
    }

    async fn get_code(&self, address: Address, block: BlockId) -> Result<Bytes, ErrorObjectOwned> {
        convert_err(self.client.get_code(address, block).await)
    }

    async fn call(
        &self,
        tx: N::TransactionRequest,
        block: BlockId,
    ) -> Result<Bytes, ErrorObjectOwned> {
        convert_err(self.client.call(&tx, block).await)
    }

    async fn estimate_gas(
        &self,
        tx: N::TransactionRequest,
        block: BlockId,
    ) -> Result<U64, ErrorObjectOwned> {
        let res = self.client.estimate_gas(&tx, block).await.map(U64::from);

        convert_err(res)
    }

    async fn create_access_list(
        &self,
        tx: N::TransactionRequest,
        block: BlockId,
    ) -> Result<AccessListResult, ErrorObjectOwned> {
        convert_err(self.client.create_access_list(&tx, block).await)
    }

    async fn chain_id(&self) -> Result<U64, ErrorObjectOwned> {
        Ok(U64::from(self.client.get_chain_id().await))
    }

    async fn gas_price(&self) -> Result<U256, ErrorObjectOwned> {
        convert_err(self.client.get_gas_price().await)
    }

    async fn max_priority_fee_per_gas(&self) -> Result<U256, ErrorObjectOwned> {
        convert_err(self.client.get_priority_fee().await)
    }

    async fn blob_base_fee(&self) -> Result<U256, ErrorObjectOwned> {
        convert_err(self.client.get_blob_base_fee().await)
    }

    async fn block_number(&self) -> Result<U64, ErrorObjectOwned> {
        convert_err(self.client.get_block_number().await).map(U64::from)
    }

    async fn get_block_by_number(
        &self,
        block: BlockNumberOrTag,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>, ErrorObjectOwned> {
        convert_err(self.client.get_block(block.into(), full_tx).await)
    }

    async fn get_block_by_hash(
        &self,
        hash: B256,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>, ErrorObjectOwned> {
        convert_err(self.client.get_block(hash.into(), full_tx).await)
    }

    async fn send_raw_transaction(&self, bytes: Bytes) -> Result<B256, ErrorObjectOwned> {
        convert_err(self.client.send_raw_transaction(&bytes).await)
    }

    async fn get_transaction_receipt(
        &self,
        hash: B256,
    ) -> Result<Option<N::ReceiptResponse>, ErrorObjectOwned> {
        convert_err(self.client.get_transaction_receipt(hash).await)
    }

    async fn get_block_receipts(
        &self,
        block: BlockId,
    ) -> Result<Option<Vec<N::ReceiptResponse>>, ErrorObjectOwned> {
        convert_err(self.client.get_block_receipts(block).await)
    }

    async fn get_transaction_by_hash(
        &self,
        hash: B256,
    ) -> Result<Option<N::TransactionResponse>, ErrorObjectOwned> {
        convert_err(self.client.get_transaction(hash).await)
    }

    async fn get_transaction_by_block_hash_and_index(
        &self,
        hash: B256,
        index: U64,
    ) -> Result<Option<N::TransactionResponse>, ErrorObjectOwned> {
        convert_err(
            self.client
                .get_transaction_by_block_and_index(hash.into(), index.to())
                .await,
        )
    }

    async fn get_transaction_by_block_number_and_index(
        &self,
        block: BlockNumberOrTag,
        index: U64,
    ) -> Result<Option<N::TransactionResponse>, ErrorObjectOwned> {
        convert_err(
            self.client
                .get_transaction_by_block_and_index(block.into(), index.to())
                .await,
        )
    }

    async fn coinbase(&self) -> Result<Address, ErrorObjectOwned> {
        convert_err(self.client.get_coinbase().await)
    }

    async fn syncing(&self) -> Result<SyncStatus, ErrorObjectOwned> {
        convert_err(self.client.syncing().await)
    }

    async fn get_logs(&self, filter: Filter) -> Result<Vec<Log>, ErrorObjectOwned> {
        convert_err(self.client.get_logs(&filter).await)
    }

    async fn get_filter_changes(
        &self,
        _filter_id: U256,
    ) -> Result<FilterChanges, ErrorObjectOwned> {
        convert_err(Err(eyre!("not implemented")))
    }

    async fn get_filter_logs(&self, filter_id: U256) -> Result<Vec<Log>, ErrorObjectOwned> {
        convert_err(self.client.get_filter_logs(filter_id).await)
    }

    async fn uninstall_filter(&self, filter_id: U256) -> Result<bool, ErrorObjectOwned> {
        convert_err(self.client.uninstall_filter(filter_id).await)
    }

    async fn new_filter(&self, filter: Filter) -> Result<U256, ErrorObjectOwned> {
        convert_err(self.client.new_filter(&filter).await)
    }

    async fn new_block_filter(&self) -> Result<U256, ErrorObjectOwned> {
        convert_err(self.client.new_block_filter().await)
    }

    async fn get_storage_at(
        &self,
        address: Address,
        slot: U256,
        block: BlockId,
    ) -> Result<B256, ErrorObjectOwned> {
        convert_err(self.client.get_storage_at(address, slot, block).await)
    }

    async fn get_proof(
        &self,
        address: Address,
        storage_keys: Vec<U256>,
        block: BlockId,
    ) -> Result<EIP1186AccountProofResponse, ErrorObjectOwned> {
        let slots = storage_keys
            .into_iter()
            .map(|k| k.into())
            .collect::<Vec<_>>();

        convert_err(self.client.get_proof(address, &slots, block).await)
    }

    async fn subscribe(
        &self,
        pending: PendingSubscriptionSink,
        event_type: SubscriptionType,
    ) -> SubscriptionResult {
        let maybe_rx = self.client.subscribe(event_type).await;

        handle_subscription(pending, maybe_rx).await
    }
}

#[async_trait]
impl<N: NetworkSpec> NetRpcServer for JsonRpc<N> {
    async fn version(&self) -> Result<u64, ErrorObjectOwned> {
        Ok(self.client.get_chain_id().await)
    }
}

#[async_trait]
impl<N: NetworkSpec> Web3RpcServer for JsonRpc<N> {
    async fn client_version(&self) -> Result<String, ErrorObjectOwned> {
        Ok(self.client.get_client_version().await)
    }
}

fn convert_err<T, E: Display>(res: Result<T, E>) -> Result<T, ErrorObjectOwned> {
    res.map_err(|err| ErrorObject::owned(1, err.to_string(), None::<()>))
}

/// Helper function to handle subscription acceptance/rejection and message forwarding
async fn handle_subscription<N: NetworkSpec>(
    pending: PendingSubscriptionSink,
    maybe_rx: Result<SubEventRx<N>>,
) -> SubscriptionResult {
    match maybe_rx {
        Ok(mut stream) => {
            let sink = pending.accept().await?;

            tokio::spawn(async move {
                while let Ok(message) = stream.recv().await {
                    let msg = SubscriptionMessage::from_json(&message).unwrap();
                    if sink.send(msg).await.is_err() {
                        break;
                    }
                }
            });
            Ok(())
        }
        Err(e) => {
            pending
                .reject(ErrorObject::owned(
                    2000,
                    "Subscription failed",
                    Some(e.to_string()),
                ))
                .await;
            Ok(())
        }
    }
}
