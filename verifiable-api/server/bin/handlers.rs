use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use alloy::{
    consensus::{Account, BlockHeader},
    eips::BlockNumberOrTag,
    network::{BlockResponse, ReceiptResponse, TransactionBuilder},
    primitives::{Address, B256, U256},
    rpc::types::{
        AccessListItem, BlockId, BlockTransactionsKind, Filter, FilterChanges, FilterSet, Log,
    },
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use axum_extra::extract::Query;
use eyre::{eyre, OptionExt, Report, Result};
use futures::future::{join_all, try_join_all};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;

use helios_common::network_spec::NetworkSpec;
use helios_core::execution::{
    constants::PARALLEL_QUERY_BATCH_SIZE, proof::create_receipt_proof, rpc::ExecutionRpc,
};
use helios_verifiable_api_types::*;

use crate::ApiState;

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
#[serde(rename_all = "camelCase")]
pub struct AccountProofQuery {
    #[serde(default)]
    pub storage_slots: Vec<U256>,
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
    pub topic0: Vec<B256>,
    #[serde(default)]
    pub topic1: Vec<B256>,
    #[serde(default)]
    pub topic2: Vec<B256>,
    #[serde(default)]
    pub topic3: Vec<B256>,
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
        if !self.topic0.is_empty() {
            filter = filter.event_signature(FilterSet::from(self.topic0));
        }
        if !self.topic1.is_empty() {
            filter = filter.topic1(FilterSet::from(self.topic1));
        }
        if !self.topic2.is_empty() {
            filter = filter.topic2(FilterSet::from(self.topic2));
        }
        if !self.topic3.is_empty() {
            filter = filter.topic3(FilterSet::from(self.topic3));
        }

        Ok(filter)
    }
}

/// This method returns the EIP-1186 account proof for a given address.
///
/// Replaces the `eth_getProof`, `eth_getTransactionCount`,
/// `eth_getBalance` `eth_getCode` and `eth_getStorageAt`RPC methods.
pub async fn get_account<N: NetworkSpec, R: ExecutionRpc<N>>(
    Path(address): Path<Address>,
    Query(AccountProofQuery {
        storage_slots,
        block,
    }): Query<AccountProofQuery>,
    State(ApiState { rpc, .. }): State<ApiState<N, R>>,
) -> Response<AccountResponse> {
    // Ensure that BlockId is of block number variant
    let block = block.unwrap_or(BlockId::latest());
    let block_num = match block {
        BlockId::Number(number_or_tag) => match number_or_tag {
            BlockNumberOrTag::Number(number) => Ok(number),
            tag => rpc
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
        BlockId::Hash(hash) => rpc
            .get_block(hash.into())
            .await
            .map_err(map_server_err)
            .map(|block| block.header().number()),
    }?;
    let block = BlockId::from(block_num);

    let storage_keys = storage_slots
        .into_iter()
        .map(|key| key.into())
        .collect::<Vec<_>>();
    let proof = rpc
        .get_proof(address, &storage_keys, block)
        .await
        .map_err(map_server_err)?;

    let code = rpc
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

/// This method returns the receipt of a transaction along with a Merkle proof of its inclusion.
///
/// Replaces the `eth_getTransactionReceipt` RPC method.
pub async fn get_transaction_receipt<N: NetworkSpec, R: ExecutionRpc<N>>(
    Path(tx_hash): Path<B256>,
    State(ApiState { rpc, .. }): State<ApiState<N, R>>,
) -> Response<Option<TransactionReceiptResponse<N>>> {
    let receipt = rpc
        .get_transaction_receipt(tx_hash)
        .await
        .map_err(map_server_err)?;
    if receipt.is_none() {
        // ToDo(@eshaan7): Maybe throw a 404 instead?
        return Ok(Json(None));
    }
    let receipt = receipt.unwrap();

    let receipts = rpc
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
    State(ApiState { rpc, .. }): State<ApiState<N, R>>,
) -> Response<LogsResponse<N>> {
    let filter: Filter = logs_filter_query.try_into().map_err(map_server_err)?;

    // Fetch the filter logs from RPC
    let logs = rpc.get_logs(&filter).await.map_err(map_server_err)?;

    // Create receipt proofs for each log
    let receipt_proofs = create_receipt_proofs_for_logs(&logs, rpc)
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
    State(ApiState { rpc, .. }): State<ApiState<N, R>>,
) -> Response<FilterLogsResponse<N>> {
    // Fetch the filter logs from RPC
    let logs = rpc
        .get_filter_logs(filter_id)
        .await
        .map_err(map_server_err)?;

    // Create receipt proofs for each log
    let receipt_proofs = create_receipt_proofs_for_logs(&logs, rpc)
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
    State(ApiState { rpc, .. }): State<ApiState<N, R>>,
) -> Response<FilterChangesResponse<N>> {
    let filter_changes = rpc
        .get_filter_changes(filter_id)
        .await
        .map_err(map_server_err)?;

    Ok(Json(match filter_changes {
        FilterChanges::Logs(logs) => {
            // Create receipt proofs for each log
            let receipt_proofs = create_receipt_proofs_for_logs(&logs, rpc)
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

/// This method returns a list of all accounts (along with their proofs) that are accessed by a given transaction.
/// The list includes the `from`, `to` and `beneficiary` addresses as well.
///
/// Replaces the `eth_createAccessList` RPC method.
pub async fn create_access_list<N: NetworkSpec, R: ExecutionRpc<N>>(
    State(ApiState { rpc, .. }): State<ApiState<N, R>>,
    Json(AccessListRequest { tx, block }): Json<AccessListRequest<N>>,
) -> Response<AccessListResponse> {
    let block_id = block.unwrap_or(BlockId::latest());
    let block = match block_id {
        BlockId::Number(number_or_tag) => rpc
            .get_block_by_number(number_or_tag, BlockTransactionsKind::Hashes)
            .await
            .map_err(map_server_err)?
            .ok_or_else(|| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json_err("Block not found"),
                )
            }),
        BlockId::Hash(hash) => rpc.get_block(hash.into()).await.map_err(map_server_err),
    }?;
    let block_id = BlockId::Number(block.header().number().into());

    let mut list = rpc
        .create_access_list(&tx, block_id)
        .await
        .map_err(map_server_err)?
        .0;

    let from_access_entry = AccessListItem {
        address: tx.from().unwrap_or_default(),
        storage_keys: Vec::default(),
    };
    let to_access_entry = AccessListItem {
        address: tx.to().unwrap_or_default(),
        storage_keys: Vec::default(),
    };
    let producer_access_entry = AccessListItem {
        address: block.header().beneficiary(),
        storage_keys: Vec::default(),
    };

    let list_addresses = list.iter().map(|elem| elem.address).collect::<Vec<_>>();

    if !list_addresses.contains(&from_access_entry.address) {
        list.push(from_access_entry)
    }
    if !list_addresses.contains(&to_access_entry.address) {
        list.push(to_access_entry)
    }
    if !list_addresses.contains(&producer_access_entry.address) {
        list.push(producer_access_entry)
    }

    let mut accounts = HashMap::new();
    for chunk in list.chunks(PARALLEL_QUERY_BATCH_SIZE) {
        let account_chunk_futs = chunk.iter().map(|account| async {
            let account_fut = get_account(
                Path(account.address),
                Query(AccountProofQuery {
                    storage_slots: account
                        .storage_keys
                        .iter()
                        .map(|key| (*key).into())
                        .collect(),
                    block: Some(block_id),
                }),
                State(ApiState::new_from_rpc(rpc.clone())),
            );
            (account.address, account_fut.await)
        });

        let account_chunk = join_all(account_chunk_futs).await;

        account_chunk.into_iter().for_each(|(address, value)| {
            if let Some(account) = value.ok() {
                accounts.insert(address, account.0);
            }
        });
    }

    Ok(Json(AccessListResponse { accounts }))
}

async fn create_receipt_proofs_for_logs<N: NetworkSpec, R: ExecutionRpc<N>>(
    logs: &[Log],
    rpc: Arc<R>,
) -> Result<HashMap<B256, TransactionReceiptResponse<N>>> {
    // Collect all (unique) block numbers
    // ToDo(@eshaan7): Might need to put a max limit on the number of blocks
    // otherwise this could make too many requests and slow down the server
    let block_nums = logs
        .iter()
        .map(|log| log.block_number.ok_or_eyre("block_number not found in log"))
        .collect::<Result<HashSet<_>, _>>()?;

    // Collect all tx receipts for all block numbers
    let blocks_receipts_fut = block_nums.into_iter().map(|block_num| {
        let rpc = Arc::clone(&rpc);
        async move {
            let block_id = BlockId::from(block_num);
            let receipts = rpc.get_block_receipts(block_id).await?;
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
