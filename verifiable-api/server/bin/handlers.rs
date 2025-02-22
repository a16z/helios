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
use eyre::{eyre, OptionExt, Report, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::json;

use helios_common::network_spec::NetworkSpec;
use helios_core::execution::{errors::ExecutionError, rpc::ExecutionRpc};
use helios_verifiable_api_client::VerifiableApi;
use helios_verifiable_api_types::*;

use crate::ApiState;

#[allow(type_alias_bounds)]
type Response<T: Serialize + DeserializeOwned> =
    Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

fn json_err(error: &str) -> Json<serde_json::Value> {
    Json(json!({ "error": error }))
}

fn map_server_err(e: Report) -> (StatusCode, Json<serde_json::Value>) {
    if let Some(ExecutionError::BlockNotFound(_)) = e.downcast_ref::<ExecutionError>() {
        (StatusCode::BAD_REQUEST, json_err(&e.to_string()))
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, json_err(&e.to_string()))
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

pub async fn get_account<N: NetworkSpec, R: ExecutionRpc<N>>(
    Path(address): Path<Address>,
    Query(AccountProofQuery {
        include_code,
        storage_slots,
        block,
    }): Query<AccountProofQuery>,
    State(ApiState { api_service }): State<ApiState<N, R>>,
) -> Response<AccountResponse> {
    api_service
        .get_account(address, &storage_slots, block, include_code)
        .await
        .map(Json)
        .map_err(map_server_err)
}

pub async fn get_transaction_receipt<N: NetworkSpec, R: ExecutionRpc<N>>(
    Path(tx_hash): Path<B256>,
    State(ApiState { api_service }): State<ApiState<N, R>>,
) -> Response<Option<TransactionReceiptResponse<N>>> {
    api_service
        .get_transaction_receipt(tx_hash)
        .await
        .map(Json)
        .map_err(map_server_err)
}

pub async fn get_logs<N: NetworkSpec, R: ExecutionRpc<N>>(
    Query(logs_filter_query): Query<LogsQuery>,
    State(ApiState { api_service }): State<ApiState<N, R>>,
) -> Response<LogsResponse<N>> {
    let filter: Filter = logs_filter_query.try_into().map_err(map_server_err)?;

    api_service
        .get_logs(&filter)
        .await
        .map(Json)
        .map_err(map_server_err)
}

pub async fn get_filter_logs<N: NetworkSpec, R: ExecutionRpc<N>>(
    Path(filter_id): Path<U256>,
    State(ApiState { api_service }): State<ApiState<N, R>>,
) -> Response<FilterLogsResponse<N>> {
    api_service
        .get_filter_logs(filter_id)
        .await
        .map(Json)
        .map_err(map_server_err)
}

pub async fn get_filter_changes<N: NetworkSpec, R: ExecutionRpc<N>>(
    Path(filter_id): Path<U256>,
    State(ApiState { api_service }): State<ApiState<N, R>>,
) -> Response<FilterChangesResponse<N>> {
    api_service
        .get_filter_changes(filter_id)
        .await
        .map(Json)
        .map_err(map_server_err)
}

pub async fn create_access_list<N: NetworkSpec, R: ExecutionRpc<N>>(
    State(ApiState { api_service }): State<ApiState<N, R>>,
    Json(AccessListRequest { tx, block }): Json<AccessListRequest<N>>,
) -> Response<AccessListResponse> {
    api_service
        .create_access_list(tx, block)
        .await
        .map(Json)
        .map_err(map_server_err)
}

pub async fn get_chain_id<N: NetworkSpec, R: ExecutionRpc<N>>(
    State(ApiState { api_service }): State<ApiState<N, R>>,
) -> Response<ChainIdResponse> {
    api_service
        .chain_id()
        .await
        .map(Json)
        .map_err(map_server_err)
}

pub async fn get_block<N: NetworkSpec, R: ExecutionRpc<N>>(
    Path(block_id): Path<BlockId>,
    State(ApiState { api_service }): State<ApiState<N, R>>,
) -> Response<Option<N::BlockResponse>> {
    api_service
        .get_block(block_id)
        .await
        .map(Json)
        .map_err(map_server_err)
}

pub async fn get_block_receipts<N: NetworkSpec, R: ExecutionRpc<N>>(
    Path(block_id): Path<BlockId>,
    State(ApiState { api_service }): State<ApiState<N, R>>,
) -> Response<Option<Vec<N::ReceiptResponse>>> {
    api_service
        .get_block_receipts(block_id)
        .await
        .map(Json)
        .map_err(map_server_err)
}

pub async fn send_raw_transaction<N: NetworkSpec, R: ExecutionRpc<N>>(
    State(ApiState { api_service }): State<ApiState<N, R>>,
    Json(SendRawTxRequest { bytes }): Json<SendRawTxRequest>,
) -> Response<SendRawTxResponse> {
    api_service
        .send_raw_transaction(&bytes)
        .await
        .map(Json)
        .map_err(map_server_err)
}

pub async fn new_filter<N: NetworkSpec, R: ExecutionRpc<N>>(
    State(ApiState { api_service }): State<ApiState<N, R>>,
    Json(NewFilterRequest { kind, filter }): Json<NewFilterRequest>,
) -> Response<NewFilterResponse> {
    let res = match kind {
        FilterKind::Logs => {
            let filter = filter
                .ok_or_eyre("Missing filter body")
                .map_err(map_server_err)?;
            api_service.new_filter(&filter).await
        }
        FilterKind::NewBlocks => api_service.new_block_filter().await,
        FilterKind::NewPendingTransactions => api_service.new_pending_transaction_filter().await,
    };

    res.map(Json).map_err(map_server_err)
}

pub async fn uninstall_filter<N: NetworkSpec, R: ExecutionRpc<N>>(
    Path(filter_id): Path<U256>,
    State(ApiState { api_service }): State<ApiState<N, R>>,
) -> Response<UninstallFilterResponse> {
    api_service
        .uninstall_filter(filter_id)
        .await
        .map(Json)
        .map_err(map_server_err)
}
