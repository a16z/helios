use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use alloy::{
    consensus::{Account, BlockHeader},
    eips::BlockNumberOrTag,
    network::{BlockResponse, ReceiptResponse},
    primitives::{Address, B256, U256},
    rpc::types::{BlockId, BlockTransactionsKind, Filter, FilterChanges, Log},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use axum_extra::extract::Query;
use eyre::{eyre, OptionExt, Report, Result};
use futures::future::try_join_all;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;

use helios_common::network_spec::NetworkSpec;
use helios_core::execution::{proof::create_receipt_proof, rpc::ExecutionRpc};
use helios_verifiable_api_types::*;

use crate::{ApiState, ExecutionClient};

#[allow(type_alias_bounds)]
type Response<T: Serialize + DeserializeOwned> =
    Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

fn json_err(error: &str) -> Json<serde_json::Value> {
    Json(json!({ "error": error }))
}

fn map_server_err(e: Report) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::INTERNAL_SERVER_ERROR, json_err(&e.to_string()))
}

#[derive(Deserialize)]
pub struct BlockQuery {
    block: Option<BlockId>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountProofQuery {
    #[serde(default)]
    pub storage_keys: Vec<B256>,
    pub block: Option<BlockId>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsQuery {
    pub from_block: Option<BlockNumberOrTag>,
    pub to_block: Option<BlockNumberOrTag>,
    pub block_hash: Option<B256>,
    #[serde(default)]
    pub address: Vec<Address>,
    #[serde(default)]
    pub topics: Vec<B256>,
}

impl TryInto<Filter> for LogsQuery {
    type Error = Report;

    fn try_into(self) -> Result<Filter, Self::Error> {
        // Validate the filter
        // We don't need extensive validation here since the RPC will do it for us
        if self.from_block.is_some() && self.to_block.is_some() && self.block_hash.is_some() {
            return Err(eyre!(
                "cannot specify both blockHash and fromBlock/toBlock, choose one or the other"
            ));
        }
        if self.topics.len() > 4 {
            return Err(eyre!("too many topics, max 4 allowed"));
        }

        let mut filter = Filter::new();
        if let Some(from_block) = self.from_block {
            filter = filter.from_block(from_block);
        }
        if let Some(to_block) = self.to_block {
            filter = filter.to_block(to_block);
        }
        if let Some(block_hash) = self.block_hash {
            filter = filter.at_block_hash(block_hash);
        }
        if !self.address.is_empty() {
            filter = filter.address(self.address);
        }
        if !self.topics.is_empty() {
            if let Some(topic0) = self.topics.get(0) {
                filter = filter.event_signature(*topic0);
            }
            if let Some(topic1) = self.topics.get(1) {
                filter = filter.topic1(*topic1);
            }
            if let Some(topic2) = self.topics.get(2) {
                filter = filter.topic2(*topic2);
            }
            if let Some(topic3) = self.topics.get(3) {
                filter = filter.topic3(*topic3);
            }
        }

        Ok(filter)
    }
}

/// This method returns the EIP-1186 account proof for a given address.
///
/// Replaces the `eth_getProof`, `eth_getTransactionCount`, `eth_getBalance` and `eth_getCode` RPC methods.
pub async fn get_account<N: NetworkSpec, R: ExecutionRpc<N>>(
    Path(address): Path<Address>,
    Query(AccountProofQuery {
        storage_keys,
        block,
    }): Query<AccountProofQuery>,
    State(ApiState { execution_client }): State<ApiState<N, R>>,
) -> Response<AccountResponse> {
    // Ensure that BlockId is of block number variant
    let block = block.unwrap_or(BlockId::latest());
    let block_num = match block {
        BlockId::Number(number_or_tag) => match number_or_tag {
            BlockNumberOrTag::Number(number) => Ok(number),
            tag => execution_client
                .rpc
                .get_block_by_number(tag, BlockTransactionsKind::Hashes)
                .await
                .map_err(map_server_err)?
                .ok_or_else(|| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        json_err("Block not found"),
                    )
                })
                .map(|block| block.header().number()),
        },
        BlockId::Hash(hash) => execution_client
            .rpc
            .get_block(hash.into())
            .await
            .map_err(map_server_err)
            .map(|block| block.header().number()),
    }?;
    let block = BlockId::from(block_num);

    let proof = execution_client
        .rpc
        .get_proof(address, &storage_keys, block)
        .await
        .map_err(map_server_err)?;

    let code = execution_client
        .rpc
        .get_code(address, block_num)
        .await
        .map_err(map_server_err)?;

    Ok(Json(AccountResponse {
        account: Account {
            balance: proof.balance,
            nonce: proof.nonce,
            code_hash: proof.code_hash,
            storage_root: proof.storage_hash,
        },
        code: code.into(),
        account_proof: proof.account_proof,
        storage_proof: proof.storage_proof,
    }))
}

/// This method returns the value stored at a specific storage position for a given address.
/// It provides the state of the contract's storage, which may not be exposed via the contract's methods.
/// The storage value can be verified against the given storage and account proofs.
///
/// Replaces the `eth_getStorageAt` RPC method.
pub async fn get_storage_at<N: NetworkSpec, R: ExecutionRpc<N>>(
    Path((address, key)): Path<(Address, U256)>,
    Query(BlockQuery { block }): Query<BlockQuery>,
    State(ApiState { execution_client }): State<ApiState<N, R>>,
) -> Response<StorageAtResponse> {
    let block = block.unwrap_or(BlockId::latest());

    let storage_slot = execution_client
        .rpc
        .get_storage_at(address, key, block)
        .await
        .map_err(map_server_err)?;

    let proof = execution_client
        .rpc
        .get_proof(address, &[storage_slot], block)
        .await
        .map_err(map_server_err)?;

    let storage = match proof.storage_proof.get(0) {
        Some(storage) => storage.clone(),
        None => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                json_err("Failed to get storage proof"),
            ))
        }
    };

    Ok(Json(StorageAtResponse {
        storage,
        account: Account {
            balance: proof.balance,
            nonce: proof.nonce,
            code_hash: proof.code_hash,
            storage_root: proof.storage_hash,
        },
        account_proof: proof.account_proof,
    }))
}

/// This method returns all transaction receipts for a given block.
///
/// Replaces the `eth_getBlockReceipts` RPC method.
pub async fn get_block_receipts<N: NetworkSpec, R: ExecutionRpc<N>>(
    Path(block): Path<BlockId>,
    State(ApiState { execution_client }): State<ApiState<N, R>>,
) -> Response<BlockReceiptsResponse<N>> {
    let receipts = execution_client
        .rpc
        .get_block_receipts(block)
        .await
        .map_err(map_server_err)?
        .unwrap_or(vec![]);

    Ok(Json(receipts))
}

/// This method returns the receipt of a transaction along with a Merkle proof of its inclusion.
///
/// Replaces the `eth_getTransactionReceipt` RPC method.
pub async fn get_transaction_receipt<N: NetworkSpec, R: ExecutionRpc<N>>(
    Path(tx_hash): Path<B256>,
    State(ApiState { execution_client }): State<ApiState<N, R>>,
) -> Response<Option<TransactionReceiptResponse<N>>> {
    let receipt = execution_client
        .rpc
        .get_transaction_receipt(tx_hash)
        .await
        .map_err(map_server_err)?;
    if receipt.is_none() {
        return Ok(Json(None));
    }
    let receipt = receipt.unwrap();

    let receipts = execution_client
        .rpc
        .get_block_receipts(BlockId::from(receipt.block_number().unwrap()))
        .await
        .map_err(map_server_err)?
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                json_err("No receipts found for the block"),
            )
        })?;

    let receipt_proof =
        create_receipt_proof::<N>(receipts, receipt.transaction_index().unwrap() as usize);

    Ok(Json(Some(TransactionReceiptResponse {
        receipt,
        receipt_proof,
    })))
}

/// This method returns an array of all logs matching the given filter.
/// Corresponding to each log, it also returns the transaction receipt and a Merkle proof of its inclusion.
///
/// Replaces the `eth_getLogs` RPC method.
pub async fn get_logs<N: NetworkSpec, R: ExecutionRpc<N>>(
    Query(logs_filter_query): Query<LogsQuery>,
    State(ApiState { execution_client }): State<ApiState<N, R>>,
) -> Response<LogsResponse<N>> {
    let filter: Filter = logs_filter_query.try_into().map_err(map_server_err)?;

    // Fetch the filter logs from RPC
    let logs = execution_client
        .rpc
        .get_logs(&filter)
        .await
        .map_err(map_server_err)?;

    // Create receipt proofs for each log
    let receipt_proofs = create_receipt_proofs_for_logs(&logs, execution_client)
        .await
        .map_err(map_server_err)?;

    Ok(Json(LogsResponse {
        logs,
        receipt_proofs,
    }))
}

/// This method returns an array of all logs matching the filter with given id.
/// Corresponding to each log, it also returns the transaction receipt and a Merkle proof of its inclusion.
///
/// Replaces the `eth_getFilterLogs` RPC method.
pub async fn get_filter_logs<N: NetworkSpec, R: ExecutionRpc<N>>(
    Path(filter_id): Path<U256>,
    State(ApiState { execution_client }): State<ApiState<N, R>>,
) -> Response<FilterLogsResponse<N>> {
    // Fetch the filter logs from RPC
    let logs = execution_client
        .rpc
        .get_filter_logs(filter_id)
        .await
        .map_err(map_server_err)?;

    // Create receipt proofs for each log
    let receipt_proofs = create_receipt_proofs_for_logs(&logs, execution_client)
        .await
        .map_err(map_server_err)?;

    Ok(Json(FilterLogsResponse {
        logs,
        receipt_proofs,
    }))
}

/// This method returns the changes since the last poll for a given filter id.
/// If filter is of logs type, then corresponding to each log,
/// it also returns the transaction receipt and a Merkle proof of its inclusion..
///
/// Replaces the `eth_getFilterChanges` RPC method.
pub async fn get_filter_changes<N: NetworkSpec, R: ExecutionRpc<N>>(
    Path(filter_id): Path<U256>,
    State(ApiState { execution_client }): State<ApiState<N, R>>,
) -> Response<FilterChangesResponse<N>> {
    let filter_changes = execution_client
        .rpc
        .get_filter_changes(filter_id)
        .await
        .map_err(map_server_err)?;

    Ok(Json(match filter_changes {
        FilterChanges::Logs(logs) => {
            // Create receipt proofs for each log
            let receipt_proofs = create_receipt_proofs_for_logs(&logs, execution_client)
                .await
                .map_err(map_server_err)?;

            FilterChangesResponse::Logs(FilterLogsResponse {
                logs,
                receipt_proofs,
            })
        }
        FilterChanges::Hashes(hashes) => FilterChangesResponse::Hashes(hashes),
        FilterChanges::Empty => FilterChangesResponse::Hashes(vec![]),
        FilterChanges::Transactions(txs) => FilterChangesResponse::Hashes(
            txs.into_iter().map(|t| t.inner.tx_hash().clone()).collect(),
        ),
    }))
}

async fn create_receipt_proofs_for_logs<N: NetworkSpec, R: ExecutionRpc<N>>(
    logs: &[Log],
    execution_client: Arc<ExecutionClient<N, R>>,
) -> Result<HashMap<B256, TransactionReceiptResponse<N>>> {
    // Collect all (unique) block numbers
    let block_nums = logs
        .iter()
        .map(|log| log.block_number.ok_or_eyre("block_number not found in log"))
        .collect::<Result<HashSet<_>, _>>()?;

    // Collect all tx receipts for all block numbers
    let blocks_receipts_fut = block_nums.into_iter().map(|block_num| {
        let execution_client = Arc::clone(&execution_client);
        async move {
            let block_id = BlockId::from(block_num);
            let receipts = execution_client.rpc.get_block_receipts(block_id).await?;
            receipts
                .ok_or_eyre("No receipts found for the block")
                .map(|receipts| (block_num, receipts))
        }
    });
    let blocks_receipts = try_join_all(blocks_receipts_fut).await?;
    let blocks_receipts = blocks_receipts.into_iter().collect::<HashMap<_, _>>();

    // Create a map of transaction hashes to their receipt and proof
    let mut receipt_proofs: HashMap<B256, TransactionReceiptResponse<N>> = HashMap::new();

    for log in logs {
        let tx_hash = log.transaction_hash.unwrap();
        if receipt_proofs.contains_key(&tx_hash) {
            continue;
        }

        let block_num = log.block_number.unwrap();
        let receipts = blocks_receipts.get(&block_num).unwrap();
        let receipt = receipts
            .get(log.transaction_index.unwrap() as usize)
            .unwrap();

        let receipt_proof = create_receipt_proof::<N>(
            receipts.to_vec(),
            receipt.transaction_index().unwrap() as usize,
        );

        receipt_proofs.insert(
            tx_hash,
            TransactionReceiptResponse {
                receipt: receipt.clone(),
                receipt_proof,
            },
        );
    }

    Ok(receipt_proofs)
}
