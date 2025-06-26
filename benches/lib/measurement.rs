use crate::lib::proxy::ProxyHandle;
use alloy::dyn_abi::DynSolValue;
use alloy::eips::BlockNumberOrTag;
use alloy::primitives::{Address, Bytes};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::rpc::types::{Filter, TransactionRequest};
use eyre::Result;
use helios_ethereum::EthereumClient;
use serde_json::Value;
use std::str::FromStr;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq)]
pub struct BenchmarkResult {
    pub name: String,
    pub helios_time: Duration,
    pub rpc_time: Duration,
    pub helios_bytes: u64,
    pub rpc_bytes: u64,
    pub helios_requests: usize,
    pub rpc_requests: usize,
    pub validated: bool,
}

impl BenchmarkResult {
    pub fn time_diff_multiple(&self) -> f64 {
        self.helios_time.as_secs_f64() / self.rpc_time.as_secs_f64()
    }

    pub fn bytes_diff_multiple(&self) -> f64 {
        if self.rpc_bytes == 0 {
            0.0
        } else {
            self.helios_bytes as f64 / self.rpc_bytes as f64
        }
    }

    pub fn requests_diff_multiple(&self) -> f64 {
        if self.rpc_requests == 0 {
            0.0
        } else {
            self.helios_requests as f64 / self.rpc_requests as f64
        }
    }
}

pub struct Benchmark {
    helios_client: EthereumClient,
    standard_proxy: ProxyHandle,
    execution_proxy: ProxyHandle,
    standard_provider: Box<dyn Provider>,
    runs: usize,
}

impl Benchmark {
    pub async fn new(
        helios_client: EthereumClient,
        execution_proxy: ProxyHandle,
        standard_proxy: ProxyHandle,
        runs: usize,
    ) -> Result<Self> {
        let url = standard_proxy.url.parse()?;
        #[allow(deprecated)]
        let provider = ProviderBuilder::new().on_http(url);
        let standard_provider = Box::new(provider) as Box<dyn Provider>;

        Ok(Self {
            helios_client,
            standard_proxy,
            execution_proxy,
            standard_provider,
            runs,
        })
    }

    pub async fn run_case(&mut self, case: BenchmarkCase) -> Result<BenchmarkResult> {
        let mut helios_times = Vec::new();
        let mut rpc_times = Vec::new();
        let mut helios_bytes = Vec::new();
        let mut rpc_bytes = Vec::new();
        let mut helios_requests = Vec::new();
        let mut rpc_requests = Vec::new();
        let mut all_validated = true;

        for _ in 0..self.runs {
            let (helios_time, rpc_time, validated) = match &case {
                BenchmarkCase::EthBalance { address } => {
                    self.benchmark_eth_balance(address).await?
                }
                BenchmarkCase::ContractCall {
                    contract,
                    method_sig,
                    args,
                } => {
                    self.benchmark_contract_call(contract, method_sig, args)
                        .await?
                }
                BenchmarkCase::BlockFetch { historical } => {
                    self.benchmark_block_fetch(*historical).await?
                }
                BenchmarkCase::ReceiptFetch { historical } => {
                    self.benchmark_receipt_fetch(*historical).await?
                }
                BenchmarkCase::TransactionFetch { historical } => {
                    self.benchmark_transaction_fetch(*historical).await?
                }
                BenchmarkCase::LogsFetch { filter } => self.benchmark_logs_fetch(filter).await?,
            };

            helios_times.push(helios_time);
            rpc_times.push(rpc_time);
            all_validated = all_validated && validated;

            let (exec_down, _, exec_req) = self.execution_proxy.metrics.get_stats();
            let (std_down, _, std_req) = self.standard_proxy.metrics.get_stats();

            helios_bytes.push(exec_down);
            rpc_bytes.push(std_down);
            helios_requests.push(exec_req);
            rpc_requests.push(std_req);
        }

        let avg_helios_time = Duration::from_secs_f64(
            helios_times.iter().map(|d| d.as_secs_f64()).sum::<f64>() / self.runs as f64,
        );
        let avg_rpc_time = Duration::from_secs_f64(
            rpc_times.iter().map(|d| d.as_secs_f64()).sum::<f64>() / self.runs as f64,
        );
        let avg_helios_bytes = helios_bytes.iter().sum::<u64>() / self.runs as u64;
        let avg_rpc_bytes = rpc_bytes.iter().sum::<u64>() / self.runs as u64;
        let avg_helios_requests = helios_requests.iter().sum::<usize>() / self.runs;
        let avg_rpc_requests = rpc_requests.iter().sum::<usize>() / self.runs;

        Ok(BenchmarkResult {
            name: case.name(),
            helios_time: avg_helios_time,
            rpc_time: avg_rpc_time,
            helios_bytes: avg_helios_bytes,
            rpc_bytes: avg_rpc_bytes,
            helios_requests: avg_helios_requests,
            rpc_requests: avg_rpc_requests,
            validated: all_validated,
        })
    }

    async fn benchmark_eth_balance(&self, address: &Address) -> Result<(Duration, Duration, bool)> {
        // Get the latest block number from Helios first
        let helios_block_number = self.helios_client.get_block_number().await?;
        let block_id = BlockNumberOrTag::Number(helios_block_number.to::<u64>()).into();

        // Reset metrics for this benchmark
        self.execution_proxy.metrics.reset();
        self.standard_proxy.metrics.reset();

        // Benchmark Helios
        let helios_start = Instant::now();
        let helios_balance = self.helios_client.get_balance(*address, block_id).await?;
        let helios_time = helios_start.elapsed();

        // Benchmark RPC using the same block number with alloy
        let rpc_start = Instant::now();
        let rpc_balance = self
            .standard_provider
            .get_balance(*address)
            .block_id(block_id)
            .await?;
        let rpc_time = rpc_start.elapsed();

        let validated = helios_balance == rpc_balance;
        if !validated {
            eprintln!(
                "Balance mismatch: Helios {} vs RPC {}",
                helios_balance, rpc_balance
            );
        }

        Ok((helios_time, rpc_time, validated))
    }

    async fn benchmark_contract_call(
        &self,
        contract: &Address,
        method_sig: &str,
        args: &Value,
    ) -> Result<(Duration, Duration, bool)> {
        // Get the latest block from Helios
        let helios_block_number = self.helios_client.get_block_number().await?;
        let block_id = BlockNumberOrTag::Number(helios_block_number.to::<u64>()).into();

        // Reset metrics for this benchmark
        self.execution_proxy.metrics.reset();
        self.standard_proxy.metrics.reset();

        // Prepare the call data
        let method_id = &alloy::primitives::keccak256(method_sig.as_bytes())[..4];
        let address_arg = args
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_str())
            .ok_or_else(|| eyre::eyre!("Invalid address argument"))?;

        let address = Address::from_str(address_arg)?;
        let encoded_args = DynSolValue::Address(address).abi_encode();

        let mut call_data = method_id.to_vec();
        call_data.extend_from_slice(&encoded_args);

        let tx_request = TransactionRequest::default()
            .to(*contract)
            .input(Bytes::from(call_data).into());

        // Benchmark Helios
        let helios_start = Instant::now();
        let helios_result = self.helios_client.call(&tx_request, block_id).await?;
        let helios_time = helios_start.elapsed();

        // Benchmark RPC with alloy
        let rpc_start = Instant::now();
        let rpc_result = self
            .standard_provider
            .call(tx_request.clone())
            .block(block_id)
            .await?;
        let rpc_time = rpc_start.elapsed();

        let validated = helios_result == rpc_result;

        Ok((helios_time, rpc_time, validated))
    }

    async fn benchmark_block_fetch(&self, historical: bool) -> Result<(Duration, Duration, bool)> {
        let block_number = if historical {
            let current = self.helios_client.get_block_number().await?;
            current.to::<u64>().saturating_sub(5000)
        } else {
            self.helios_client.get_block_number().await?.to::<u64>()
        };

        let block_id = BlockNumberOrTag::Number(block_number).into();

        // Reset metrics for this benchmark
        self.execution_proxy.metrics.reset();
        self.standard_proxy.metrics.reset();

        // Benchmark Helios
        let helios_start = Instant::now();
        let helios_block = self.helios_client.get_block(block_id, false).await?;
        let helios_time = helios_start.elapsed();

        // Benchmark RPC with alloy
        let rpc_start = Instant::now();
        let rpc_block = self.standard_provider.get_block(block_id).await?;
        let rpc_time = rpc_start.elapsed();

        // Basic validation
        let validated = helios_block.is_some() == rpc_block.is_some();
        if !validated {
            eprintln!(
                "Block fetch mismatch: Helios returned {:?}, RPC returned {:?}",
                helios_block.is_some(),
                rpc_block.is_some()
            );
        }

        Ok((helios_time, rpc_time, validated))
    }

    async fn benchmark_receipt_fetch(
        &self,
        historical: bool,
    ) -> Result<(Duration, Duration, bool)> {
        // Get a transaction hash from a block
        let mut tx_hash = None;
        for i in 0..20 {
            let block_number = if historical {
                let current = self.helios_client.get_block_number().await?;
                current.to::<u64>().saturating_sub(5000 + i)
            } else {
                let helios_block_number = self.helios_client.get_block_number().await?;
                helios_block_number.to::<u64>().saturating_sub(i)
            };

            let block_id = BlockNumberOrTag::Number(block_number).into();

            if let Some(block) = self.helios_client.get_block(block_id, true).await? {
                // Check if block has transactions
                if block.transactions.len() > 0 {
                    // Get first transaction hash
                    tx_hash = block
                        .transactions
                        .hashes()
                        .collect::<Vec<_>>()
                        .into_iter()
                        .next();
                    if tx_hash.is_some() {
                        break;
                    }
                }
            }
        }

        let tx_hash =
            tx_hash.ok_or_else(|| eyre::eyre!("No transactions found in recent blocks"))?;

        // Reset metrics for this benchmark after finding the transaction
        self.execution_proxy.metrics.reset();
        self.standard_proxy.metrics.reset();

        // Benchmark Helios
        let helios_start = Instant::now();
        let helios_receipt = self.helios_client.get_transaction_receipt(tx_hash).await?;
        let helios_time = helios_start.elapsed();

        // Benchmark RPC with alloy
        let rpc_start = Instant::now();
        let rpc_receipt = self
            .standard_provider
            .get_transaction_receipt(tx_hash)
            .await?;
        let rpc_time = rpc_start.elapsed();

        // Validate
        let validated = helios_receipt.is_some() == rpc_receipt.is_some();
        if !validated {
            eprintln!("Receipt fetch mismatch");
        }

        Ok((helios_time, rpc_time, validated))
    }

    async fn benchmark_transaction_fetch(
        &self,
        historical: bool,
    ) -> Result<(Duration, Duration, bool)> {
        // Get a transaction hash from a block
        let mut tx_hash = None;
        for i in 0..20 {
            let block_number = if historical {
                let current = self.helios_client.get_block_number().await?;
                current.to::<u64>().saturating_sub(5000 + i)
            } else {
                let helios_block_number = self.helios_client.get_block_number().await?;
                helios_block_number.to::<u64>().saturating_sub(i)
            };

            let block_id = BlockNumberOrTag::Number(block_number).into();

            if let Some(block) = self.helios_client.get_block(block_id, true).await? {
                // Check if block has transactions
                if block.transactions.len() > 0 {
                    // Get first transaction hash
                    tx_hash = block
                        .transactions
                        .hashes()
                        .collect::<Vec<_>>()
                        .into_iter()
                        .next();
                    if tx_hash.is_some() {
                        break;
                    }
                }
            }
        }

        let tx_hash =
            tx_hash.ok_or_else(|| eyre::eyre!("No transactions found in recent blocks"))?;

        // Reset metrics for this benchmark after finding the transaction
        self.execution_proxy.metrics.reset();
        self.standard_proxy.metrics.reset();

        // Benchmark Helios
        let helios_start = Instant::now();
        let helios_tx = self.helios_client.get_transaction(tx_hash).await?;
        let helios_time = helios_start.elapsed();

        // Benchmark RPC with alloy
        let rpc_start = Instant::now();
        let rpc_tx = self
            .standard_provider
            .get_transaction_by_hash(tx_hash)
            .await?;
        let rpc_time = rpc_start.elapsed();

        // Validate
        let validated = helios_tx.is_some() == rpc_tx.is_some();
        if !validated {
            eprintln!("Transaction fetch mismatch");
        }

        Ok((helios_time, rpc_time, validated))
    }

    async fn benchmark_logs_fetch(&self, filter: &Value) -> Result<(Duration, Duration, bool)> {
        // Get the latest block from Helios
        let helios_block_number = self.helios_client.get_block_number().await?;
        let block_tag = BlockNumberOrTag::Number(helios_block_number.to::<u64>());

        // Reset metrics for this benchmark
        self.execution_proxy.metrics.reset();
        self.standard_proxy.metrics.reset();

        let address = filter
            .get("address")
            .and_then(|v| v.as_str())
            .and_then(|s| Address::from_str(s).ok());

        let filter = Filter::new().from_block(block_tag).to_block(block_tag);

        let filter = if let Some(addr) = address {
            filter.address(addr)
        } else {
            filter
        };

        // Benchmark Helios
        let helios_start = Instant::now();
        let helios_logs = self.helios_client.get_logs(&filter).await?;
        let helios_time = helios_start.elapsed();

        // Benchmark RPC with alloy
        let rpc_start = Instant::now();
        let rpc_logs = self.standard_provider.get_logs(&filter).await?;
        let rpc_time = rpc_start.elapsed();

        // Basic validation - check count
        let validated = helios_logs.len() == rpc_logs.len();
        if !validated {
            eprintln!(
                "Logs count mismatch: Helios {} vs RPC {}",
                helios_logs.len(),
                rpc_logs.len()
            );
        }

        Ok((helios_time, rpc_time, validated))
    }
}

#[derive(Debug, Clone)]
pub enum BenchmarkCase {
    EthBalance {
        address: Address,
    },
    ContractCall {
        contract: Address,
        method_sig: String,
        args: Value,
    },
    BlockFetch {
        historical: bool,
    },
    ReceiptFetch {
        historical: bool,
    },
    TransactionFetch {
        historical: bool,
    },
    LogsFetch {
        filter: Value,
    },
}

impl BenchmarkCase {
    pub fn name(&self) -> String {
        match self {
            Self::EthBalance { .. } => "ETH Balance".to_string(),
            Self::ContractCall { .. } => "Contract Call (ERC20 balanceOf)".to_string(),
            Self::BlockFetch { historical } => {
                if *historical {
                    "Block Fetch (Historical)".to_string()
                } else {
                    "Block Fetch".to_string()
                }
            }
            Self::ReceiptFetch { historical } => {
                if *historical {
                    "Receipt Fetch (Historical)".to_string()
                } else {
                    "Receipt Fetch".to_string()
                }
            }
            Self::TransactionFetch { historical } => {
                if *historical {
                    "Transaction Fetch (Historical)".to_string()
                } else {
                    "Transaction Fetch".to_string()
                }
            }
            Self::LogsFetch { .. } => "Logs Fetch".to_string(),
        }
    }
}
