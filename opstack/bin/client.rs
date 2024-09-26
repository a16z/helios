use alloy::primitives::address;
use eyre::Result;
use helios_core::client::Client;
use helios_opstack::{consensus::ConsensusClient, spec::Optimism};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[tokio::main]
async fn main() -> Result<()> {
    enable_tracing();

    let server_url = "http://localhost:3000";
    let signer = address!("Af6E19BE0F9cE7f8afd49a1824851023A8249e8a");
    let chain_id = 8453;

    let execution_rpc = "https://base-mainnet.g.alchemy.com/v2/a--NIcyeycPntQX42kunxUIVkg6_ekYc";
    let consensus = ConsensusClient::new(&server_url, signer, chain_id);
    let rpc_address = Some("127.0.0.1:8545".parse()?);

    let mut client = Client::<Optimism, _>::new(execution_rpc, consensus, rpc_address)?;
    client.start().await?;

    let fut = std::future::pending();
    fut.await
}

fn enable_tracing() {
    let env_filter = EnvFilter::builder()
        .with_default_directive("helios_opstack=info".parse().unwrap())
        .from_env()
        .expect("invalid env filter");

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(env_filter)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("subscriber set failed");
}
