use axum::{
    routing::{get, post},
    Router,
};
use tower_http::{
    compression::CompressionLayer,
    trace::{DefaultOnRequest, DefaultOnResponse, TraceLayer},
};

use helios_common::network_spec::NetworkSpec;
use tracing::Level;

use crate::{handlers, state::ApiState};

pub fn build_router<N: NetworkSpec>() -> Router<ApiState<N>> {
    Router::new()
        .route("/openapi.yaml", get(handlers::openapi))
        .route("/ping", get(handlers::ping))
        .nest(
            "/eth/v1/proof",
            Router::new()
                .route("/account/{address}", get(handlers::get_account))
                .route("/transaction/{txHash}", get(handlers::get_transaction))
                .route(
                    "/transaction/{blockId}/{index}",
                    get(handlers::get_transaction_by_location),
                )
                .route("/receipt/{txHash}", get(handlers::get_transaction_receipt))
                .route("/logs", get(handlers::get_logs))
                .route("/getExecutionHint", post(handlers::get_execution_hint)),
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
                .route("/sendRawTransaction", post(handlers::send_raw_transaction)),
        )
        .layer(CompressionLayer::new())
        .layer(
            TraceLayer::new_for_http()
                .on_request(DefaultOnRequest::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
}
