# helios-verifiable-api

## Usage

```bash
$ cargo run -- --help
Helios' Verifiable API server

Usage: server <COMMAND>

Commands:
  ethereum
  opstack
  help      Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
```

```bash
$ cargo run -- ethereum --help
Usage: server ethereum [OPTIONS] --execution-rpc <EXECUTION_RPC>

Options:
  -s, --server-address <SERVER_ADDRESS>  [default: 127.0.0.1:4000]
  -e, --execution-rpc <EXECUTION_RPC>
  -h, --help                             Print help
```

#### Ethereum

```bash
$ cargo run -- ethereum --execution-rpc https://ethereum-rpc.publicnode.com
```

#### OP Stack (E.g. Base)

```bash
$ cargo run -- opstack --execution-rpc https://base-rpc.publicnode.com
```

## JSON-RPC to REST API map

| Ethereum JSON-RPC Method       | Helios Verifiable API Endpoint                                                  |
|--------------------------------|--------------------------------------------------------------------------------|
| `eth_getProof`                 | `/eth/v1/proof/account/{address}?storageSlots={}&block={}`                     |
| `eth_getBalance`               | `/eth/v1/proof/account/{address}?storageSlots={}&block={}`                     |
| `eth_getTransactionCount`      | `/eth/v1/proof/account/{address}?storageSlots={}&block={}`                     |
| `eth_getCode`                  | `/eth/v1/proof/account/{address}?storageSlots={}&block={}`                     |
| `eth_getStorageAt`             | `/eth/v1/proof/account/{address}?storageSlots={}&block={}`                     |
| `eth_getTransactionReceipt`    | `/eth/v1/proof/tx_receipt/{tx_hash}`                                           |
| `eth_getLogs`                  | `/eth/v1/proof/logs?fromBlock={}&toBlock={}&blockHash={}&address={}&topic0={}` |
| `eth_getFilterLogs`            | `/eth/v1/proof/filter_logs/{filter_id}`                                          |
| `eth_getFilterChanges`         | `/eth/v1/proof/filter_changes/{filter_id}`                                       |
| `eth_createAccessList`         | `/eth/v1/proof/create_access_list`                                             |
