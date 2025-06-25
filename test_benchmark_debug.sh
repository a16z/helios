#!/bin/bash
set -x

export MAINNET_EXECUTION_RPC="http://ec2-52-91-168-149.compute-1.amazonaws.com:8545"
export BENCHMARK_RUNS=1
export RUST_LOG=debug

echo "Starting benchmark debug test..."

timeout 30 cargo test -p helios test_proxy --bin 2>&1 || echo "Test timed out or failed"