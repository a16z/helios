use config::{CliConfig, Config};
use env_logger::Env;
use eyre::Result;
use helios::prelude::*;

#[tokio::test]
async fn feehistory() -> Result<()> {
    //Instantiate Client
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    // Load the config from the global config file
    let config_path = home::home_dir().unwrap().join(".helios/helios.toml");
    let config = Config::from_file(&config_path, "mainnet", &CliConfig::default());

    let mut client: Client<FileDB> = match ClientBuilder::new().config(config).build() {
        Ok(client) => client,
        Err(err) => {
            panic!("Couldn't load the client{}", err);
        }
    };

    if let Err(err) = client.start().await {
        panic!("Couldn't start the client{}", err)
    }

    //Get inputs for fee_history calls
    let head_block_num = client.get_block_number().await?;
    log::info!("head_block_num: {}", &head_block_num);
    let block = BlockTag::Latest;
    let block_number = BlockTag::Number(head_block_num);
    log::info!("block {:?} and block_number {:?}", block, block_number);
    let my_array: Vec<f64> = vec![];

    //below test are lacking access to ExecutionPayload which would give visibility into which blocks we know about

    //test 1 block query from latest
    let fee_history = client
        .get_fee_history(1, head_block_num, &my_array)
        .await?
        .unwrap();
    assert_eq!(fee_history.base_fee_per_gas.len(), 2);
    assert_eq!(fee_history.oldest_block.as_u64(), head_block_num);

    //test 10000 delta, helios will return as many as it can
    let fee_history = match client
        .get_fee_history(10_000, head_block_num, &my_array)
        .await?
    {
        Some(fee_history) => fee_history,
        None => panic!(
            "returned empty gas fee, inputs were the following: 
                       Block amount {:?}, head_block_num {:?}, my_array {:?}",
            10_000, head_block_num, &my_array
        ),
    };
    assert!(
        fee_history.base_fee_per_gas.len() > 0,
        "fee_history.base_fee_per_gas.len() {:?}",
        fee_history.base_fee_per_gas.len()
    );

    //test 10000 block away, Helios will return none
    let fee_history = client
        .get_fee_history(1, head_block_num - 10_000, &my_array)
        .await?;
    assert!(
        fee_history.is_none(),
        "fee_history.is_none() {:?}",
        fee_history.is_none()
    );

    //test 20 block away, should return array of size 21, our 20 block of interest + the next one
    //oldest block should be 19 block away, including it
    let fee_history = client
        .get_fee_history(20, head_block_num, &my_array)
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
        head_block_num - 19,
        "fee_history.oldest_block.as_u64() {:?} vs head_block_num {:?} - 19",
        fee_history.oldest_block.as_u64(),
        head_block_num
    );

    //test whatever blocks ahead, but that will fetch one block behind.
    //This should return an answer of size two as Helios will cap this request to the newest block it knows
    // we refresh parameters to make sure head_block_num is in line with newest block of our payload
    let head_block_num = client.get_block_number().await?;
    let fee_history = client
        .get_fee_history(1, head_block_num + 1000, &my_array)
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
        head_block_num,
        "fee_history.oldest_block.as_u64() {:?} vs head_block_num {:?}",
        fee_history.oldest_block.as_u64(),
        head_block_num
    );

    Ok(())
}
