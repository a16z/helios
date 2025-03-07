# helios-verifiable-api

A JSON REST API for Ethereum Execution Layer with verifiable responses as a first-class feature.

It wraps and replaces [Ethereum's execution JSON-RPC](https://ethereum.org/en/developers/docs/apis/json-rpc/).

## API Spec

[openapi.yaml](https://github.com/a16z/helios/blob/master/verifiable-api/server/openapi.yaml)

## Server Usage

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

### Examples

**Ethereum**

```bash
$ cargo run -- ethereum --execution-rpc https://ethereum-rpc.publicnode.com
```

**OP Stack (E.g. Base)**

```bash
$ cargo run -- opstack --execution-rpc https://base-rpc.publicnode.com
```

## Client Usage

See [helios-verifiable-api-client](verifiable-api/client) crate.

## JSON-RPC to REST API map

### Verifiable endpoints

Below is a map of RPC methods to Verifiable-API endpoints that are augmented with relevant proofs for verifiability.

| Ethereum JSON-RPC Method       | Helios Verifiable API Endpoint                                                           |
|--------------------------------|-----------------------------------------------------------------------------------------|
| `eth_getProof`                 | `GET /eth/v1/proof/account/{address}?block={}`                                          |
| `eth_getBalance`               | `GET /eth/v1/proof/account/{address}?block={}`                                          |
| `eth_getTransactionCount`      | `GET /eth/v1/proof/account/{address}?block={}`                                          |
| `eth_getCode`                  | `GET /eth/v1/proof/account/{address}?includeCode=true&block={}`                         |
| `eth_getStorageAt`             | `GET /eth/v1/proof/account/{address}?storageSlots={}&block={}`                          |
| `eth_getTransactionReceipt`    | `GET /eth/v1/proof/transaction/{tx_hash}/receipt`                                       |
| `eth_getLogs`                  | `GET /eth/v1/proof/logs?fromBlock={}&toBlock={}&blockHash={}&address={}&topic0={}`      |
| `eth_getFilterLogs`            | `GET /eth/v1/proof/filterLogs/{filter_id}`                                                |
| `eth_getFilterChanges`         | `GET /eth/v1/proof/filterChanges/{filter_id}`                                             |
| `eth_createAccessList`         | `POST /eth/v1/proof/createExtendedAccessList`                                           |


### Proxy endpoints

Below is a map of RPC methods to Verifiable-API endpoints that cannot be augmented with proofs but still supported
so a client only needs to consume a single data source.

| Ethereum JSON-RPC Method         | Helios Verifiable API Endpoint               |
|----------------------------------|---------------------------------------------|
| `eth_chainId`                    | `GET /eth/v1/chainId`                       |
| `eth_sendRawTransaction`         | `POST /eth/v1/sendRawTransaction`           |
| `eth_getBlockByHash`             | `GET /eth/v1/block/{block_id}`              |
| `eth_getBlockByNumber`           | `GET /eth/v1/block/{block_id}`              |
| `eth_getBlockReceipts`           | `GET /eth/v1/block/{block_id}/receipts`     |
| `eth_newFilter`                  | `POST /eth/v1/filter`                        |
| `eth_newBlockFilter`             | `POST /eth/v1/filter`                        |
| `eth_newPendingTransactionFilter`| `POST /eth/v1/filter`                        |
| `eth_uninstallFilter`            | `DELETE /eth/v1/filter/{filter_id}`           |
