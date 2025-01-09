use std::time::Duration;
use wasmtimer::tokio::sleep;

/// Retry mechanism configuration
#[derive(Clone, Debug)]
pub struct RetryConfig {
    /// Maximum number of attempts to execute the request
    pub max_attempts: u32,
    /// Initial delay between retry attempts
    pub initial_backoff: Duration,
    /// Maximum delay between retry attempts
    pub max_backoff: Duration,
}

impl HttpProvider {
    // Add the ability to configure retry settings
    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    async fn execute_with_retry<T>(&self, request: Request<T>) -> Result<Response<T>, Error> 
    where 
        T: serde::Serialize + Send + Sync,
    {
        let mut attempts = 0;
        let mut backoff = self.retry_config.initial_backoff;

        loop {
            attempts += 1;
            match self.execute_internal(request.clone()).await {
                Ok(response) => return Ok(response),
                Err(err) => {
                    if !should_retry(&err) || attempts >= self.retry_config.max_attempts {
                        return Err(err.into());
                    }

                    tracing::debug!(
                        "Request failed with error: {:?}. Retrying ({}/{})",
                        err,
                        attempts,
                        self.retry_config.max_attempts
                    );

                    sleep(backoff).await;
                    
                    backoff = std::cmp::min(
                        backoff * 2,
                        self.retry_config.max_backoff
                    );
                }
            }
        }
    }
}

// Extend the list of errors that can trigger a retry
fn should_retry(error: &Error) -> bool {
    match error {
        Error::RateLimitExceeded(_) => true,
        Error::ConnectionError(_) => true,
        Error::TimeoutError => true,
        Error::ServerError(status) => status.as_u16() >= 500,
        _ => false,
    }
}

// Update the existing execute method to use the retry mechanism
impl Provider for HttpProvider {
    async fn execute<T>(&self, request: Request<T>) -> Result<Response<T>, Error>
    where
        T: serde::Serialize + Send + Sync,
    {
        self.execute_with_retry(request).await
    }
}
