use axum::{
    routing::{get, post},
    Router,
};

use helios_common::network_spec::NetworkSpec;
use helios_core::execution::rpc::ExecutionRpc;

use crate::{handlers, ApiState};

pub fn build_router<N: NetworkSpec, R: ExecutionRpc<N>>() -> Router<ApiState<N, R>> {
    Router::new().nest(
        "/eth/v1/proof",
        Router::new()
            .route("/account/{address}", get(handlers::get_account))
            .route("/storage/{address}/{slot}", get(handlers::get_storage_at))
            .route("/block_receipts/{block}", get(handlers::get_block_receipts))
            .route(
                "/tx_receipt/{tx_hash}",
                get(handlers::get_transaction_receipt),
            )
            .route("/logs", get(handlers::get_logs))
            .route("/filter_logs/{filter_id}", get(handlers::get_filter_logs))
            .route(
                "/filter_changes/{filter_id}",
                get(handlers::get_filter_changes),
            )
            .route("/create_access_list", post(handlers::create_access_list)),
    )
}
