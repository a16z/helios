# Helios Remote Procedure Calls

Helios provides a variety of RPC methods for interacting with the Ethereum network. These methods are exposed via the `Client` struct.  The RPC methods follow the [Ethereum JSON RPC Spec](https://ethereum.github.io/execution-apis/api-documentation). See [examples](./examples/readme.rs) of running remote procedure calls with Helios.

## RPC Methods

| RPC Method | Client Function | Description | Example |
| ---------- | --------------- | ----------- | ------- |
| `eth_getBalance` | `get_balance` | Returns the balance of the account given an address. | `client.get_balance(&self, address: &str, block: BlockTag)` |
| `eth_getTransactionCount` | `get_transaction_count` | Returns the number of transactions sent from the given address. | `client.get_transaction_count(&self, address: &str, block: BlockTag)` |
| `eth_getCode` | `get_code` | Returns the code at a given address. | `client.get_code(&self, address: &str, block: BlockTag)` |
| `eth_call` | `call` | Executes a new message call immediately without creating a transaction on the blockchain. | `client.call(&self, opts: CallOpts, block: BlockTag)` |
| `eth_estimateGas` | `estimate_gas` | Generates and returns an estimate of how much gas is necessary to allow the transaction to complete. | `client.estimate_gas(&self, opts: CallOpts)` |
| `eth_getChainId` | `chain_id` | Returns the chain ID of the current network. | `client.chain_id(&self)` |
| `eth_gasPrice` | `gas_price` | Returns the current price per gas in wei. | `client.gas_price(&self)` |
| `eth_maxPriorityFeePerGas` | `max_priority_fee_per_gas` | Returns the current max priority fee per gas in wei. | `client.max_priority_fee_per_gas(&self)` |
| `eth_blockNumber` | `block_number` | Returns the number of the most recent block. | `client.block_number(&self)` |
| `eth_getBlockByNumber` | `get_block_by_number` | Returns the information of a block by number. | `get_block_by_number(&self, block: BlockTag, full_tx: bool)` |
| `eth_getBlockByHash` | `get_block_by_hash` | Returns the information of a block by hash. | `get_block_by_hash(&self, hash: &str, full_tx: bool)` |
| `eth_sendRawTransaction` | `send_raw_transaction` | Submits a raw transaction to the network. | `client.send_raw_transaction(&self, bytes: &str)` |
| `eth_getTransactionReceipt` | `get_transaction_receipt` | Returns the receipt of a transaction by transaction hash. | `client.get_transaction_receipt(&self, hash: &str)` |
| `eth_getLogs` | `get_logs` | Returns an array of logs matching the filter. | `client.get_logs(&self, filter: Filter)` |
| `eth_getStorageAt` | `get_storage_at` | Returns the value from a storage position at a given address. | `client.get_storage_at(&self, address: &str, slot: H256, block: BlockTag)` |