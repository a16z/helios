#![allow(deprecated)]

use std::str::FromStr;

use dotenv::dotenv;

use env_logger::Env;
use ethers::prelude::*;
use eyre::Result;
use helios::{
    client::ClientBuilder,
    config::networks::Network,
    types::{BlockTag, CallOpts},
};

// Generate the type-safe contract bindings with an ABI
// abigen!(
//     Renderer,
//     r#"[
//         function renderBroker(uint256, uint256) external view returns (string memory, uint256)
//     ]"#,
//     event_derives(serde::Deserialize, serde::Serialize)
// );

#[tokio::main]
async fn main() -> Result<()> {
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


    // Call the erroneous account method
    // The expected asset is: https://0x8bb9a8baeec177ae55ac410c429cbbbbb9198cac.w3eth.io/renderBroker/5
    // Retrieved by calling `renderBroker(5)` on the contract: https://etherscan.io/address/0x8bb9a8baeec177ae55ac410c429cbbbbb9198cac#code
    // let contract = Renderer::new(account, client.clone());
    let account = "0x8bb9a8baeec177ae55ac410c429cbbbbb9198cac";
    let method = "renderBroker(uint256)";
    log::debug!("Calling {}::{}", account, method);
    let args = vec![ethers::abi::Token::Uint(U256::from(5))];
    log::debug!("Arguments: {:?}", args);
    let function: ethers::abi::Function = ethers::abi::Function {
        name: method.to_string(),
        inputs: vec![ethers::abi::Param {
            name: "brokerId".to_string(),
            kind: ethers::abi::ParamType::Uint(256),
            internal_type: None
        }],
        outputs: vec![ethers::abi::Param {
            name: "brokerName".to_string(),
            kind: ethers::abi::ParamType::String,
            internal_type: Some("memory".to_string())
        }, ethers::abi::Param {name:"brokerId".to_string(),kind:ethers::abi::ParamType::Uint(256), internal_type: None }],
        constant: None,
        state_mutability: ethers::abi::StateMutability::View,
    };
    let encoded = function.encode_input(&args)?;
    log::debug!("Encoded function input: {:?}", encoded);
    let call_opts = CallOpts {
        from: None,
        to: Address::from_str(account)?,
        gas: None,
        gas_price: None,
        value: None,
        data: Some(encoded.clone()),
    };
    log::debug!("Constructed CallOpts: {:?}", call_opts);
    let block = BlockTag::Latest;

    // Call on ethers-rs client
    let mut tx = ethers::types::Eip1559TransactionRequest::new();
    tx = tx.chain_id(1);
    tx = tx.to(account);
    tx = tx.data(encoded.clone());
    let provider = Provider::<Http>::try_from(eth_rpc_url)?;
    log::debug!("[ETHERS] Calling on block: {:?}", block);
    let result = provider.call(&tx.into(), None).await?;
    log::debug!("[ETHERS] {}::{}\n  ->{:?}", account, method, result);

    // Call on helios client
    log::debug!("[HELIOS] Calling on block: {:?}", block);
    let result = client.call(&call_opts, block).await?;
    log::info!("[HELIOS] {}::{}\n  ->{:?}", account, method, result);

    Ok(())
}
