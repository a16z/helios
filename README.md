## Helios
[![build](https://github.com/a16z/helios/actions/workflows/test.yml/badge.svg)](https://github.com/a16z/helios/actions/workflows/test.yml) [![License: MIT](https://img.shields.io/badge/License-MIT-brightgreen.svg)](https://opensource.org/licenses/MIT)

Helios is a fully trustless, efficient, and portable Ethereum light client written in Rust.

Helios converts an untrusted centralized RPC endoint into a safe unmanipulable local RPC for its users. It syncs in seconds, requires no storage, and is lightweight enough to run on mobile devices.

The entire size of Helios's binary is 13Mb and should be easy to compile into WebAssembly. This makes it a perfect target to embed directly inside wallets and dapps.

## Installing
First install `heliosup`, Helios's installer:
```
curl https://github.com/a16z/helios/blob/master/heliosup/install | bash
```
To install Helios, run `heliosup`.

## Usage
To run Helios, run the below command, replacing `$ETH_RPC_URL` with an RPC provider URL such as Alchemy or Infura:
```
helios --execution-rpc $ETH_RPC_URL
```
Helios will now run a local RPC server at `http://127.0.0.1:8545`.

### Additional Options
`--consensus-rpc` or `-c` can be used to set a custom consensus layer rpc endpoint. This must be a consenus node that supports the light client beaconchain api. We recomment using Nimbus for this. If no consensus rpc is supplied, it defaults to `https://www.lightclientdata.org` which is run by us.

`--checkpoint` or `-w` can be used to set a custom weak subjectivity checkpoint. This must be equal the first beacon blockhash of an epoch. Weak subjectivity checkpoints are the root of trust in the system. If this is set to a malicious value, an attacker can cause the client to sync to the wrong chain. Helios sets a default value initially, then caches the most recent finalized block it has seen for later use.

`--network` or `-n` sets the network to sync to. Current valid option are `mainnet` and `goerli`, however users can add custom networks in their configurationf files.

`--rpc-port` or `-p` sets the port that the local RPC should run on. The default value is `8545`.

`--data-dir` or `-d` sets the directory that Helios should use to store cached weak subjectivity checkpoints in. Each network only stores the latest checkpoint, which is just 32 bytes.

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

## Contributing
All contributions to Helios are welcome. Before opening a PR, please submit an issue detailing the bug or feature. When opening a PR, please ensure that your contribution builds on the nightly rust toolchain, has been linted with `cargo fmt`, and contains tests when applicable.
