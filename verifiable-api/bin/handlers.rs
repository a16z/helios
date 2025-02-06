use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use alloy::{
    consensus::{Account, BlockHeader},
    eips::BlockNumberOrTag,
    network::{BlockResponse, ReceiptResponse},
    primitives::{Address, B256, U256},
    rpc::types::{BlockId, BlockTransactionsKind, Log},
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use eyre::{OptionExt, Report, Result};
use futures::future::try_join_all;
use serde::Deserialize;
use serde_json::json;

use helios_core::{execution::rpc::ExecutionRpc, network_spec::NetworkSpec, types::BlockTag};
use helios_verifiable_api::{proof::create_receipt_proof, rpc_client::ExecutionClient, types::*};

use crate::ApiState;

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

/// This method returns the balance of an account for a given address,
/// along with the Merkle proof of the account's inclusion in the state trie.
///
/// Replaces the `eth_getBalance` RPC method.
pub async fn get_balance<N: NetworkSpec, R: ExecutionRpc<N>>(
    Path(address): Path<Address>,
    Query(BlockQuery { block }): Query<BlockQuery>,
    State(ApiState { execution_client }): State<ApiState<N, R>>,
) -> Result<Json<GetBalanceResponse>, (StatusCode, Json<serde_json::Value>)> {
    let block = block.unwrap_or(BlockId::latest());

    let proof = execution_client
        .rpc
        .get_proof(address, &[], block)
        .await
        .map_err(map_server_err)?;

    Ok(Json(GetBalanceResponse {
        account: Account {
            balance: proof.balance,
            nonce: proof.nonce,
            code_hash: proof.code_hash,
            storage_root: proof.storage_hash,
        },
        account_proof: proof.account_proof,
    }))
}

/// This method returns the number of transactions sent from an address,
/// along with the Merkle proof of the account's inclusion in the state trie.
///
/// Replaces the `eth_getTransactionCount` RPC method.
pub async fn get_transaction_count<N: NetworkSpec, R: ExecutionRpc<N>>(
    Path(address): Path<Address>,
    Query(BlockQuery { block }): Query<BlockQuery>,
    State(ApiState { execution_client }): State<ApiState<N, R>>,
) -> Result<Json<GetTransactionCountResponse>, (StatusCode, Json<serde_json::Value>)> {
    let block = block.unwrap_or(BlockId::latest());

    let proof = execution_client
        .rpc
        .get_proof(address, &[], block)
        .await
        .map_err(map_server_err)?;

    Ok(Json(GetTransactionCountResponse {
        account: Account {
            balance: proof.balance,
            nonce: proof.nonce,
            code_hash: proof.code_hash,
            storage_root: proof.storage_hash,
        },
        account_proof: proof.account_proof,
    }))
}

/// This method returns the code stored at a given address,
/// along with the Merkle proof of the account's inclusion in the state trie.
///
/// Replaces the `eth_getCode` RPC method.
pub async fn get_code<N: NetworkSpec, R: ExecutionRpc<N>>(
    Path(address): Path<Address>,
    Query(BlockQuery { block }): Query<BlockQuery>,
    State(ApiState { execution_client }): State<ApiState<N, R>>,
) -> Result<Json<GetCodeResponse>, (StatusCode, Json<serde_json::Value>)> {
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
        .get_proof(address, &[], block)
        .await
        .map_err(map_server_err)?;

    let code = execution_client
        .rpc
        .get_code(address, block_num)
        .await
        .map_err(map_server_err)?;

    Ok(Json(GetCodeResponse {
        code: code.into(),
        account: Account {
            balance: proof.balance,
            nonce: proof.nonce,
            code_hash: proof.code_hash,
            storage_root: proof.storage_hash,
        },
        account_proof: proof.account_proof,
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
) -> Result<Json<GetStorageAtResponse>, (StatusCode, Json<serde_json::Value>)> {
    let block = block.unwrap_or(BlockId::latest());

    let storage_slot = execution_client
        .rpc
        .get_storage_at(address, key, block)
        .await
        .map_err(map_server_err)?;

    let proof = execution_client
        .rpc
        .get_proof(address, &[storage_slot.into()], block)
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

    Ok(Json(GetStorageAtResponse {
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

/// This method returns the receipt of a transaction along with a Merkle proof of its inclusion.
///
/// Replaces the `eth_getTransactionReceipt` RPC method.
pub async fn get_transaction_receipt<N: NetworkSpec, R: ExecutionRpc<N>>(
    Path(tx_hash): Path<B256>,
    State(ApiState { execution_client }): State<ApiState<N, R>>,
) -> Result<Json<GetTransactionReceiptResponse<N>>, (StatusCode, Json<serde_json::Value>)> {
    let receipt = execution_client
        .rpc
        .get_transaction_receipt(tx_hash)
        .await
        .map_err(map_server_err)?
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                json_err("Transaction not found"),
            )
        })?;

    let receipts = execution_client
        .rpc
        .get_block_receipts(BlockTag::Number(receipt.block_number().unwrap()))
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

    Ok(Json(GetTransactionReceiptResponse {
        receipt,
        receipt_proof,
    }))
}

/// This method returns an array of all logs matching the filter with given id.
/// Corresponding to each log, it also returns the transaction receipt and a Merkle proof of its inclusion.
///
/// Replaces the `eth_getFilterLogs` RPC method.
pub async fn get_filter_logs<N: NetworkSpec, R: ExecutionRpc<N>>(
    Path(filter_id): Path<U256>,
    State(ApiState { execution_client }): State<ApiState<N, R>>,
) -> Result<Json<GetFilterLogsResponse<N>>, (StatusCode, Json<serde_json::Value>)> {
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

    Ok(Json(GetFilterLogsResponse {
        logs,
        receipt_proofs,
    }))
}

async fn create_receipt_proofs_for_logs<N: NetworkSpec, R: ExecutionRpc<N>>(
    logs: &[Log],
    execution_client: Arc<ExecutionClient<N, R>>,
) -> Result<HashMap<B256, GetTransactionReceiptResponse<N>>> {
    // Collect all (unique) block numbers
    let block_nums = logs
        .iter()
        .map(|log| log.block_number.ok_or_eyre("block_number not found in log"))
        .collect::<Result<HashSet<_>, _>>()?;

    // Collect all tx receipts for all block numbers
    let blocks_receipts_fut = block_nums.into_iter().map(|block_num| {
        let execution_client = Arc::clone(&execution_client);
        async move {
            let tag = BlockTag::Number(block_num);
            let receipts = execution_client.rpc.get_block_receipts(tag).await?;
            receipts
                .ok_or_eyre("No receipts found for the block")
                .map(|receipts| (block_num, receipts))
        }
    });
    let blocks_receipts = try_join_all(blocks_receipts_fut).await?;
    let blocks_receipts = blocks_receipts.into_iter().collect::<HashMap<_, _>>();

    // Create a map of transaction hashes to their receipt and proof
    let mut receipt_proofs: HashMap<B256, GetTransactionReceiptResponse<N>> = HashMap::new();

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
            GetTransactionReceiptResponse {
                receipt: receipt.clone(),
                receipt_proof,
            },
        );
    }

    Ok(receipt_proofs)
}
