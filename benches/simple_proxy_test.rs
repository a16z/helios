use eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Testing simple RPC request...");
    
    let rpc_url = std::env::var("MAINNET_EXECUTION_RPC")
        .expect("MAINNET_EXECUTION_RPC environment variable must be set");
    
    println!("Testing direct RPC: {}", rpc_url);
    
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;
        
    let response = client.post(&rpc_url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": [],
            "id": 1
        }))
        .send()
        .await?;
    
    let json: serde_json::Value = response.json().await?;
    println!("Response: {:?}", json);
    
    Ok(())
}