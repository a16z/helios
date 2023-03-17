use env_logger::Env;
use eyre::Result;
use helios::{config::networks::Network, prelude::*};
use std::time::Duration;
use std::{env, path::PathBuf};

#[tokio::test]
async fn feehistory() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    // Client Configuration
    let api_key = env::var("MAINNET_RPC_URL").expect("MAINNET_RPC_URL env variable missing");
    let checkpoint = "0x4d9b87a319c52e54068b7727a93dd3d52b83f7336ed93707bcdf7b37aefce700";
    let consensus_rpc = "https://www.lightclientdata.org";
    let data_dir = "/tmp/helios";
    log::info!("Using consensus RPC URL: {}", consensus_rpc);

    // Instantiate Client
    let mut client: Client<FileDB> = ClientBuilder::new()
        .network(Network::MAINNET)
        .consensus_rpc(consensus_rpc)
        .execution_rpc(&api_key)
        .checkpoint(checkpoint)
        .load_external_fallback()
        .data_dir(PathBuf::from(data_dir))
        .build()?;

    log::info!(
        "Built client on \"{}\" with external checkpoint fallbacks",
        Network::MAINNET
    );

    client.start().await?;

    // Wait for syncing
    std::thread::sleep(Duration::from_secs(5));

    // Get inputs for fee_history calls
    let head_block_num = client.get_block_number().await?;
    log::info!("head_block_num: {}", &head_block_num);
    let block = BlockTag::Latest;
    let block_number = BlockTag::Number(head_block_num);
    log::info!("block {:?} and block_number {:?}", block, block_number);
    let reward_percentiles: Vec<f64> = vec![];

    // Get fee history for 1 block back from latest
    let fee_history = client
        .get_fee_history(1, head_block_num, &reward_percentiles)
        .await?
        .unwrap();
    assert_eq!(fee_history.base_fee_per_gas.len(), 2);
    assert_eq!(fee_history.oldest_block.as_u64(), head_block_num - 1);

    // Fetch 10000 delta, helios will return as many as it can
    let fee_history = match client
        .get_fee_history(10_000, head_block_num, &reward_percentiles)
        .await?
    {
        Some(fee_history) => fee_history,
        None => panic!(
            "empty gas fee returned with inputs: Block count: {:?}, Head Block #: {:?}, Reward Percentiles: {:?}",
            10_000, head_block_num, &reward_percentiles
        ),
    };
    assert!(
        !fee_history.base_fee_per_gas.is_empty(),
        "fee_history.base_fee_per_gas.len() {:?}",
        fee_history.base_fee_per_gas.len()
    );

    // Fetch 10000 blocks in the past
    // Helios will error since it won't have those historical blocks
    let fee_history = client
        .get_fee_history(1, head_block_num - 10_000, &reward_percentiles)
        .await;
    assert!(fee_history.is_err(), "fee_history() {fee_history:?}");

    // Fetch 20 block away
    // Should return array of size 21: our 20 block of interest + the next one
    // The oldest block should be 19 block away, including it
    let fee_history = client
        .get_fee_history(20, head_block_num, &reward_percentiles)
        .await?
        .unwrap();
    assert_eq!(
        fee_history.base_fee_per_gas.len(),
        21,
        "fee_history.base_fee_per_gas.len() {:?} vs 21",
        fee_history.base_fee_per_gas.len()
    );
    assert_eq!(
        fee_history.oldest_block.as_u64(),
        head_block_num - 20,
        "fee_history.oldest_block.as_u64() {:?} vs head_block_num {:?} - 19",
        fee_history.oldest_block.as_u64(),
        head_block_num
    );

    // Fetch whatever blocks ahead, but that will fetch one block behind.
    // This should return an answer of size two as Helios will cap this request to the newest block it knows
    // we refresh parameters to make sure head_block_num is in line with newest block of our payload
    let head_block_num = client.get_block_number().await?;
    let fee_history = client
        .get_fee_history(1, head_block_num + 1000, &reward_percentiles)
        .await?
        .unwrap();
    assert_eq!(
        fee_history.base_fee_per_gas.len(),
        2,
        "fee_history.base_fee_per_gas.len() {:?} vs 2",
        fee_history.base_fee_per_gas.len()
    );
    assert_eq!(
        fee_history.oldest_block.as_u64(),
        head_block_num - 1,
        "fee_history.oldest_block.as_u64() {:?} vs head_block_num {:?}",
        fee_history.oldest_block.as_u64(),
        head_block_num
    );

    Ok(())
}
