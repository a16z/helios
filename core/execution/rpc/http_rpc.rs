use std::time::Duration;
use wasmtimer::tokio::sleep;

// Adding a structure for retry configuration
#[derive(Clone)]
struct RetryConfig {
    max_attempts: u32,
    initial_backoff: Duration,
    max_backoff: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(5),
        }
    }
}

impl HttpProvider {
    // Adding a new method to perform a request with retries
    async fn execute_with_retry<T>(&self, request: Request<T>) -> Result<Response<T>, Error> 
    where 
        T: serde::Serialize + Send + Sync,
    {
        let config = RetryConfig::default();
        let mut attempts = 0;
        let mut backoff = config.initial_backoff;

        loop {
            attempts += 1;
            match self.execute(request.clone()).await {
                Ok(response) => return Ok(response),
                Err(err) => {
                    // Check if a retry should be attempted
                    if !should_retry(&err) || attempts >= config.max_attempts {
                        return Err(err);
                    }

                    // Wait before the next attempt
                    sleep(backoff).await;
                    
                    // Increase the backoff time exponentially
                    backoff = std::cmp::min(
                        backoff * 2,
                        config.max_backoff
                    );
                }
            }
        }
    }
}

// Helper function to determine if a retry should be attempted
fn should_retry(error: &Error) -> bool {
    matches!(
        error,
        Error::RateLimitExceeded(_) |
        Error::ConnectionError(_) |
        Error::TimeoutError
    )
}

// Updating the existing execute method to use the retry mechanism
impl Provider for HttpProvider {
    async fn execute<T>(&self, request: Request<T>) -> Result<Response<T>, Error>
    where
        T: serde::Serialize + Send + Sync,
    {
        self.execute_with_retry(request).await
    }
}
