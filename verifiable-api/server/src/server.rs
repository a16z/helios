use std::net::SocketAddr;

use clap::{Args, Subcommand};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tracing::debug;
use url::Url;

use helios_ethereum::spec::Ethereum as EthereumSpec;
use helios_opstack::spec::OpStack as OpStackSpec;
use helios_verifiable_api_client::VerifiableApi;

use crate::router::build_router;
use crate::service::ApiService;
use crate::state::ApiState;

pub struct VerifiableApiServer {
    network: Network,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl VerifiableApiServer {
    pub fn new(network: Network) -> Self {
        Self {
            network,
            shutdown_tx: None,
        }
    }

    pub fn start(&mut self) -> JoinHandle<()> {
        // Create a shutdown signal channel
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);

        // construct API state and build the router for our server
        let (server_addr, app) = match &self.network {
            Network::Ethereum(args) => {
                let server_addr = args.server_address;
                let execution_rpc = &args.execution_rpc;
                let api_service = ApiService::<EthereumSpec>::new(execution_rpc);
                let router = build_router().with_state(ApiState { api_service });
                (server_addr, router)
            }
            Network::OpStack(args) => {
                let server_addr = args.server_address;
                let execution_rpc = &args.execution_rpc;
                let api_service = ApiService::<OpStackSpec>::new(execution_rpc);
                let router = build_router().with_state(ApiState { api_service });
                (server_addr, router)
            }
        };

        tokio::spawn(async move {
            let listener = TcpListener::bind(server_addr).await.unwrap();

            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap();
        })
    }

    pub fn shutdown(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            debug!(target: "helios::verifiable-api-server", "shutting down");
            shutdown_tx.send(()).unwrap();
        }
    }
}

#[derive(Subcommand)]
pub enum Network {
    #[command(name = "ethereum")]
    Ethereum(ServerArgs),
    #[command(name = "opstack")]
    OpStack(ServerArgs),
}

#[derive(Args)]
pub struct ServerArgs {
    #[arg(short, long, default_value = "127.0.0.1:4000")]
    pub server_address: SocketAddr,
    #[arg(short, long)]
    pub execution_rpc: Url,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_start_and_shutdown() {
        let network = Network::Ethereum(ServerArgs {
            server_address: "127.0.0.1:4000".parse().unwrap(),
            execution_rpc: Url::parse("http://localhost:8545").unwrap(),
        });

        // Start the server in a background task
        let mut server = VerifiableApiServer::new(network);
        let handle = server.start();

        // Shutdown the server
        server.shutdown();

        // Wait for shutdown
        handle.await.unwrap();
    }
}
