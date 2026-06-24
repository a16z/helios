#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

DEVNET_NAME="${DEVNET_NAME:-glamsterdam-devnet-5}"
CONFIG_BASE="${CONFIG_BASE:-https://config.glamsterdam-devnet-5.ethpandaops.io}"
CHECKPOINT_SYNC_URL="${CHECKPOINT_SYNC_URL:-https://checkpoint-sync.glamsterdam-devnet-5.ethpandaops.io}"
NIMBUS_IMAGE="${NIMBUS_IMAGE:-ethpandaops/nimbus-eth2:glamsterdam-devnet-5}"

WORK_DIR="${HELIOS_GLAMSTERDAM_DIR:-$REPO_ROOT/.devnets/$DEVNET_NAME/nimbus-no-el}"
CONFIG_DIR="$WORK_DIR/network-config"
DATA_DIR="$WORK_DIR/nimbus-data"

REST_PORT="${REST_PORT:-5052}"
P2P_PORT="${P2P_PORT:-9000}"
LIGHT_CLIENT_IMPORT_MODE="${LIGHT_CLIENT_IMPORT_MODE:-full}"
BACKFILL="${BACKFILL:-false}"
RESET="${RESET:-false}"

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "missing required command: $1" >&2
        exit 1
    fi
}

download() {
    local url="$1"
    local out="$2"

    echo "downloading $url"
    curl -fsSL --retry 3 --retry-delay 2 "$url" -o "$out"
}

is_empty_dir() {
    [ -d "$1" ] && [ -z "$(find "$1" -mindepth 1 -maxdepth 1 -print -quit)" ]
}

require_command curl
require_command docker

mkdir -p "$CONFIG_DIR" "$DATA_DIR"

download "$CONFIG_BASE/cl/config.yaml" "$CONFIG_DIR/config.yaml"
download "$CONFIG_BASE/cl/genesis.ssz" "$CONFIG_DIR/genesis.ssz"
download "$CONFIG_BASE/cl/bootstrap_nodes.txt" "$CONFIG_DIR/bootstrap_nodes.txt"

if [ "$RESET" = "true" ] || [ "$RESET" = "1" ]; then
    echo "resetting Nimbus data dir: $DATA_DIR"
    rm -rf "$DATA_DIR"
    mkdir -p "$DATA_DIR"
fi

if is_empty_dir "$DATA_DIR"; then
    echo "running Nimbus trusted sync from $CHECKPOINT_SYNC_URL"
    docker run --rm \
        -v "$CONFIG_DIR:/network-config:ro" \
        -v "$DATA_DIR:/data" \
        "$NIMBUS_IMAGE" \
        trustedNodeSync \
        --network=/network-config \
        --data-dir=/data \
        --trusted-node-url="$CHECKPOINT_SYNC_URL" \
        --backfill="$BACKFILL"
else
    echo "Nimbus data dir is not empty; skipping trusted sync"
    echo "set RESET=1 to clear it and run trusted sync again"
fi

echo "starting Nimbus without an execution layer"
echo "beacon API: http://127.0.0.1:$REST_PORT"

exec docker run --rm \
    -p "127.0.0.1:$REST_PORT:5052" \
    -p "$P2P_PORT:9000" \
    -p "$P2P_PORT:9000/udp" \
    -v "$CONFIG_DIR:/network-config:ro" \
    -v "$DATA_DIR:/data" \
    "$NIMBUS_IMAGE" \
    --network=/network-config \
    --data-dir=/data \
    --no-el \
    --bootstrap-file=/network-config/bootstrap_nodes.txt \
    --rest=true \
    --rest-address=0.0.0.0 \
    --rest-port=5052 \
    --rest-allow-origin='*' \
    --light-client-data-serve=true \
    --light-client-data-import-mode="$LIGHT_CLIENT_IMPORT_MODE" \
    --validator-monitor-auto=false \
    --doppelganger-detection=off
