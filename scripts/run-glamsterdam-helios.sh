#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

NETWORK="${NETWORK:-plataberget}"
CONSENSUS_RPC="${CONSENSUS_RPC:-https://beacon.plataberget.ethpandaops.io/nimbus}"
EXECUTION_RPC="${EXECUTION_RPC:-https://rpc.plataberget.ethpandaops.io}"
RPC_BIND_IP="${RPC_BIND_IP:-127.0.0.1}"
RPC_PORT="${RPC_PORT:-8545}"
DATA_DIR="${HELIOS_GLAMSTERDAM_DATA_DIR:-$REPO_ROOT/.devnets/$NETWORK/helios-data}"

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "missing required command: $1" >&2
        exit 1
    fi
}

require_command cargo
require_command curl
require_command jq

mkdir -p "$DATA_DIR"

if [ -z "${CHECKPOINT:-}" ]; then
    echo "fetching latest light-client checkpoint from $CONSENSUS_RPC"
    FINALIZED_SLOT="$(
        curl -fsSL "$CONSENSUS_RPC/eth/v1/beacon/light_client/finality_update" \
            | jq -er '.data.finalized_header.beacon.slot'
    )"
    CHECKPOINT="$(
        curl -fsSL "$CONSENSUS_RPC/eth/v1/beacon/headers/$FINALIZED_SLOT" \
            | jq -er '.data.root'
    )"
fi

echo "network:       $NETWORK"
echo "consensus rpc: $CONSENSUS_RPC"
echo "execution rpc: $EXECUTION_RPC"
echo "checkpoint:    $CHECKPOINT"
echo "helios rpc:    http://$RPC_BIND_IP:$RPC_PORT"

cd "$REPO_ROOT"
exec cargo run -- ethereum \
    --network "$NETWORK" \
    --consensus-rpc "$CONSENSUS_RPC" \
    --execution-rpc "$EXECUTION_RPC" \
    --checkpoint "$CHECKPOINT" \
    --data-dir "$DATA_DIR" \
    --rpc-bind-ip "$RPC_BIND_IP" \
    --rpc-port "$RPC_PORT" \
    "$@"
