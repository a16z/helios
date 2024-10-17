## Helios Configuration

All configuration options can be set on a per-network level in `~/.helios/helios.toml`.

#### Comprehensive Example

```toml
[mainnet]
# The consensus rpc to use. This should be a trusted rpc endpoint. Defaults to "https://www.lightclientdata.org".
consensus_rpc = "https://www.lightclientdata.org"
# [REQUIRED] The execution rpc to use. This should be a trusted rpc endpoint.
execution_rpc = "https://eth-mainnet.g.alchemy.com/v2/XXXXX"
# The port to run the JSON-RPC server on. By default, Helios will use port 8545.
rpc_port = 8545
# The ip that binds to the JSON-RPC server. By default, Helios will use 127.0.0.1. Use 0.0.0.0 to allow access from remote.
rpc_bind_ip = "127.0.0.1"
# The latest checkpoint. This should be a trusted checkpoint that is no greater than ~2 weeks old.
# If you are unsure what checkpoint to use, you can skip this option and set either `load_external_fallback` or `fallback` values (described below) to fetch a checkpoint. Though this is not recommended and less secure.
checkpoint = "0x85e6151a246e8fdba36db27a0c7678a575346272fe978c9281e13a8b26cdfa68"
# The directory to store the checkpoint database in. If not provided, Helios will use "~/.helios/data/mainnet", where `mainnet` is the network.
# It is recommended to set this directory to a persistent location mapped to a fast storage device.
data_dir = "/home/user/.helios/mainnet"
# The maximum age of a checkpoint in seconds. If the checkpoint is older than this, Helios will attempt to fetch a new checkpoint.
max_checkpoint_age = 86400
# A checkpoint fallback is used if no checkpoint is provided or the given checkpoint is too old.
# This is expected to be a trusted checkpoint sync api (like provided in https://github.com/ethpandaops/checkpoint-sync-health-checks/blob/master/_data/endpoints.yaml).
fallback = "https://sync-mainnet.beaconcha.in"
# If no checkpoint is provided, or the checkpoint is too old, Helios will attempt to dynamically fetch a checkpoint from a maintained list of checkpoint sync apis.
# NOTE: This is an insecure feature and not recommended for production use. Checkpoint manipulation is possible.
load_external_fallback = true

[goerli]
# The consensus rpc to use. This should be a trusted rpc endpoint. Defaults to Nimbus testnet.
consensus_rpc = "http://testing.prater.beacon-api.nimbus.team"
# [REQUIRED] The execution rpc to use. This should be a trusted rpc endpoint.
execution_rpc = "https://eth-goerli.g.alchemy.com/v2/XXXXX"
# The port to run the JSON-RPC server on. By default, Helios will use port 8545.
rpc_port = 8545
# The ip that binds to the JSON-RPC server. By default, Helios will use 127.0.0.1. Use 0.0.0.0 to allow access from remote.
rpc_bind_ip = "127.0.0.1"
# The latest checkpoint. This should be a trusted checkpoint that is no greater than ~2 weeks old.
# If you are unsure what checkpoint to use, you can skip this option and set either `load_external_fallback` or `fallback` values (described below) to fetch a checkpoint. Though this is not recommended and less secure.
checkpoint = "0xb5c375696913865d7c0e166d87bc7c772b6210dc9edf149f4c7ddc6da0dd4495"
# The directory to store the checkpoint database in. If not provided, Helios will use "~/.helios/data/goerli", where `goerli` is the network.
# It is recommended to set this directory to a persistent location mapped to a fast storage device.
data_dir = "/home/user/.helios/goerli"
# The maximum age of a checkpoint in seconds. If the checkpoint is older than this, Helios will attempt to fetch a new checkpoint.
max_checkpoint_age = 86400
# A checkpoint fallback is used if no checkpoint is provided or the given checkpoint is too old.
# This is expected to be a trusted checkpoint sync api (like provided in https://github.com/ethpandaops/checkpoint-sync-health-checks/blob/master/_data/endpoints.yaml).
fallback = "https://sync-goerli.beaconcha.in"
# If no checkpoint is provided, or the checkpoint is too old, Helios will attempt to dynamically fetch a checkpoint from a maintained list of checkpoint sync apis.
# NOTE: This is an insecure feature and not recommended for production use. Checkpoint manipulation is possible.
load_external_fallback = true
```


#### Options

All configuration options below are available on a per-network level, where network is specified by a header (eg `[mainnet]` or `[goerli]`). Many of these options can be configured through cli flags as well. See [README.md](./README.md#additional-options) or run `helios --help` for more information.


- `consensus_rpc` - The URL of the consensus RPC endpoint used to fetch the latest beacon chain head and sync status. This must be a consensus node that supports the light client beaconchain api. We recommend using Nimbus for this. If no consensus rpc is supplied, it defaults to `https://www.lightclientdata.org` which is run by us.

- `execution_rpc` - The URL of the execution RPC endpoint used to fetch the latest execution chain head and sync status. This must be an execution node that supports the light client execution api. We recommend using Geth for this.

- `rpc_port` - The port to run the JSON-RPC server on. By default, Helios will use port 8545.

- `rpc_bind_ip` - The ip that binds to the JSON-RPC server. By default, Helios will use 127.0.0.1. Use 0.0.0.0 to allow access from remote.

- `checkpoint` - The latest checkpoint. This should be a trusted checkpoint that is no greater than ~2 weeks old. If you are unsure what checkpoint to use, you can skip this option and set either `load_external_fallback` or `fallback` values (described below) to fetch a checkpoint. Though this is not recommended and less secure.

- `data_dir` - The directory to store the checkpoint database in. If not provided, Helios will use "~/.helios/data/<NETWORK>", where `<NETWORK>` is the network. It is recommended to set this directory to a persistent location mapped to a fast storage device.

- `max_checkpoint_age` - The maximum age of a checkpoint in seconds. If the checkpoint is older than this, Helios will attempt to fetch a new checkpoint.

- `fallback` - A checkpoint fallback is used if no checkpoint is provided or the given checkpoint is too old. This is expected to be a trusted checkpoint sync api (eg https://sync-mainnet.beaconcha.in). An extensive list of checkpoint sync apis can be found here: https://github.com/ethpandaops/checkpoint-sync-health-checks/blob/master/_data/endpoints.yaml.

- `load_external_fallback` - If no checkpoint is provided, or the checkpoint is too old, Helios will attempt to dynamically fetch a checkpoint from a maintained list of checkpoint sync apis. NOTE: This is an insecure feature and not recommended for production use. Checkpoint manipulation is possible.

