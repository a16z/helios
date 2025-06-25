#!/bin/bash

# Test benchmark compilation script

echo "Testing Helios Benchmark Compilation..."

# Set environment variables
export MAINNET_EXECUTION_RPC="http://localhost:8545"
export BENCHMARK_STANDARD_RPC="http://localhost:8545"
export BENCHMARK_RUNS=1

# Try to build the benchmark
echo "Building benchmark..."
cargo build --bench helios_comparison --release 2>&1 | tail -20

# Check if benchmark executable exists
if [ -f "target/release/deps/helios_comparison-"* ]; then
    echo "✓ Benchmark compiled successfully!"
else
    echo "✗ Benchmark compilation failed"
fi