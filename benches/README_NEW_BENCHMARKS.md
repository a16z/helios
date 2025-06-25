# Helios Benchmarking Suite

This is a comprehensive benchmarking framework for comparing Helios performance against traditional RPC endpoints.

## Overview

The benchmarking suite measures three key metrics:
1. **Time**: How long requests take to complete
2. **Bandwidth**: Amount of data downloaded
3. **Request Count**: Number of RPC requests made

## Running the Benchmarks

### Prerequisites

Set the following environment variables:
```bash
export MAINNET_EXECUTION_RPC="http://your-mainnet-rpc-url"
export BENCHMARK_STANDARD_RPC="http://your-comparison-rpc-url"  # Optional, defaults to MAINNET_EXECUTION_RPC
export BENCHMARK_RUNS=10  # Optional, defaults to 5
```

### Execute Benchmarks

```bash
cargo bench --bench helios_comparison
```

## Benchmark Cases

The suite includes the following test cases:

1. **ETH Balance**: Fetching ETH balance for an address
2. **Contract Call**: ERC20 balanceOf call (USDC)
3. **Block Fetch**: Retrieving block data (current)
4. **Block Fetch (Historical)**: Retrieving block data from 5000 blocks ago
5. **Receipt Fetch**: Getting transaction receipts (current)
6. **Receipt Fetch (Historical)**: Getting historical transaction receipts
7. **Transaction Fetch**: Retrieving transaction data (current)
8. **Transaction Fetch (Historical)**: Retrieving historical transaction data
9. **Logs Fetch**: Fetching event logs

## Architecture

### Proxy Service
The framework uses a transparent proxy service to measure:
- Bytes uploaded/downloaded
- Number of requests made
- This allows accurate measurement without modifying Helios or RPC client code

### Benchmark Framework
- Configurable number of runs per test
- Averages results across multiple runs
- Handles both current and historical data requests
- Generates comprehensive comparison reports

### Report Generation
The report shows:
- Side-by-side comparison of Helios vs RPC
- Percentage differences for each metric
- Visual indicators (↑↓→) for performance comparison
- Summary statistics

## Implementation Details

The benchmarking framework consists of:

1. **`framework/proxy.rs`**: HTTP proxy for measuring RPC traffic
2. **`framework/benchmark.rs`**: Core benchmarking logic and test cases
3. **`framework/report.rs`**: Terminal report generation
4. **`helios_comparison.rs`**: Main benchmark executable

## Example Output

```
╔══════════════════════════════════════════════════════════════════════════════════════════════╗
║                              Helios vs Standard RPC Benchmark Report                         ║
╚══════════════════════════════════════════════════════════════════════════════════════════════╝

┌─────────────────────────────┬───────────────────────┬───────────────────────┬───────────────────────┐
│ Benchmark                   │ Time                  │ Data Downloaded       │ Requests Made         │
├─────────────────────────────┼───────────────────────┼───────────────────────┼───────────────────────┤
│ ETH Balance                 │                       │                       │                       │
│   Helios                    │             125.5 ms │             1024 B │                     3 │
│   RPC                       │              45.2 ms │              256 B │                     1 │
│   Difference                │            177.7% ↑ │            300.0% ↑ │               200.0% ↑ │
└─────────────────────────────┴───────────────────────┴───────────────────────┴───────────────────────┘
```

## Notes

- Historical benchmarks fetch data from blocks that Helios hasn't seen before (5000 blocks old)
- This tests the different performance characteristics of historical vs current data access
- The proxy introduces minimal overhead that affects both Helios and RPC equally