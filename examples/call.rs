#![allow(deprecated)]

use std::sync::Arc;
use dotenv::dotenv;
use env_logger::Env;
use ethers::prelude::*;

use helios::{
    client::ClientBuilder,
    config::networks::Network,
    types::{BlockTag, CallOpts},
};

// Generate the type-safe contract bindings with an ABI
abigen!(
    Renderer,
    r#"[
        function renderBroker(uint256) external view returns (string memory)
        function renderBroker(uint256, uint256) external view returns (string memory)
    ]"#,
    event_derives(serde::Deserialize, serde::Serialize)
);

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // Load the .env file
    dotenv().ok();

    // Initialize the logger
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();

    // Load the rpc url using the `ETH_RPC_URL` environment variable
    let eth_rpc_url = std::env::var("ETH_RPC_URL")?;
    let consensus_rpc = "https://www.lightclientdata.org";
    log::info!("Consensus RPC URL: {}", consensus_rpc);

    // Construct the client
    let mut client = ClientBuilder::new()
        .network(Network::MAINNET)
        .consensus_rpc(consensus_rpc)
        .execution_rpc(&eth_rpc_url)
        .load_external_fallback()
        .build()?;
    log::info!("[\"{}\"] Client built with external checkpoint fallbacks", Network::MAINNET);

    // Start the client
    client.start().await?;

    // TODO: Wait until the client is synced
    // client.wait_for_sync().await?;

    // Call the erroneous account method
    // The expected asset is: https://0x8bb9a8baeec177ae55ac410c429cbbbbb9198cac.w3eth.io/renderBroker/5
    // Retrieved by calling `renderBroker(5)` on the contract: https://etherscan.io/address/0x8bb9a8baeec177ae55ac410c429cbbbbb9198cac#code
    let account = "0x8bb9a8baeec177ae55ac410c429cbbbbb9198cac";
    let method = "renderBroker(uint256)";
    let method2 = "renderBroker(uint256, uint256)";
    let argument = U256::from(5);
    let address = account.parse::<Address>()?;
    let block = BlockTag::Latest;
    log::debug!("Context: call @ {account}::{method} <{argument}>");

    // Call using abigen
    let provider = Provider::<Http>::try_from(eth_rpc_url)?;
    let render  = Renderer::new(address, Arc::new(provider.clone()));
    let result = render.render_broker_0(argument).call().await?;
    log::info!("[ABIGEN] {account}::{method} -> Response Length: {:?}", result.len());
    let render  = Renderer::new(address, Arc::new(provider.clone()));
    let result = render.render_broker_1(argument, U256::from(10)).call().await?;
    log::info!("[ABIGEN] {account}::{method2} -> Response Length: {:?}", result.len());

    // Call on helios client
    log::debug!("Calling helios client on block: {block:?}");
    let encoded_call = render.render_broker_0(argument).calldata().unwrap();
    let call_opts = CallOpts {
        from: Some("0xBE0eB53F46cd790Cd13851d5EFf43D12404d33E8".parse::<Address>()?),
        to: address,
        gas: Some(U256::from(U64::MAX.as_u64())),
        gas_price: None,
        value: None,
        data: Some(encoded_call.to_vec()),
    };
    let result = client.call(&call_opts, block).await?;
    log::info!("[HELIOS] {account}::{method}  ->{:?}", result.len());

    Ok(())
}
