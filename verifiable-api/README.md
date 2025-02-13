# helios-verifiable-api

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
