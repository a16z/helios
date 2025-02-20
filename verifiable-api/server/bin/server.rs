use std::net::SocketAddr;

use api_service::ApiService;
use clap::{Args, Parser, Subcommand};
use tracing::{debug, level_filters::LevelFilter};
use tracing_subscriber::{EnvFilter, FmtSubscriber};
use url::Url;

use helios_core::execution::rpc::http_rpc::HttpRpc;
use helios_ethereum::spec::Ethereum as EthereumSpec;
use helios_opstack::spec::OpStack as OpStackSpec;

use crate::router::build_router;
use crate::state::ApiState;

mod api_service;
mod handlers;
mod router;
mod state;

#[tokio::main]
async fn main() {
    // Pre setup
    enable_tracing();

    // parse CLI arguments
    let cli = Cli::parse();

    // construct API state and build the router for our server
    let (server_addr, app) = match cli.command {
        Command::Ethereum(args) => {
            let server_addr = args.server_address;
            let execution_rpc = args.execution_rpc;
            let api_service =
                ApiService::<EthereumSpec, HttpRpc<EthereumSpec>>::new(&execution_rpc.as_str())
                    .unwrap();
            let router = build_router().with_state(ApiState { api_service });
            (server_addr, router)
        }
        Command::OpStack(args) => {
            let server_addr = args.server_address;
            let execution_rpc = args.execution_rpc;
            let api_service =
                ApiService::<OpStackSpec, HttpRpc<OpStackSpec>>::new(&execution_rpc.as_str())
                    .unwrap();
            let router = build_router().with_state(ApiState { api_service });
            (server_addr, router)
        }
    };

    // run the server
    let listener = tokio::net::TcpListener::bind(server_addr).await.unwrap();
    debug!(
        target: "helios::verifiable-api-server",
        "listening on {}",
        listener.local_addr().unwrap()
    );

    axum::serve(listener, app).await.unwrap();
}

fn enable_tracing() {
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()
        .expect("invalid env filter");

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(env_filter)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("subscriber set failed");
}

#[derive(Parser)]
#[clap(version, about)]
/// Helios' Verifiable API server
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    #[clap(name = "ethereum")]
    Ethereum(CliArgs),
    #[clap(name = "opstack")]
    OpStack(CliArgs),
}

#[derive(Args)]
struct CliArgs {
    #[clap(short, long, default_value = "127.0.0.1:4000")]
    server_address: SocketAddr,
    #[clap(short, long)]
    execution_rpc: Url,
}
