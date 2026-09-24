#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

DEVNET_NAME="${DEVNET_NAME:-plataberget}"
CONFIG_BASE="${CONFIG_BASE:-https://config.plataberget.ethpandaops.io}"
CHECKPOINT_SYNC_URL="${CHECKPOINT_SYNC_URL:-https://checkpoint-sync.plataberget.ethpandaops.io}"
BEACON_RPC="${BEACON_RPC:-https://beacon.plataberget.ethpandaops.io}"
CHECKPOINT_SYNC_FALLBACK_URL="${CHECKPOINT_SYNC_FALLBACK_URL:-$BEACON_RPC}"
DIRECT_PEER_ENDPOINT="${DIRECT_PEER_ENDPOINT:-lighthouse}"
DIRECT_PEER="${DIRECT_PEER:-}"
NIMBUS_IMAGE="${NIMBUS_IMAGE:-ethpandaops/nimbus-eth2:unstable}"

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

reset_data_dir() {
    echo "resetting Nimbus data dir: $DATA_DIR"
    rm -rf "$DATA_DIR"
    mkdir -p "$DATA_DIR"
}

trusted_sync() {
    local sync_url="$1"

    echo "running Nimbus trusted sync from $sync_url"
    docker run --rm \
        -v "$CONFIG_DIR:/network-config:ro" \
        -v "$DATA_DIR:/data" \
        "$NIMBUS_IMAGE" \
        trustedNodeSync \
        --network=/network-config \
        --data-dir=/data \
        --trusted-node-url="$sync_url" \
        --backfill="$BACKFILL"
}

require_command curl
require_command docker
require_command jq

mkdir -p "$CONFIG_DIR" "$DATA_DIR"

download "$CONFIG_BASE/cl/config.yaml" "$CONFIG_DIR/config.yaml"
download "$CONFIG_BASE/cl/genesis.ssz" "$CONFIG_DIR/genesis.ssz"
download "$CONFIG_BASE/cl/bootstrap_nodes.txt" "$CONFIG_DIR/bootstrap_nodes.txt"

if [ "$RESET" = "true" ] || [ "$RESET" = "1" ]; then
    reset_data_dir
fi

if is_empty_dir "$DATA_DIR"; then
    if ! trusted_sync "$CHECKPOINT_SYNC_URL"; then
        if [ "$CHECKPOINT_SYNC_URL" = "$CHECKPOINT_SYNC_FALLBACK_URL" ]; then
            exit 1
        fi

        echo "trusted sync failed; retrying from $CHECKPOINT_SYNC_FALLBACK_URL" >&2
        reset_data_dir
        trusted_sync "$CHECKPOINT_SYNC_FALLBACK_URL"
    fi
else
    echo "Nimbus data dir is not empty; skipping trusted sync"
    echo "set RESET=1 to clear it and run trusted sync again"
fi

if [ -z "$DIRECT_PEER" ]; then
    echo "fetching direct peer from $DIRECT_PEER_ENDPOINT via $BEACON_RPC"
    DIRECT_PEER="$(
        curl -fsSL -H "X-Dugtrio-Next-Endpoint: $DIRECT_PEER_ENDPOINT" \
            "$BEACON_RPC/eth/v1/node/identity" \
            | jq -er 'first(.data.p2p_addresses[] | select(startswith("/ip4/") and contains("/tcp/")))'
    )"
fi

echo "starting Nimbus without an execution layer"
echo "beacon API: http://127.0.0.1:$REST_PORT"
echo "direct peer: $DIRECT_PEER"

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
    --direct-peer="$DIRECT_PEER" \
    --rest=true \
    --rest-address=0.0.0.0 \
    --rest-port=5052 \
    --rest-allow-origin='*' \
    --light-client-data-serve=true \
    --light-client-data-import-mode="$LIGHT_CLIENT_IMPORT_MODE" \
    --validator-monitor-auto=false \
    --doppelganger-detection=off
