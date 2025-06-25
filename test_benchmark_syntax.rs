// Test if our benchmark code has syntax errors
use std::time::{Duration, Instant};
use std::str::FromStr;
use eyre::Result;
use alloy::primitives::{Address, B256, U256};
use alloy::rpc::types::{Filter, TransactionRequest, BlockTransactionsKind};
use alloy::eips::{BlockId, BlockNumberOrTag};
use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::transports::http::{Http, Client};
use helios_ethereum::{EthereumClient, database::FileDB};
use serde_json::Value;

fn main() {
    println!("Types check out!");
}

// Include our modules to check syntax
#[path = "benches/framework/mod.rs"]
mod framework;

#[path = "benches/framework/benchmark.rs"]
mod benchmark_test;

#[path = "benches/framework/proxy.rs"]
mod proxy_test;

#[path = "benches/framework/report.rs"]
mod report_test;