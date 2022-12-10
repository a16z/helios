# Helios Remote Procedure Calls

Helios provides a variety of RPC methods for interacting with the Ethereum network. These methods are exposed via the `Client` struct.  The RPC methods follow the [Ethereum JSON RPC Spec](https://ethereum.github.io/execution-apis/api-documentation). See [examples](./examples/readme.rs) of running remote procedure calls with Helios.

## RPC Methods

| Method | Description | Example |
| ------ | ----------- | ------- |
| `get_balance` | Returns the balance of the account given address. | `client.get_balance(&self, address: &str, block: BlockTag)` |
| `get_transaction_count` | Returns the number of transactions sent from the given address. | `client.get_transaction_count(&self, address: &str, block: BlockTag)` |
| `get_code` | Returns the code at a given address. | `client.get_code(&self, address: &str, block: BlockTag)` |
| `call` | Executes a new message call immediately without creating a transaction on the blockchain. | `client.call(&self, opts: CallOpts, block: BlockTag)` |
| `estimate_gas` | Generates and returns an estimate of how much gas is necessary to allow the transaction to complete. | `client.estimate_gas(&self, opts: CallOpts)` |
| `chain_id` | Returns the chain ID of the current network. | `client.chain_id(&self)` |
| `gas_price` | Returns the current price per gas in wei. | `client.gas_price(&self)` |
| `max_priority_fee_per_gas` | Returns the current max priority fee per gas in wei. | `client.max_priority_fee_per_gas(&self)` |
| `block_number` | Returns the number of most recent block. | `client.block_number(&self)` |
| `get_block_by_number` | Returns the information of a block by number. | `get_block_by_number(&self, block: BlockTag, full_tx: bool)` |
| `get_block_by_hash` | Returns the information of a block by hash. | `get_block_by_hash(&self, hash: &str, full_tx: bool)` |
| `send_raw_transaction` | Submits a raw transaction to the network. | `client.send_raw_transaction(&self, bytes: &str)` |
| `get_transaction_receipt` | Returns the receipt of a transaction by transaction hash. | `client.get_transaction_receipt(&self, hash: &str)` |
| `get_logs` | Returns an array of logs matching the filter. | `client.get_logs(&self, filter: Filter)` |
| `get_storage_at` | Returns the value from a storage position at a given address. | `client.get_storage_at(&self, address: &str, slot: H256, block: BlockTag)` |