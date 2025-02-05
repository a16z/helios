# helios-verifiable-api

## JSON-RPC to REST API map

| Ethereum JSON-RPC Method       | Helios Verifiable API Endpoint                                           |
|--------------------------------|-------------------------------------------------------------------------|
| `eth_getBalance`               | `/eth/v1/proof/balance/{address}?block={tag_or_hash_number}`            |
| `eth_getTransactionCount`      | `/eth/v1/proof/transaction_count/{address}?block={tag_or_hash_number}`  |
| `eth_getCode`                  | `/eth/v1/proof/code/{address}?block={tag_or_hash_number}`               |
| `eth_getStorageAt`             | `/eth/v1/proof/storage/{address}/{slot}?block={tag_or_hash_number}`     |
| `eth_getTransactionReceipt`    | `/eth/v1/proof/tx_receipt/{tx_hash}`                                    |
| `eth_getFilterLogs`            | `/eth/v1/proof/filter_logs/{filter_id}`                                   |