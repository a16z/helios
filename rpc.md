# Helios Remote Procedure Calls

Helios provides a variety of RPC methods for interacting with the Ethereum network. These methods are exposed via the `Client` struct.  The RPC methods follow the [Ethereum JSON RPC Spec](https://ethereum.github.io/execution-apis/api-documentation). See [examples](./examples) of running remote procedure calls with Helios.

## RPC Methods

| RPC Method | Client Function | Description | Example |
| ---------- | --------------- | ----------- | ------- |
| `eth_getBalance` | `get_balance` | Returns the balance of the account given an address. | `client.get_balance(&self, address: &str, block: BlockTag)` |
| `eth_getTransactionCount` | `get_nonce` | Returns the number of transactions sent from the given address. | `client.get_nonce(&self, address: &str, block: BlockTag)` |
| `eth_getCode` | `get_code` | Returns the code at a given address. | `client.get_code(&self, address: &str, block: BlockTag)` |
| `eth_call` | `call` | Executes a new message call immediately without creating a transaction on the blockchain. | `client.call(&self, opts: CallOpts, block: BlockTag)` |
| `eth_estimateGas` | `estimate_gas` | Generates and returns an estimate of how much gas is necessary to allow the transaction to be completed. | `client.estimate_gas(&self, opts: CallOpts, block: BlockTag)` |
| `eth_createAccessList` | `create_access_list` | Creates an EIP2930 type `accessList` based on a given Transaction object. Returns list of addresses and storage keys that are read and written by the transaction (except precompiles), plus the estimated gas consumed when the access list is added. | `client.create_access_list(&self, opts: CallOpts, block: BlockTag)` |
| `eth_getChainId` | `chain_id` | Returns the chain ID of the current network. | `client.chain_id(&self)` |
| `eth_gasPrice` | `gas_price` | Returns the current price per gas in wei. | `client.gas_price(&self)` |
| `eth_maxPriorityFeePerGas` | `max_priority_fee_per_gas` | Returns the current max priority fee per gas in wei. | `client.max_priority_fee_per_gas(&self)` |
| `eth_blobBaseFee` | `blob_base_fee` | Returns the base fee per blob gas in wei. | `client.blob_base_fee(&self, block: BlockTag)` |
| `eth_blockNumber` | `block_number` | Returns the number of the most recent block. | `client.block_number(&self)` |
| `eth_getBlockByNumber` | `get_block_by_number` | Returns the information of a block by number. | `client.get_block_by_number(&self, block: BlockTag, full_tx: bool)` |
| `eth_getBlockByHash` | `get_block_by_hash` | Returns the information of a block by hash. | `client.get_block_by_hash(&self, hash: &str, full_tx: bool)` |
| `eth_sendRawTransaction` | `send_raw_transaction` | Submits a raw transaction to the network. | `client.send_raw_transaction(&self, bytes: &str)` |
| `eth_getTransactionReceipt` | `get_transaction_receipt` | Returns the receipt of a transaction by transaction hash. | `client.get_transaction_receipt(&self, hash: &str)` |
| `eth_getTransactionByHash` | `get_transaction_by_hash` | Returns the information about a transaction requested by transaction hash. | `client.get_transaction_by_hash(&self, hash: &str)` |
| `eth_getTransactionByBlockHashAndIndex` | `get_transaction_by_block_hash_and_index` | Returns information about a transaction by block hash and transaction index position. | `client.get_transaction_by_block_hash_and_index(&self, hash: &str, index: u64)` |
| `eth_getTransactionByBlockNumberAndIndex` | `get_transaction_by_block_number_and_index` | Returns information about a transaction by block number and transaction index position. | `client.get_transaction_by_block_number_and_index(&self, block: BlockTag, index: u64)` |
| `eth_getBlockReceipts` | `get_block_receipts` | Returns all transaction receipts of a block by number. | `client.get_block_receipts(&self, block: BlockTag)` |
| `eth_getBlockTransactionCountByHash` | `get_block_transaction_count_by_hash` | Returns the number of transactions in a block from a block matching the transaction hash. | `client.get_block_transaction_count_by_hash(&self, hash: &str)` |
| `eth_getBlockTransactionCountByNumber` | `get_block_transaction_count_by_number` | Returns the number of transactions in a block from a block matching the block number. | `client.get_block_transaction_count_by_number(&self, block: BlockTag)` |
| `eth_getLogs` | `get_logs` | Returns an array of logs matching the filter. | `client.get_logs(&self, filter: Filter)` |
| `eth_getFilterChanges` | `get_filter_changes` | Polling method for a filter, which returns an array of logs or transaction hashes or block hashes (depending on the type of filter) which occurred since the last poll. | `client.get_filter_changes(&self, filter_id: H256)` |
| `eth_getFilterLogs` | `get_filter_logs` | Returns an array of all logs matching the filter with a given id. | `client.get_filter_logs(&self, filter_id: H256)` |
| `eth_newFilter` | `new_filter` | Creates a filter object, based on filter options, to notify when the state changes (logs). | `client.new_filter(&self, filter: Filter)` |
| `eth_newBlockFilter` | `new_block_filter` | Creates a filter in the node, to notify when a new block arrives. | `client.new_block_filter(&self)` |
| `eth_newPendingTransactionFilter` | `new_pending_transaction_filter` | Creates a filter in the node, to notify when new pending transactions arrive. | `client.new_pending_transaction_filter(&self)` |
| `eth_getStorageAt` | `get_storage_at` | Returns the value from a storage position at a given address. | `client.get_storage_at(&self, address: &str, slot: H256, block: BlockTag)` |
| `eth_coinbase` | `get_coinbase` | Returns the client coinbase address. | `client.get_coinbase(&self)` |
| `eth_syncing` | `syncing` | Returns an object with data about the sync status or false. | `client.syncing(&self)` |
| `web3_clientVersion` | `client_version` | Returns the current version of the chain client. | `client.client_version(&self)` |
