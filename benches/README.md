# Helios Benchmarks

This directory contains performance benchmarks for Helios, comparing it against traditional RPC endpoints to measure the trade-offs between trustless verification and performance.

## Overview

The Helios benchmark suite measures three key metrics:

- **Response Time**: How long operations take to complete
- **Data Downloaded**: Amount of data transferred over the network  
- **Request Count**: Number of RPC requests made

All benchmarks validate that Helios returns the same results as a standard RPC endpoint, ensuring correctness while measuring performance differences.

## Running Benchmarks

### Prerequisites

Set your RPC endpoint:
```bash
export MAINNET_EXECUTION_RPC="https://eth-mainnet.g.alchemy.com/v2/YOUR-API-KEY"
```

### Basic Usage

Run the full benchmark suite:
```bash
cargo bench -p helios --bench helios_comparison
```

### Configuration Options

```bash
# Run with custom number of runs (default: 5)
BENCHMARK_RUNS=10 cargo bench -p helios --bench helios_comparison

# Use a different RPC for comparison (defaults to MAINNET_EXECUTION_RPC)
BENCHMARK_STANDARD_RPC="https://your-other-rpc.com" cargo bench -p helios --bench helios_comparison
```

## Benchmark Types

### helios_comparison

The main benchmark suite that compares Helios against a standard RPC across various operations:

- **ETH Balance**: Fetching account balances
- **Contract Calls**: Executing smart contract view functions (e.g., ERC20 balanceOf)
- **Block Fetching**: Retrieving current and historical blocks
- **Receipt Fetching**: Getting transaction receipts for recent and historical transactions
- **Transaction Fetching**: Retrieving transaction details
- **Logs Fetching**: Querying contract event logs

### Other Benchmarks

- `file_db`: Database performance benchmarks
- `get_balance`: Focused benchmarks on balance retrieval
- `get_code`: Contract code retrieval benchmarks
- `sync`: Synchronization performance benchmarks

Run individual benchmarks:
```bash
cargo bench -p helios --bench <benchmark_name>
```

## Understanding the Output

### Example Report

```
╔══════════════════════════════════════════════════════════════════════════════════════════════╗
║                              Helios vs Standard RPC Benchmark Report                         ║
╚══════════════════════════════════════════════════════════════════════════════════════════════╝

┌─────────────────────────────────────┬────────────┬─────────────────┬────────────┬──────────┐
│              Benchmark              │ Time (ms)  │ Data Downloaded │  Requests  │Validated │
├─────────────────────────────────────┼────────────┼─────────────────┼────────────┼──────────┤
│ ETH Balance                         │            │                 │            │          │
│   Helios                            │       50 ms│           7.6K B│           1│    ✓     │
│   RPC                               │       35 ms│             61 B│           1│          │
│   Difference                        │     1.41x ↑│        127.07x ↑│     1.00x →│          │
├─────────────────────────────────────┼────────────┼─────────────────┼────────────┼──────────┤
│ Contract Call (ERC20 balanceOf)     │            │                 │            │          │
│   Helios                            │      161 ms│          99.7K B│           9│    ✓     │
│   RPC                               │       66 ms│            103 B│           1│          │
│   Difference                        │     2.44x ↑│        991.64x ↑│     9.00x ↑│          │
└─────────────────────────────────────┴────────────┴─────────────────┴────────────┴──────────┘

Legend:
  ↑ Higher than RPC (worse)
  ↓ Lower than RPC (better)
  → Similar to RPC (±10%)
```

### Interpreting Results

- **Time**: Shows milliseconds for each operation. Helios is typically 2-10x slower due to cryptographic verification.
- **Data**: Shows bytes downloaded. Helios downloads significantly more data (proofs, headers) for trustless verification.
- **Requests**: Number of RPC calls made. Varies by operation type.
- **Validated**: Confirms that Helios returned the same result as the RPC (✓ = matched).

The "Difference" row shows multiples (e.g., "2.44x") rather than percentages for clearer comparison.

## Performance Expectations

Helios trades performance for trustlessness. Expected characteristics:

1. **Higher Latency**: Cryptographic verification adds overhead
2. **More Data Transfer**: Downloading proofs and headers for verification
3. **Variable Request Count**: Depends on operation complexity

These trade-offs eliminate the need to trust your RPC provider, providing cryptographic guarantees of data correctness.

## Advanced Benchmarking

### Criterion Integration

The benchmarks use [Criterion.rs](https://github.com/bheisler/criterion.rs) for statistical analysis:

```bash
# Run all criterion benchmarks
cargo bench

# Generate HTML reports (in target/criterion/)
cargo bench -- --verbose
```

### Performance Profiling

Generate flamegraphs to identify bottlenecks:

```bash
# Install flamegraph
cargo install flamegraph

# Generate flamegraph for a specific example
cargo flamegraph --example client -o ./benches/flamegraphs/client.svg

# Generate flamegraph for benchmarks
cargo flamegraph --bench helios_comparison -o ./benches/flamegraphs/benchmark.svg
```

### Custom Benchmarks

To add new benchmarks:

1. Create a new benchmark file in `benchmarks/` directory
2. Add the benchmark configuration to `Cargo.toml` with the appropriate path
3. Implement the benchmark using the provided harness and measurement utilities from `lib/`

## Troubleshooting

### Common Issues

1. **"MAINNET_EXECUTION_RPC not set"**: Export the environment variable with your RPC URL
2. **Timeouts**: Some operations may timeout with slow RPC providers. Try a different provider.
3. **Validation Failures**: Ensure you're using a reliable RPC endpoint that returns correct data.

### Debug Mode

For more verbose output during benchmarking:
```bash
RUST_LOG=debug cargo bench -p helios --bench helios_comparison
```

## Architecture

The benchmark framework consists of:

- `lib/proxy/`: HTTP proxy module for measuring RPC traffic
  - `mod.rs`: Main proxy implementation
  - `metrics.rs`: Traffic metrics collection
- `lib/measurement.rs`: Core benchmarking logic and test cases
- `lib/report/`: Report generation module
  - `mod.rs`: Report generation with dynamic table formatting
  - `formatters.rs`: Output formatting utilities
- `lib/harness.rs`: Benchmark harness implementation
- `benchmarks/`: Individual benchmark implementations
  - `helios_comparison.rs`: Main benchmark runner comparing Helios vs standard RPC
  - `file_db.rs`: Database performance benchmarks
  - `get_balance.rs`: Balance retrieval benchmarks
  - `get_code.rs`: Contract code retrieval benchmarks
  - `sync.rs`: Synchronization performance benchmarks

The proxy intercepts all HTTP traffic to measure exact bytes transferred and request counts, while the benchmark framework ensures fair comparison by using the same block heights for both Helios and RPC queries.