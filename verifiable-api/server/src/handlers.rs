use std::{fs::File, io::Read};

use alloy::{
    eips::BlockNumberOrTag,
    primitives::{Address, B256, U256},
    rpc::types::{BlockId, Filter, FilterSet},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use axum_extra::extract::Query;
use eyre::{eyre, Report, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use helios_common::network_spec::NetworkSpec;
use helios_core::execution::errors::ExecutionError;
use helios_verifiable_api_client::VerifiableApi;
use helios_verifiable_api_types::*;

use crate::state::ApiState;

#[allow(type_alias_bounds)]
type Response<T: Serialize + DeserializeOwned> = Result<Json<T>, (StatusCode, Json<ErrorResponse>)>;

fn map_server_err(e: Report) -> (StatusCode, Json<ErrorResponse>) {
    let json_err = Json(ErrorResponse {
        error: e.to_string(),
    });
    if let Some(ExecutionError::BlockNotFound(_)) = e.downcast_ref::<ExecutionError>() {
        (StatusCode::BAD_REQUEST, json_err)
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, json_err)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountProofQuery {
    #[serde(default)]
    include_code: bool,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockQuery {
    #[serde(default)]
    pub transaction_detail_flag: bool,
}

impl TryInto<Filter> for LogsQuery {
    type Error = Report;

    fn try_into(self) -> Result<Filter, Self::Error> {
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

/// Returns information about an address along with its EIP-1186 account proof.
///
/// ## Path Parameters
///
/// - `address` - The address of the account.
///
/// ## Query Parameters
///
/// - `includeCode` - A flag indicating whether to include the account's code.
/// - `storageSlots` - A list of storage positions (in hex) to include in the proof.
/// - `block` - The block number, tag or hash to query the account state at.
///
/// ## Why is this useful?
///
/// Replaces the `eth_getProof`, `eth_getTransactionCount`,
/// `eth_getBalance`, `eth_getCode`, and `eth_getStorageAt` RPC methods.
///
/// ## How to verify response?
///
/// - RLP encode the `TrieAccount` struct and keccak-256 hash it.
/// - Verify the given `accountProof` against the trusted block's state root using the address as the key (path) and the hashed account as the value (leaf).
/// - For each item in `storageProof`, verify the given leaf’s Merkle Proof against the `storageHash`
pub async fn get_account<N: NetworkSpec>(
    Path(address): Path<Address>,
    Query(AccountProofQuery {
        include_code,
        storage_slots,
        block,
    }): Query<AccountProofQuery>,
    State(ApiState { api_service }): State<ApiState<N>>,
) -> Response<AccountResponse> {
    api_service
        .get_account(address, &storage_slots, block, include_code)
        .await
        .map(Json)
        .map_err(map_server_err)
}

/// Returns the receipt of a transaction along with a Merkle Proof of its inclusion.
///
/// ## Path Parameters
///
/// - `txHash` - The hash of the transaction.
///
/// ## Why is this useful?
///
/// Replaces the `eth_getTransactionReceipt` RPC method.
///
/// ## How to verify response?
///
/// - RLP encode the given receipt and keccak-256 hash it.
/// - Verify the given `receiptProof` against the trusted block's receipt root with the given receipt's hash as the leaf.
pub async fn get_transaction_receipt<N: NetworkSpec>(
    Path(tx_hash): Path<B256>,
    State(ApiState { api_service }): State<ApiState<N>>,
) -> Response<Option<TransactionReceiptResponse<N>>> {
    api_service
        .get_transaction_receipt(tx_hash)
        .await
        .map(Json)
        .map_err(map_server_err)
}

/// Returns the transaction along with a Merkle Proof of its inclusion.
///
/// ## Path Parameters
///
/// - `txHash` - The hash of the transaction.
///
/// ## Why is this useful?
///
/// Replaces the `eth_getTransaction` RPC method.
///
/// ## How to verify response?
///
/// - RLP encode the given transaction and keccak-256 hash it.
/// - Verify the given `transactionProof` against the trusted block's transaction root with the
/// transaction hash as the leaf.
pub async fn get_transaction<N: NetworkSpec>(
    Path(tx_hash): Path<B256>,
    State(ApiState { api_service }): State<ApiState<N>>,
) -> Response<Option<TransactionResponse<N>>> {
    api_service
        .get_transaction(tx_hash)
        .await
        .map(Json)
        .map_err(map_server_err)
}

/// Returns the transaction along with a Merkle Proof of its inclusion.
///
/// ## Path Parameters
///
/// - `blockId` - The block of the transaction
/// - `index` - The index of the transaction within the block
///
/// ## Why is this useful?
///
/// Replaces the `eth_getTransactionByBlockNumberAndIndex` and
/// `eth_getTransactionByBlockHashAndIndex` RPC methods.
///
/// ## How to verify response?
///
/// - RLP encode the given transaction and keccak-256 hash it.
/// - Verify the given `transactionProof` against the trusted block's transaction root with the
/// transaction hash as the leaf.
pub async fn get_transaction_by_location<N: NetworkSpec>(
    Path(block_id): Path<BlockId>,
    Path(index): Path<u64>,
    State(ApiState { api_service }): State<ApiState<N>>,
) -> Response<Option<TransactionResponse<N>>> {
    api_service
        .get_transaction_by_location(block_id, index)
        .await
        .map(Json)
        .map_err(map_server_err)
}

/// Returns an array of all logs matching the given filter object.
/// Corresponding to each log, it also returns the transaction receipt and a Merkle Proof of its inclusion.
///
/// ## Query Parameters
///
/// - `fromBlock`: range — starting block number or tag.
/// - `toBlock`: range — ending block number or tag.
/// - `blockHash`: Using blockHash is equivalent to fromBlock = toBlock = the block number with hash blockHash. If blockHash is present in the filter criteria, then neither fromBlock nor toBlock are allowed.
/// - `address`: Contract address or a list of addresses from which logs should originate.
/// - `topic0`: Array of 32 Bytes DATA topics.
/// - `topic1`: Array of 32 Bytes DATA topics.
/// - `topic2`: Array of 32 Bytes DATA topics.
/// - `topic3`: Array of 32 Bytes DATA topics.
/// > Note: Topics are order-dependent. Each topic can also be an array of DATA with "or" options.
///
/// ## Why is this useful?
///
/// Replaces the `eth_getLogs` RPC method.
///
/// ## How to verify response?
///
/// For each log,
/// - Find the corresponding transaction receipt for the log from the `receiptProofs` field. Let’s call this `receipt`.
/// - Ensure that this log entry is included in the `receipt.logs` array.
/// - RLP encode the `receipt` and keccak-256 hash it.
/// - Verify the given `receiptProof` against the trusted block's receipt root with the given receipt's hash as the leaf.
pub async fn get_logs<N: NetworkSpec>(
    Query(logs_filter_query): Query<LogsQuery>,
    State(ApiState { api_service }): State<ApiState<N>>,
) -> Result<axum::response::Response, (StatusCode, Json<ErrorResponse>)> {
    let filter: Filter = logs_filter_query.try_into().map_err(map_server_err)?;

    api_service
        .get_logs(&filter)
        .await
        .map_err(map_server_err)
        .map(json_response)
}

/// Returns a list of all addresses and storage keys (along with their EIP-1186 proofs) that are accessed by a given transaction.
/// It's an extended list because it includes the `from`, `to` and `block.beneficiary` addresses as well.
///
/// ## Path Parameters
///
/// - `tx` - The transaction call object to create the access list for.
/// - `validateTx` - A flag indicating whether to validate the transaction (such as enforcing gas limit).
/// - `block` - The block number, tag or hash to simulate the transaction at.
///
/// ## Why is this useful?
///
/// Replaces the `eth_createAccessList` RPC method.
///
/// ## How to verify response?
///
/// For each account:
/// - RLP encode the `TrieAccount` struct and keccak-256 hash it.
/// - Verify the given `accountProof` against the trusted block's state root using the address as the key (path) and the hashed account as the value (leaf).
/// - For each item in `storageProof`: verify the given leaf’s Merkle Proof against the `storageHash`.
pub async fn get_execution_hint<N: NetworkSpec>(
    State(ApiState { api_service }): State<ApiState<N>>,
    Json(ExtendedAccessListRequest {
        tx,
        validate_tx,
        block,
    }): Json<ExtendedAccessListRequest<N>>,
) -> Response<ExtendedAccessListResponse> {
    api_service
        .get_execution_hint(tx, validate_tx, block)
        .await
        .map(Json)
        .map_err(map_server_err)
}

/// Returns the chain id of the network of the underlying RPC node.
///
/// ## Why is this useful?
///
/// Checking that we are connected to the correct network
pub async fn get_chain_id<N: NetworkSpec>(
    State(ApiState { api_service }): State<ApiState<N>>,
) -> Response<ChainIdResponse> {
    api_service
        .chain_id()
        .await
        .map(Json)
        .map_err(map_server_err)
}

/// Returns information about a block by block number, tag or hash.
///
/// ## Path Parameters
///
/// - `blockId` - The block number, tag, or hash.
///
/// ## Query Parameters
///
/// - `transactionDetailFlag` - A flag indicating whether to include full transaction details or just the hashes.
///
/// ## Why is this useful?
///
/// Replaces the `eth_getBlockByNumber` and `eth_getBlockByHash` RPC methods.
pub async fn get_block<N: NetworkSpec>(
    Path(block_id): Path<BlockId>,
    Query(BlockQuery {
        transaction_detail_flag,
    }): Query<BlockQuery>,
    State(ApiState { api_service }): State<ApiState<N>>,
) -> Response<Option<N::BlockResponse>> {
    api_service
        .get_block(block_id, transaction_detail_flag)
        .await
        .map(Json)
        .map_err(map_server_err)
}

/// Returns all transaction receipts for a given block.
///
/// ## Path Parameters
///
/// - `blockId` - The block number, tag, or hash.
///
/// ## Why is this useful?
///
/// Replaces the `eth_getBlockReceipts` RPC method.
///
/// ## How to verify response?
///
/// - RLP encode each receipt and keccak-256 hash these encoded receipts.
/// - Construct a Merkle Patricia Trie (MPT) from these hashes.
/// - Verify the root of the constructed MPT against the trusted block's receipt root.
pub async fn get_block_receipts<N: NetworkSpec>(
    Path(block_id): Path<BlockId>,
    State(ApiState { api_service }): State<ApiState<N>>,
) -> Response<Option<Vec<N::ReceiptResponse>>> {
    api_service
        .get_block_receipts(block_id)
        .await
        .map(Json)
        .map_err(map_server_err)
}

/// Creates a new message call transaction or a contract creation for signed transactions.
///
/// ## Path Parameters
///
/// - `bytes` - Bytes of the signed transaction data.
///
/// ## Why is this useful?
///
/// Replaces the `eth_sendRawTransaction` RPC method.
pub async fn send_raw_transaction<N: NetworkSpec>(
    State(ApiState { api_service }): State<ApiState<N>>,
    Json(SendRawTxRequest { bytes }): Json<SendRawTxRequest>,
) -> Response<SendRawTxResponse> {
    api_service
        .send_raw_transaction(&bytes)
        .await
        .map(Json)
        .map_err(map_server_err)
}

/// Returns the OpenAPI specification in YAML format.
pub async fn openapi() -> Result<String, (StatusCode, String)> {
    let mut file = match File::open("openapi.yaml") {
        Ok(file) => file,
        Err(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to open openapi.yaml".to_string(),
            ));
        }
    };

    let mut contents = String::new();
    if file.read_to_string(&mut contents).is_err() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to read openapi.yaml".to_string(),
        ));
    }

    Ok(contents)
}

pub async fn ping() -> Result<String, (StatusCode, String)> {
    Ok("pong".to_string())
}

fn json_response<T: Serialize>(val: T) -> axum::response::Response {
    let body = serde_json::to_string(&val).unwrap();
    let len = body.len().to_string();

    axum::response::Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .header(axum::http::header::CONTENT_LENGTH, len)
        .body(body.into())
        .unwrap()
}
