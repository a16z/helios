#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{mock, Server};
    use std::time::Duration;
    use crate::execution::rpc::HttpRpc;
    use crate::network_spec::NetworkSpec;

    #[tokio::test]
    #[cfg(target_arch = "wasm32")]
    async fn test_retry_mechanism() {
        let mut server = Server::new();
        
        // Test rate limit retry
        let mock = server.mock("POST", "/")
            .with_status(429)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": "rate limit exceeded"}"#)
            .expect(3)
            .create();

        let provider = HttpRpc::<NetworkSpec>::new(&server.url()).unwrap();
        let result = provider.chain_id().await;
        
        assert!(result.is_err());
        mock.assert();

        // Test non-retryable error
        let mock = server.mock("POST", "/")
            .with_status(400)
            .with_header("content-type", "application/json")
            .with_body(r#"{"error": "bad request"}"#)
            .expect(1)
            .create();

        let result = provider.chain_id().await;
        assert!(result.is_err());
        mock.assert();
    }
}
