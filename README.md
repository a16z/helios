## Helios

[![build](https://github.com/a16z/helios/actions/workflows/test.yml/badge.svg)](https://github.com/a16z/helios/actions/workflows/test.yml) [![license: MIT](https://img.shields.io/badge/license-MIT-brightgreen.svg)](https://opensource.org/licenses/MIT) [![chat](https://img.shields.io/badge/chat-telegram-blue)](https://t.me/+IntDY_gZJSRkNTJj)

Helios is a trustless, efficient, and portable multichain light client written in Rust.

Helios converts an untrusted centralized RPC endpoint into a safe unmanipulable local RPC for its users. It syncs in seconds, requires no storage, and is lightweight enough to run on mobile devices.

Helios has a small binary size and compiles into WebAssembly. This makes it a perfect target to embed directly inside wallets and dapps.

## Installing

First install `heliosup`, Helios's installer:

```
curl https://raw.githubusercontent.com/a16z/helios/master/heliosup/install | bash
```

To install Helios, run `heliosup`.

## Usage
### Ethereum

To run Helios on Ethereum, run the command below, replacing `$ETH_RPC_URL` with an RPC provider URL such as Alchemy:

```
helios ethereum --execution-rpc $ETH_RPC_URL
```

`$ETH_RPC_URL` must be a [supported Ethereum Execution API Provider](#supported-execution-api-providers) that provides the `eth_getProof` endpoint. Infura does not currently support this. We recommend using Alchemy.

Helios will now run a local RPC server at `http://127.0.0.1:8545`.

Helios also provides documentation of its supported RPC methods in the [rpc.md](./rpc.md) file.


### OP Stack

To run Helios on an OP Stack chain, run the command below, replacing `$ETH_RPC_URL` with an RPC provider URL such as Alchemy and `$NETWORK` with a supported OP Stack network:

```
helios opstack --network $NETWORK --execution-rpc $ETH_RPC_URL
```

Currently supported network values are `op-mainnet` and `base`, with more to come soon.

### Additional Ethereum CLI Options <a id="additional-cli-options"></a>

`--consensus-rpc` or `-c` can be used to set a custom consensus layer rpc endpoint. This must be a consensus node that supports the light client beaconchain api. We recommend using Nimbus for this. If no consensus rpc is supplied, it defaults to `https://www.lightclientdata.org` which is run by us.

`--checkpoint` or `-w` can be used to set a custom weak subjectivity checkpoint. This must be equal to the first beacon block hash of an epoch. Weak subjectivity checkpoints are the root of trust in the system. If this is set to a malicious value, an attacker can cause the client to sync to the wrong chain. Helios sets a default value initially, then caches the most recent finalized block it has seen for later use.

`--network` or `-n` sets the network to sync to. Current valid options are `mainnet`, `sepolia`, and `holesky` however users can add custom networks in their configuration files.

`--rpc-port` or `-p` sets the port that the local RPC should run on. The default value is `8545`.

`--rpc-bind-ip` or `-b` sets the ip that binds to the JSON-RPC server. By default, Helios will use `127.0.0.1`. Use `0.0.0.0` to allow remote access.

`--data-dir` or `-d` sets the directory that Helios should use to store cached weak subjectivity checkpoints in. Each network only stores the latest checkpoint, which is just 32 bytes.

`--fallback` or `-f` sets the checkpoint fallback url (a string). This is only used if the checkpoint provided by the `--checkpoint` flag is too outdated for Helios to use to sync.
If none is provided and the `--load-external-fallback` flag is not set, Helios will error.
For example, you can specify the fallback like so: `helios --fallback "https://sync-mainnet.beaconcha.in"` (or using shorthand like so: `helios -f "https://sync-mainnet.beaconcha.in"`)

`--load-external-fallback` or `-l` enables weak subjectivity checkpoint fallback (no value needed).
For example, say you set a checkpoint value that is too outdated and Helios cannot sync to it.
If this flag is set, Helios will query all network apis in the community-maintained list
at [ethpandaops/checkpoint-sync-health-checks](https://github.com/ethpandaops/checkpoint-sync-health-checks/blob/master/_data/endpoints.yaml) for their latest slots.
The list of slots is filtered for healthy apis and the most frequent checkpoint occurring in the latest epoch will be returned.
Note: this is a community-maintained list and thus no security guarantees are provided. Use this as a last resort if your checkpoint passed into `--checkpoint` fails.
This is not recommended as malicious checkpoints can be returned from the listed apis, even if they are considered _healthy_.
This can be run like so: `helios --load-external-fallback` (or `helios -l` with the shorthand).

`--strict-checkpoint-age` or `-s` enables strict checkpoint age checking. If the checkpoint is over two weeks old and this flag is enabled, Helios will error. Without this flag, Helios will instead surface a warning to the user and continue. If the checkpoint is greater than two weeks old, there are theoretical attacks that can cause Helios and over light clients to sync incorrectly. These attacks are complex and expensive, so Helios disables this by default.

`--help` or `-h` prints the help message.

### Configuration Files <a id="configuration-files"></a>

All configuration options can be set on a per-network level in `~/.helios/helios.toml`. Here is an example config file:

```toml
[mainnet]
consensus_rpc = "https://ethereum.operationsolarstorm.org"
execution_rpc = "https://eth-mainnet.g.alchemy.com/v2/XXXXX"
checkpoint = "0x85e6151a246e8fdba36db27a0c7678a575346272fe978c9281e13a8b26cdfa68"

[op-mainnet]
consensus_rpc = "https://op-mainnet.operationsolarstorm.org"
execution_rpc = "https://opt-mainnet.g.alchemy.com/v2/XXXXX"

[base]
consensus_rpc = "https://base.operationsolarstorm.org"
execution_rpc = "https://base-mainnet.g.alchemy.com/v2/XXXXX"
```

A comprehensive breakdown of config options is available in the [config.md](./config.md) file.

### Using Helios as a library

Examples of running Helios as a rust library can be seen in the [examples](./examples) directory.

### Supported Ethereum Checkpoints <a id="supported-checkpoints"></a>

A checkpoint is a Beacon Chain Consensus Layer block hash rather than a Execution Layer block hash. An example of an Execution Layer block hash for Holesky is shown at https://holesky.etherscan.io/blocks

Checkpoints may be obtained from the following links:
* Ethereum Mainnet https://beaconcha.in
* Holesky Testnet https://holesky.beaconcha.in

It is recommended to use a block hash as a checkpoint that is less than two weeks old, however you can actually use older checkpoints and it will still work but will give you a warning. Using a checkpoint that is less than two weeks old prevents a few attacks that are pretty hard to pull off.

For example, to obtain a recent checkpoint for Holesky Testnet go to https://holesky.beaconcha.in/ and get the block hash of the first block in any finalised epoch. At the time of writing, the [first block hash in epoch 78425](https://holesky.beaconcha.in/epoch/78425) is the [oldest slot 2509600](https://holesky.beaconcha.in/slot/2509600) that has a Block Root of 0x60409a013161b33c8c68c6183c7753e779ec6c24be2f3c50c6036c30e13b34a6 and is the latest checkpoint value to use.

This latest checkpoint may be provided as an [Additional CLI Option](#additional-cli-options) at the command line to run a Helios Light Client node on Ethereum Holesky Testnet:
```bash
helios ethereum \
    --network holesky \
    --consensus-rpc http://testing.holesky.beacon-api.nimbus.team \
    --execution-rpc https://ethereum-holesky.g.allthatnode.com \
    --checkpoint 0x60409a013161b33c8c68c6183c7753e779ec6c24be2f3c50c6036c30e13b34a6
```

For example, to obtain a recent checkpoint for Ethereum Mainnet go to https://beaconcha.in and get the block hash of the first block in any finalised epoch. At the time of writing the [first block hash in epoch 222705](https://beaconcha.in/epoch/222705) is the [oldest slot 7126560](https://beaconcha.in/slot/7126560) that has a Block Root of 0xe1912ca8ca3b45dac497cae7825bab055b0f60285533721b046e8fefb5b076f2 and is the latest checkpoint value to use.

This latest checkpoint may be provided as an [Additional CLI Option](#additional-cli-options) at the command line to run a Helios Light Client node on Ethereum Mainnet:
```bash
helios ethereum \
    --network mainnet \
    --consensus-rpc https://www.lightclientdata.org \
    --execution-rpc https://ethereum-mainnet.g.allthatnode.com \
    --checkpoint 0xe1912ca8ca3b45dac497cae7825bab055b0f60285533721b046e8fefb5b076f2
```

If you wish to use a [Configuration File](#configuration-files) instead of CLI arguments then you should replace the example checkpoints in the configuration file with the latest checkpoints obtained above.

## Testing

To ensure that Helios works as expected, we have a comprehensive test suite that you can run. Before running the tests, make sure to create a `.env` file in the root of the project directory. You can copy the contents of the `.env.example` file and fill in your own secrets.
```sh
cp .env.example .env
```

To run all tests, use the following command:

```sh
cargo test --all
```

To run tests for an individual package, use this command, replacing <package-name> with the package you want to test:

```sh
cargo test -p <package-name>
```

## Contributing

All contributions to Helios are welcome. Before opening a PR, please submit an issue detailing the bug or feature. When opening a PR, please ensure that your contribution builds, has been linted with `cargo fmt`, and contains tests when applicable.

## Telegram

If you are having trouble with Helios or are considering contributing, feel free to join our Telegram [here](https://t.me/+IntDY_gZJSRkNTJj).

## Disclaimer

_This code is being provided as is. No guarantee, representation or warranty is being made, express or implied, as to the safety or correctness of the code. It has not been audited and as such there can be no assurance it will work as intended, and users may experience delays, failures, errors, omissions or loss of transmitted information. Nothing in this repo should be construed as investment advice or legal advice for any particular facts or circumstances and is not meant to replace competent counsel. It is strongly advised for you to contact a reputable attorney in your jurisdiction for any questions or concerns with respect thereto. a16z is not liable for any use of the foregoing, and users should proceed with caution and use at their own risk. See a16z.com/disclosures for more info._
