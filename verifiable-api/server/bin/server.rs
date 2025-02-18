use std::{net::SocketAddr, str::FromStr};

use clap::Parser;
use tracing_subscriber::{EnvFilter, FmtSubscriber};
use url::Url;

use helios_core::execution::rpc::http_rpc::HttpRpc;
use helios_ethereum::spec::Ethereum as EthereumSpec;
use helios_opstack::spec::OpStack as OpStackSpec;

use crate::router::build_router;
use crate::state::ApiState;

mod handlers;
mod router;
mod state;

#[tokio::main]
async fn main() {
    // Pre setup
    enable_tracing();

    // parse CLI arguments
    let cli = Cli::parse();
    let network = cli.network;
    let server_addr = cli.server_address;
    let execution_rpc = cli.execution_rpc;

    // construct API state and build the router for our server
    let app = match network {
        Network::Ethereum => {
            let state =
                ApiState::<EthereumSpec, HttpRpc<EthereumSpec>>::new(&execution_rpc.as_str())
                    .unwrap();
            build_router().with_state(state)
        }
        Network::Opstack => {
            let state = ApiState::<OpStackSpec, HttpRpc<OpStackSpec>>::new(&execution_rpc.as_str())
                .unwrap();
            build_router().with_state(state)
        }
    };

    // run the server
    let listener = tokio::net::TcpListener::bind(server_addr).await.unwrap();

    axum::serve(listener, app).await.unwrap();
}

fn enable_tracing() {
    let env_filter = EnvFilter::builder()
        .with_default_directive("helios_verifiable_api=info".parse().unwrap())
        .from_env()
        .expect("invalid env filter");

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(env_filter)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("subscriber set failed");
}

#[derive(Debug, Parser)]
struct Cli {
    #[clap(short, long, default_value = "127.0.0.1:4000")]
    server_address: SocketAddr,
    #[clap(short, long, default_value = "ethereum")]
    network: Network,
    #[clap(short, long)]
    execution_rpc: Url,
}

#[derive(Debug, Clone)]
enum Network {
    Ethereum,
    Opstack,
}

impl FromStr for Network {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ethereum" => Ok(Network::Ethereum),
            "opstack" => Ok(Network::Opstack),
            _ => Err(format!("'{}' is not a valid network", s)),
        }
    }
}
