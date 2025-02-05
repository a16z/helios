use std::collections::HashMap;

use alloy::{
    consensus::{Account, BlockHeader},
    eips::BlockNumberOrTag,
    network::{BlockResponse, ReceiptResponse},
    primitives::{Address, B256, U256},
    rpc::types::{BlockId, BlockTransactionsKind},
};
use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use eyre::Result;
use serde::Deserialize;
use serde_json::json;

use helios_core::{execution::rpc::ExecutionRpc, network_spec::NetworkSpec, types::BlockTag};
use helios_verifiable_api::{proof::create_receipt_proof, types::*};

use crate::ApiState;

fn json_err(error: &str) -> Json<serde_json::Value> {
    Json(json!({ "error": error }))
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
) -> Result<Json<GetBalanceResponse>, Json<serde_json::Value>> {
    let block = block.unwrap_or(BlockId::latest());

    let proof = execution_client
        .rpc
        .get_proof(address, &[], block)
        .await
        .map_err(|e| json_err(&e.to_string()))?;

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
) -> Result<Json<GetTransactionCountResponse>, Json<serde_json::Value>> {
    let block = block.unwrap_or(BlockId::latest());

    let proof = execution_client
        .rpc
        .get_proof(address, &[], block)
        .await
        .map_err(|e| json_err(&e.to_string()))?;

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
) -> Result<Json<GetCodeResponse>, Json<serde_json::Value>> {
    // Ensure that BlockId is of block number variant
    let block = block.unwrap_or(BlockId::latest());
    let block_num = match block {
        BlockId::Number(number_or_tag) => match number_or_tag {
            BlockNumberOrTag::Number(number) => Ok(number),
            tag => execution_client
                .rpc
                .get_block_by_number(tag, BlockTransactionsKind::Hashes)
                .await
                .map_err(|e| json_err(&e.to_string()))?
                .ok_or_else(|| json_err("Block not found"))
                .map(|block| block.header().number()),
        },
        BlockId::Hash(hash) => execution_client
            .rpc
            .get_block(hash.into())
            .await
            .map_err(|e| json_err(&e.to_string()))
            .map(|block| block.header().number()),
    }?;
    let block = BlockId::from(block_num);

    let proof = execution_client
        .rpc
        .get_proof(address, &[], block)
        .await
        .map_err(|e| json_err(&e.to_string()))?;

    let code = execution_client
        .rpc
        .get_code(address, block_num)
        .await
        .map_err(|e| json_err(&e.to_string()))?;

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
) -> Result<Json<GetStorageAtResponse>, Json<serde_json::Value>> {
    let block = block.unwrap_or(BlockId::latest());

    let storage_slot = execution_client
        .rpc
        .get_storage_at(address, key, block)
        .await
        .map_err(|e| json_err(&e.to_string()))?;

    let proof = execution_client
        .rpc
        .get_proof(address, &[storage_slot.into()], block)
        .await
        .map_err(|e| json_err(&e.to_string()))?;

    let storage = match proof.storage_proof.get(0) {
        Some(storage) => storage.clone(),
        None => return Err(json_err("Failed to get storage proof")),
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
) -> Result<Json<GetTransactionReceiptResponse<N>>, Json<serde_json::Value>> {
    let receipt = execution_client
        .rpc
        .get_transaction_receipt(tx_hash)
        .await
        .map_err(|e| json_err(&e.to_string()))?
        .ok_or_else(|| json_err("Transaction not found"))?;

    let receipts = execution_client
        .rpc
        .get_block_receipts(BlockTag::Number(receipt.block_number().unwrap()))
        .await
        .map_err(|e| json_err(&e.to_string()))?
        .ok_or_else(|| json_err("No receipts found for the block"))?;

    let receipt_proof =
        create_receipt_proof::<N>(receipts, receipt.transaction_index().unwrap() as usize);

    Ok(Json(GetTransactionReceiptResponse {
        receipt,
        receipt_proof,
    }))
}

/// This method returns an array of all logs matching filter with given id.
/// Corresponding to each log, it also returns the transaction receipt and a Merkle proof of its inclusion.
///
/// Replaces the `eth_getFilterLogs` RPC method.
pub async fn get_filter_logs<N: NetworkSpec, R: ExecutionRpc<N>>(
    Path(filter_id): Path<U256>,
    State(ApiState { execution_client }): State<ApiState<N, R>>,
) -> Result<Json<GetFilterLogsResponse<N>>, Json<serde_json::Value>> {
    let logs = execution_client
        .rpc
        .get_filter_logs(filter_id)
        .await
        .map_err(|e| json_err(&e.to_string()))?;

    let mut receipt_proofs: HashMap<B256, GetTransactionReceiptResponse<N>> = HashMap::new();

    // ToDo(@eshaan7): Optimise this by fetching receipts once per block
    for log in &logs {
        let tx_hash: alloy::primitives::FixedBytes<32> = log.transaction_hash.unwrap();
        if receipt_proofs.contains_key(&tx_hash) {
            continue;
        }

        let receipt_and_proof = get_transaction_receipt(
            Path(tx_hash),
            State(ApiState {
                execution_client: execution_client.clone(),
            }),
        )
        .await?
        .0;

        receipt_proofs.insert(tx_hash, receipt_and_proof);
    }

    Ok(Json(GetFilterLogsResponse {
        logs,
        receipt_proofs,
    }))
}
