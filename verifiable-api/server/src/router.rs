use axum::{
    routing::{delete, get, post},
    Router,
};

use helios_common::network_spec::NetworkSpec;
use helios_core::execution::rpc::ExecutionRpc;

use crate::{handlers, state::ApiState};

pub fn build_router<N: NetworkSpec, R: ExecutionRpc<N>>() -> Router<ApiState<N, R>> {
    Router::new()
        .nest(
            "/eth/v1/proof",
            Router::new()
                .route("/account/{address}", get(handlers::get_account))
                .route(
                    "/transaction/{txHash}/receipt",
                    get(handlers::get_transaction_receipt),
                )
                .route("/logs", get(handlers::get_logs))
                .route("/filterLogs/{filterId}", get(handlers::get_filter_logs))
                .route(
                    "/filterChanges/{filterId}",
                    get(handlers::get_filter_changes),
                )
                .route("/createAccessList", post(handlers::create_access_list)),
        )
        .nest(
            "/eth/v1",
            Router::new()
                .route("/chainId", get(handlers::get_chain_id))
                .route("/block/{blockId}", get(handlers::get_block))
                .route(
                    "/block/{blockId}/receipts",
                    get(handlers::get_block_receipts),
                )
                .route("/sendRawTransaction", post(handlers::send_raw_transaction))
                .route("/filter", post(handlers::new_filter))
                .route("/filter/{filterId}", delete(handlers::uninstall_filter)),
        )
}
