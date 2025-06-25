use eyre::Result;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

#[derive(Clone, Debug, Default)]
pub struct ProxyMetrics {
    pub bytes_downloaded: Arc<AtomicU64>,
    pub bytes_uploaded: Arc<AtomicU64>,
    pub request_count: Arc<AtomicUsize>,
}

impl ProxyMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&self) {
        self.bytes_downloaded.store(0, Ordering::SeqCst);
        self.bytes_uploaded.store(0, Ordering::SeqCst);
        self.request_count.store(0, Ordering::SeqCst);
    }

    pub fn get_stats(&self) -> (u64, u64, usize) {
        (
            self.bytes_downloaded.load(Ordering::SeqCst),
            self.bytes_uploaded.load(Ordering::SeqCst),
            self.request_count.load(Ordering::SeqCst),
        )
    }
}

pub struct RpcProxy {
    target_url: String,
    metrics: ProxyMetrics,
}

impl RpcProxy {
    pub fn new(target_url: String) -> Self {
        Self {
            target_url,
            metrics: ProxyMetrics::new(),
        }
    }

    pub async fn start(&self, port: u16) -> Result<String> {
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
        let addr = listener.local_addr()?;
        let proxy_url = format!("http://{}", addr);

        let target_url = self.target_url.clone();
        let metrics = self.metrics.clone();

        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let target_url = target_url.clone();
                let metrics = metrics.clone();
                tokio::spawn(handle_connection(stream, target_url, metrics));
            }
        });

        Ok(proxy_url)
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    target_url: String,
    metrics: ProxyMetrics,
) -> Result<()> {
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);
    let mut headers = Vec::new();
    let mut request_line = String::new();

    // Read request line
    reader.read_line(&mut request_line).await?;

    // Read headers
    let mut content_length = None;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).await?;

        if line.trim().is_empty() {
            break;
        }

        if line.to_lowercase().starts_with("content-length:") {
            content_length = line
                .split(':')
                .nth(1)
                .and_then(|v| v.trim().parse::<usize>().ok());
        }
        headers.push(line);
    }

    // Read body
    let mut body = vec![0u8; content_length.unwrap_or(0)];
    if content_length.is_some() {
        reader.read_exact(&mut body).await?;
    }

    // Track upload metrics
    let upload_size =
        request_line.len() + headers.iter().map(|h| h.len()).sum::<usize>() + 2 + body.len();
    metrics
        .bytes_uploaded
        .fetch_add(upload_size as u64, Ordering::SeqCst);
    metrics.request_count.fetch_add(1, Ordering::SeqCst);

    // Connect to target
    let target_host = target_url.trim_start_matches("http://");
    let mut target_stream = TcpStream::connect(target_host).await?;

    // Forward request
    let forward_request = format!(
        "POST / HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        target_host,
        body.len()
    );

    target_stream.write_all(forward_request.as_bytes()).await?;
    target_stream.write_all(&body).await?;
    target_stream.flush().await?;

    // Read response
    let (target_reader, _) = target_stream.split();
    let mut target_reader = BufReader::new(target_reader);
    let mut response_headers = Vec::new();
    let mut response_line = String::new();

    // Read response line
    target_reader.read_line(&mut response_line).await?;
    response_headers.push(response_line.clone());

    // Read response headers
    let mut response_content_length = None;
    loop {
        let mut line = String::new();
        target_reader.read_line(&mut line).await?;

        if line.trim().is_empty() {
            response_headers.push(line);
            break;
        }

        if line.to_lowercase().starts_with("content-length:") {
            response_content_length = line
                .split(':')
                .nth(1)
                .and_then(|v| v.trim().parse::<usize>().ok());
        }
        response_headers.push(line);
    }

    // Read response body
    let mut response_body = Vec::new();
    if let Some(content_length) = response_content_length {
        response_body.resize(content_length, 0);
        target_reader.read_exact(&mut response_body).await?;
    } else {
        // Read until EOF for chunked or no content-length
        target_reader.read_to_end(&mut response_body).await?;
    }

    // Track download metrics - only count body bytes
    metrics
        .bytes_downloaded
        .fetch_add(response_body.len() as u64, Ordering::SeqCst);

    // Send response back to client
    for header in &response_headers {
        writer.write_all(header.as_bytes()).await?;
    }
    writer.write_all(&response_body).await?;
    writer.flush().await?;

    Ok(())
}

pub async fn start_proxy_pair(
    execution_rpc: &str,
    standard_rpc: &str,
) -> Result<(ProxyHandle, ProxyHandle)> {
    let execution_proxy = RpcProxy::new(execution_rpc.to_string());
    let standard_proxy = RpcProxy::new(standard_rpc.to_string());

    let execution_port = 8545;
    let standard_port = 8546;

    let execution_url = execution_proxy.start(execution_port).await?;
    let standard_url = standard_proxy.start(standard_port).await?;

    Ok((
        ProxyHandle {
            url: execution_url,
            metrics: execution_proxy.metrics,
        },
        ProxyHandle {
            url: standard_url,
            metrics: standard_proxy.metrics,
        },
    ))
}

pub struct ProxyHandle {
    pub url: String,
    pub metrics: ProxyMetrics,
}
