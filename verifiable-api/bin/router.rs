use axum::{routing::get, Router};

use helios_core::{execution::rpc::ExecutionRpc, network_spec::NetworkSpec};

use crate::{handlers, ApiState};

pub fn build_router<N: NetworkSpec, R: ExecutionRpc<N>>() -> Router<ApiState<N, R>> {
    Router::new().nest(
        "/eth/v1/proof",
        Router::new()
            .route("/balance/{address}", get(handlers::get_balance))
            .route(
                "/transaction_count/{address}",
                get(handlers::get_transaction_count),
            )
            .route("/code/{address}", get(handlers::get_code))
            .route("/storage/{address}/{slot}", get(handlers::get_storage_at))
            .route(
                "/tx_receipt/{tx_hash}",
                get(handlers::get_transaction_receipt),
            )
            .route("/filter_logs/{filter_id}", get(handlers::get_filter_logs)),
    )
}
