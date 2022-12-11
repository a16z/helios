## Helios

[![build](https://github.com/a16z/helios/actions/workflows/test.yml/badge.svg)](https://github.com/a16z/helios/actions/workflows/test.yml) [![license: MIT](https://img.shields.io/badge/license-MIT-brightgreen.svg)](https://opensource.org/licenses/MIT) [![chat](https://img.shields.io/badge/chat-telegram-blue)](https://t.me/+IntDY_gZJSRkNTJj)

Helios is a fully trustless, efficient, and portable Ethereum light client written in Rust.

Helios converts an untrusted centralized RPC endpoint into a safe unmanipulable local RPC for its users. It syncs in seconds, requires no storage, and is lightweight enough to run on mobile devices.

The entire size of Helios's binary is 13Mb and should be easy to compile into WebAssembly. This makes it a perfect target to embed directly inside wallets and dapps.

## Installing

First install `heliosup`, Helios's installer:

```
curl https://raw.githubusercontent.com/a16z/helios/master/heliosup/install | bash
```

To install Helios, run `heliosup`.

## Usage

To run Helios, run the below command, replacing `$ETH_RPC_URL` with an RPC provider URL such as Alchemy or Infura:

```
helios --execution-rpc $ETH_RPC_URL
```

`$ETH_RPC_URL` must be an Ethereum provider that supports the `eth_getProof` endpoint. Infura does not currently support this. We recommend using Alchemy.

Helios will now run a local RPC server at `http://127.0.0.1:8545`.

Helios provides examples in the [`examples/`](./examples/) directory. To run an example, you can execute `cargo run --example <example_name>` from inside the helios repository.

Helios also provides documentation of its supported RPC methods in the [rpc.md](./rpc.md) file.

### Warning

Helios is still experimental software. While we hope you try it out, we do not suggest adding it as your main RPC in wallets yet. Sending high-value transactions from a wallet connected to Helios is discouraged.

### Additional Options

`--consensus-rpc` or `-c` can be used to set a custom consensus layer rpc endpoint. This must be a consenus node that supports the light client beaconchain api. We recommend using Nimbus for this. If no consensus rpc is supplied, it defaults to `https://www.lightclientdata.org` which is run by us.

`--checkpoint` or `-w` can be used to set a custom weak subjectivity checkpoint. This must be equal the first beacon blockhash of an epoch. Weak subjectivity checkpoints are the root of trust in the system. If this is set to a malicious value, an attacker can cause the client to sync to the wrong chain. Helios sets a default value initially, then caches the most recent finalized block it has seen for later use.

`--network` or `-n` sets the network to sync to. Current valid options are `mainnet` and `goerli`, however users can add custom networks in their configuration files.

`--rpc-port` or `-p` sets the port that the local RPC should run on. The default value is `8545`.

`--data-dir` or `-d` sets the directory that Helios should use to store cached weak subjectivity checkpoints in. Each network only stores the latest checkpoint, which is just 32 bytes.

`--fallback` or `-f` sets the checkpoint fallback url (a string). This is only used if the checkpoint provided by the `--checkpoint` flag is too outdated for Helios to use to sync.
If none is provided and the `--load-external-fallback` flag is not set, Helios will error.
For example, you can specify the fallback like so: `helios --fallback "https://sync-mainnet.beaconcha.in"` (or using shorthand like so: `helios -f "https://sync-mainnet.beaconcha.in"`)

`--load-external-fallback` or `-l` enables weak subjectivity checkpoint fallback (no value needed).
For example, say you set a checkpoint value that is too outdated and Helios cannot sync to it.
If this flag is set, Helios will query all network apis in the community-maintained list
at [ethpandaops/checkpoint-synz-health-checks](https://github.com/ethpandaops/checkpoint-sync-health-checks/blob/master/_data/endpoints.yaml) for their latest slots.
The list of slots is filtered for healthy apis and the most frequent checkpoint occuring in the latest epoch will be returned.
Note: this is a community-maintained list and thus no security guarantees are provided. Use this is a last resort if your checkpoint passed into `--checkpoint` fails.
This is not recommened as malicious checkpoints can be returned from the listed apis, even if they are considered _healthy_.
This can be run like so: `helios --load-external-fallback` (or `helios -l` with the shorthand).

`--help` or `-h` prints the help message.

### Configuration Files

All configuration options can be set on a per-network level in `~/.helios/helios.toml`. Here is an example config file:

```toml
[mainnet]
consensus_rpc = "https://www.lightclientdata.org"
execution_rpc = "https://eth-mainnet.g.alchemy.com/v2/XXXXX"
checkpoint = "0x85e6151a246e8fdba36db27a0c7678a575346272fe978c9281e13a8b26cdfa68"

[goerli]
consensus_rpc = "http://testing.prater.beacon-api.nimbus.team"
execution_rpc = "https://eth-goerli.g.alchemy.com/v2/XXXXX"
checkpoint = "0xb5c375696913865d7c0e166d87bc7c772b6210dc9edf149f4c7ddc6da0dd4495"
```

### Using Helios as a Library

Helios can be imported into any Rust project. Helios requires the Rust nightly toolchain to compile.

```rust
use std::{str::FromStr, env};

use helios::{client::ClientBuilder, config::networks::Network, types::BlockTag};
use ethers::{types::Address, utils};
use eyre::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let untrusted_rpc_url = env::var("UNTRUSTED_RPC_URL")?;

    let mut client = ClientBuilder::new()
        .network(Network::MAINNET)
        .consensus_rpc("https://www.lightclientdata.org")
        .execution_rpc(&untrusted_rpc_url)
        .build()?;

    client.start().await?;

    let head_block_num = client.get_block_number().await?;
    let addr = Address::from_str("0x00000000219ab540356cBB839Cbe05303d7705Fa")?;
    let block = BlockTag::Latest;
    let balance = client.get_balance(&addr, block).await?;

    println!("synced up to block: {}", head_block_num);
    println!("balance of deposit contract: {}", utils::format_ether(balance));

    Ok(())
}
```

Below we demonstrate fetching checkpoints from the community-maintained list of checkpoint sync apis maintained by [ethPandaOps](https://github.com/ethpandaops/checkpoint-sync-health-checks/blob/master/_data/endpoints.yaml).

> **Warning**
>
> This is a community-maintained list and thus no security guarantees are provided. Attacks on your light client can occur if malicious checkpoints are set in the list. Please use the explicit `checkpoint` flag, environment variable, or config setting with an updated, and verified checkpoint.

```rust
use eyre::Result;
use helios::config::{checkpoints, networks};

#[tokio::main]
async fn main() -> Result<()> {
    // Construct the checkpoint fallback services
    let cf = checkpoints::CheckpointFallback::new().build().await.unwrap();

    // Fetch the latest goerli checkpoint
    let goerli_checkpoint = cf.fetch_latest_checkpoint(&networks::Network::GOERLI).await.unwrap();
    println!("Fetched latest goerli checkpoint: {}", goerli_checkpoint);

    // Fetch the latest mainnet checkpoint
    let mainnet_checkpoint = cf.fetch_latest_checkpoint(&networks::Network::MAINNET).await.unwrap();
    println!("Fetched latest mainnet checkpoint: {}", mainnet_checkpoint);

    Ok(())
}
```

## Benchmarks

Benchmarks are defined in the [benches](./benches/) subdirectory. They are built using the [criterion](https://github.com/bheisler/criterion.rs) statistics-driven benchmarking library.

To run all benchmarks, you can use `cargo bench`. To run a specific benchmark, you can use `cargo bench --bench <name>`, where `<name>` is one of the benchmarks defined in the [Cargo.toml](./Cargo.toml) file under a `[[bench]]` section.

## Contributing

All contributions to Helios are welcome. Before opening a PR, please submit an issue detailing the bug or feature. When opening a PR, please ensure that your contribution builds on the nightly rust toolchain, has been linted with `cargo fmt`, and contains tests when applicable.

## Telegram

If you are having trouble with Helios or are considering contributing, feel free to join our telegram [here](https://t.me/+IntDY_gZJSRkNTJj).

## Disclaimer

_This code is being provided as is. No guarantee, representation or warranty is being made, express or implied, as to the safety or correctness of the code. It has not been audited and as such there can be no assurance it will work as intended, and users may experience delays, failures, errors, omissions or loss of transmitted information. Nothing in this repo should be construed as investment advice or legal advice for any particular facts or circumstances and is not meant to replace competent counsel. It is strongly advised for you to contact a reputable attorney in your jurisdiction for any questions or concerns with respect thereto. a16z is not liable for any use of the foregoing, and users should proceed with caution and use at their own risk. See a16z.com/disclosures for more info._
