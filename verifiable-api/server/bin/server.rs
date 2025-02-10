use std::{net::SocketAddr, str::FromStr, sync::Arc};

use clap::Parser;
use helios_core::execution::rpc::http_rpc::HttpRpc;
use router::build_router;
use tracing_subscriber::{EnvFilter, FmtSubscriber};
use url::Url;

use helios_ethereum::spec::Ethereum as EthereumSpec;
// use helios_opstack::spec::OpStack as OpStackSpec;

use crate::state::{ApiState, ExecutionClient};

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

    // construct execution RPC client
    let rpc = match network {
        Network::Ethereum => {
            ExecutionClient::<EthereumSpec, HttpRpc<EthereumSpec>>::new(&execution_rpc.as_str())
                .unwrap()
        }
        Network::Opstack => {
            // ToDo(@eshaan7): make this generic work
            unimplemented!()
            // ExecutionClient::<OpStackSpec, HttpRpc<OpStackSpec>>::new(&execution_rpc.as_str())
            //     .unwrap()
        }
    };

    // build the router for our server
    let state = ApiState {
        execution_client: Arc::new(rpc),
    };
    let app = build_router().with_state(state);

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
