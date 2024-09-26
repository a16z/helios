use alloy::primitives::address;
use eyre::Result;
use helios_opstack::OpStackClientBuilder;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[tokio::main]
async fn main() -> Result<()> {
    enable_tracing();

    let chain_id = 8453;
    let signer = address!("Af6E19BE0F9cE7f8afd49a1824851023A8249e8a");
    let server_url = "http://localhost:3000";
    let execution_rpc = "https://base-mainnet.g.alchemy.com/v2/a--NIcyeycPntQX42kunxUIVkg6_ekYc";
    let rpc_ip = "127.0.0.1".parse()?;
    let rpc_port = 8545;

    let mut client = OpStackClientBuilder::new()
        .chain_id(chain_id)
        .unsafe_signer(signer)
        .server_rpc(server_url)
        .execution_rpc(execution_rpc)
        .rpc_bind_ip(rpc_ip)
        .rpc_port(rpc_port)
        .build()?;

    client.start().await?;

    std::future::pending().await
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
