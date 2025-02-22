use axum::{
    routing::{delete, get, post},
    Router,
};

use helios_common::network_spec::NetworkSpec;
use helios_core::execution::rpc::ExecutionRpc;

use crate::{handlers, ApiState};

// ToDo(@eshaan7): prob use kebab-case for routes since that's the convention in REST APIs
pub fn build_router<N: NetworkSpec, R: ExecutionRpc<N>>() -> Router<ApiState<N, R>> {
    Router::new()
        .nest(
            "/eth/v1/proof",
            Router::new()
                .route("/account/{address}", get(handlers::get_account))
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
        .nest(
            "/eth/v1",
            Router::new()
                .route("/chain_id", get(handlers::get_chain_id))
                .route("/block/{block_id}", get(handlers::get_block))
                .route(
                    "/block/{block_id}/receipts",
                    get(handlers::get_block_receipts),
                )
                .route(
                    "/send_raw_transaction",
                    post(handlers::send_raw_transaction),
                )
                .route("/filter", post(handlers::new_filter))
                .route("/filter/{filter_id}", delete(handlers::uninstall_filter)),
        )
}
