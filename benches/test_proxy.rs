use eyre::Result;

mod framework;
use framework::proxy;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Testing proxy setup...");
    
    let execution_rpc = std::env::var("MAINNET_EXECUTION_RPC")
        .expect("MAINNET_EXECUTION_RPC environment variable must be set");
    
    println!("Starting proxy services...");
    let (execution_proxy, standard_proxy) = proxy::start_proxy_pair(&execution_rpc, &execution_rpc).await?;
    println!("  Execution proxy: {}", execution_proxy.url);
    println!("  Standard proxy: {}", standard_proxy.url);
    
    // Test making a simple request through proxy
    println!("\nTesting proxy with simple eth_blockNumber request...");
    
    let client = reqwest::Client::new();
    let response = client.post(&standard_proxy.url)
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
    
    let (down, up, reqs) = standard_proxy.metrics.get_stats();
    println!("\nProxy metrics:");
    println!("  Bytes downloaded: {}", down);
    println!("  Bytes uploaded: {}", up);
    println!("  Request count: {}", reqs);
    
    Ok(())
}