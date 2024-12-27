use mockito::{mock, Server};
use std::time::Duration;

#[tokio::test]
async fn test_retry_mechanism() {
    let mut server = Server::new();
    
    // Test successful retry after a temporary error
    let mock = server.mock("POST", "/")
        .with_status(429) // Rate limit error
        .create();
    
    let provider = HttpProvider::new(server.url().as_str());
    let request = Request::new("eth_blockNumber", ());
    
    let result = provider.execute(request).await;
    assert!(result.is_err());
    mock.assert_hits(1);
    
    // Test the maximum number of retry attempts
    let mock = server.mock("POST", "/")
        .with_status(429)
        .expect(3) // Expect exactly 3 attempts
        .create();
    
    let result = provider.execute(request).await;
    assert!(result.is_err());
    mock.assert();
    
    // Test for errors that should not trigger retries
    let mock = server.mock("POST", "/")
        .with_status(400) // Bad Request
        .expect(1) // Expect only 1 attempt
        .create();
    
    let result = provider.execute(request).await;
    assert!(result.is_err());
    mock.assert();
}
