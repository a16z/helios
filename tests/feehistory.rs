use std::{path::PathBuf};
use env_logger::Env;
use eyre::Result;
use helios::{config::networks::Network, prelude::*};

#[tokio::test]
async fn feehistory() -> Result<()> {
    //Instantiate Client
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let untrusted_rpc_url = "https://eth-mainnet.g.alchemy.com/v2/XXXXX";
    log::info!("Using untrusted RPC URL [REDACTED]");

    let consensus_rpc = "https://www.lightclientdata.org";
    log::info!("Using consensus RPC URL: {}", consensus_rpc);

    let checkpoint = "0x4bcc641667c22564124e84b270f005f00925ca51a840548904a624bcb22306d0";

    let mut client: Client<FileDB> = ClientBuilder::new()
        .network(Network::MAINNET)
        .consensus_rpc(consensus_rpc)
        .execution_rpc(untrusted_rpc_url)
        .checkpoint(checkpoint)
        .load_external_fallback()
        .data_dir(PathBuf::from("/tmp/helios"))
        .build()?;

    log::info!(
        "Built client on network \"{}\" with external checkpoint fallbacks",
        Network::MAINNET
    );

    client.start().await?;

    //Get inputs for fee history calls
    let head_block_num = client.get_block_number().await?;
    log::info!("head_block_num: {}", &head_block_num);
    let block = BlockTag::Latest;
    let block_number = BlockTag::Number(head_block_num);
    log::info!(
        "block {:?} and block_number {:?}",
        block, block_number
    );
    let my_array: Vec<f64> = vec![];
    
    //below test are lacking access to ExecutionPayload which would give visibility into which blocks we know about
    
    //test one block query from latest
    let fee_history = client.get_fee_history(1, head_block_num, &my_array).await?.unwrap();
    assert_eq!(fee_history.base_fee_per_gas.len(), 2);
    assert_eq!(fee_history.oldest_block.as_u64(), head_block_num);

    //test 10000 delta, will return as much as we can
    let fee_history = client.get_fee_history(10_000, head_block_num, &my_array).await?.unwrap();
    assert!(fee_history.base_fee_per_gas.len() > 0, "fee_history.base_fee_per_gas.len() {:?}", fee_history.base_fee_per_gas.len());

    //test 10000 block away, will return none 
    let fee_history = client.get_fee_history(1, head_block_num - 10_000, &my_array).await?;
    assert!(fee_history.is_none(), "fee_history.is_none() {:?}", fee_history.is_none());
    

    //test 20 block away, should return array of size 21, our 20 block of interest + the next one 
    //oldest block should be 19 block away, including it
    let fee_history = client.get_fee_history(20, head_block_num, &my_array).await?.unwrap();
    assert_eq!(fee_history.base_fee_per_gas.len(), 21, "fee_history.base_fee_per_gas.len() {:?} vs 21", fee_history.base_fee_per_gas.len());
    assert_eq!(fee_history.oldest_block.as_u64(), head_block_num - 19,
              "fee_history.oldest_block.as_u64() {:?} vs head_block_num {:?} - 19",
               fee_history.oldest_block.as_u64(), head_block_num);

    //test whatever blocks ahead, but that will fetch one block behind. 
    //This should returns an answer of size two as we'll cap this request to newest block we know
    // we refresh paramaters to make sure head_block_num is in line with newest block of our payload
    let head_block_num = client.get_block_number().await?;
    let fee_history = client.get_fee_history(1, head_block_num+1000, &my_array).await?.unwrap();
    assert_eq!(fee_history.base_fee_per_gas.len(), 2, "fee_history.base_fee_per_gas.len() {:?} vs 2", fee_history.base_fee_per_gas.len());
    assert_eq!(fee_history.oldest_block.as_u64(), head_block_num,
              "fee_history.oldest_block.as_u64() {:?} vs head_block_num {:?}",
              fee_history.oldest_block.as_u64(), head_block_num);

    Ok(())
}


