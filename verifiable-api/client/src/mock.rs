use std::{fs::read_to_string, path::PathBuf, str::FromStr};

use alloy::{
    eips::BlockId,
    network::{ReceiptResponse, TransactionResponse as TxTr},
    primitives::{Address, B256, U256},
    rpc::types::{EIP1186AccountProofResponse, Filter},
};
use async_trait::async_trait;
use eyre::{eyre, Result};
use url::Url;

use helios_common::network_spec::NetworkSpec;
use helios_verifiable_api_types::*;

use super::VerifiableApi;

#[derive(Clone)]
pub struct MockVerifiableApi {
    path: PathBuf,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec> VerifiableApi<N> for MockVerifiableApi {
    fn new(base_url: &Url) -> Self {
        // For mock, we expect a file:// URL or just use the path portion
        let path = if base_url.scheme() == "file" {
            PathBuf::from(base_url.path())
        } else {
            PathBuf::from(base_url.as_str())
        };
        Self { path }
    }

    async fn get_account(
        &self,
        address: Address,
        _storage_slots: &[U256],
        _block_id: Option<BlockId>,
        include_code: bool,
    ) -> Result<AccountResponse> {
        let proof: EIP1186AccountProofResponse =
            serde_json::from_str(&read_to_string(self.path.join("rpc/proof.json"))?)?;
        let block_miner_proof: EIP1186AccountProofResponse = serde_json::from_str(
            &read_to_string(self.path.join("rpc/block_miner_proof.json"))?,
        )?;
        let account: AccountResponse =
            serde_json::from_str(&read_to_string(self.path.join("rpc/account.json"))?)?;
        let block_miner_account: AccountResponse = serde_json::from_str(&read_to_string(
            self.path.join("rpc/block_miner_account.json"),
        )?)?;

        let mut account_response = match address {
            address if address == proof.address => Ok(account),
            address if address == block_miner_proof.address => Ok(block_miner_account),
            _ => Err(eyre!("Account not found")),
        }?;

        if !include_code {
            account_response.code = None;
        }
        Ok(account_response)
    }

    async fn get_transaction_receipt(
        &self,
        tx_hash: B256,
    ) -> Result<Option<TransactionReceiptResponse<N>>> {
        let json_str = read_to_string(self.path.join("verifiable_api/receipt.json"))?;
        let receipt: TransactionReceiptResponse<N> = serde_json::from_str(&json_str)?;
        let receipt = if receipt.receipt.transaction_hash() == tx_hash {
            Some(receipt)
        } else {
            None
        };
        Ok(receipt)
    }

    async fn get_transaction(&self, tx_hash: B256) -> Result<Option<TransactionResponse<N>>> {
        let json_str = read_to_string(self.path.join("rpc/transaction.json"))?;
        let transaction: N::TransactionResponse = serde_json::from_str(&json_str)?;
        let transaction = if transaction.tx_hash() == tx_hash {
            Some(TransactionResponse { 
                transaction,
                transaction_proof: vec![] // Empty proof for mock implementation
            })
        } else {
            None
        };
        Ok(transaction)
    }

    async fn get_transaction_by_location(
        &self,
        _block_id: BlockId,
        index: u64,
    ) -> Result<Option<TransactionResponse<N>>> {
        // Read block data to get transaction hash at index
        let block_json_str = read_to_string(self.path.join("rpc/block.json"))?;
        let block: serde_json::Value = serde_json::from_str(&block_json_str)?;
        
        let transactions = block
            .get("transactions")
            .and_then(|t| t.as_array())
            .ok_or_else(|| eyre!("Block has no transactions array"))?;

        let tx_hash_str = transactions
            .get(index as usize)
            .and_then(|t| t.as_str())
            .ok_or_else(|| eyre!("Transaction not found at index {}", index))?;

        let tx_hash = B256::from_str(tx_hash_str)
            .map_err(|_| eyre!("Invalid transaction hash format"))?;

        // Read transaction data
        let tx_json_str = read_to_string(self.path.join("rpc/transaction.json"))?;
        let transaction: N::TransactionResponse = serde_json::from_str(&tx_json_str)?;
        
        let transaction = if transaction.tx_hash() == tx_hash {
            Some(TransactionResponse { 
                transaction,
                transaction_proof: vec![] // Empty proof for mock implementation
            })
        } else {
            None
        };
        Ok(transaction)
    }

    async fn get_logs(&self, _filter: &Filter) -> Result<LogsResponse<N>> {
        let json_str = read_to_string(self.path.join("verifiable_api/logs.json"))?;
        Ok(serde_json::from_str(&json_str)?)
    }

    async fn get_execution_hint(
        &self,
        _tx: N::TransactionRequest,
        _validate_tx: bool,
        _block_id: Option<BlockId>,
    ) -> Result<ExtendedAccessListResponse> {
        let json_str = read_to_string(self.path.join("verifiable_api/access_list.json"))?;
        Ok(serde_json::from_str(&json_str)?)
    }

    async fn chain_id(&self) -> Result<ChainIdResponse> {
        let json_str = read_to_string(self.path.join("rpc/chain_id.txt"))?;
        Ok(ChainIdResponse {
            chain_id: u64::from_str(&json_str)?,
        })
    }

    async fn get_block(
        &self,
        _block_id: BlockId,
        _full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        let json_str = read_to_string(self.path.join("rpc/block.json"))?;
        let block = serde_json::from_str(&json_str)?;
        Ok(Some(block))
    }

    async fn get_block_receipts(&self, _block: BlockId) -> Result<Option<Vec<N::ReceiptResponse>>> {
        let receipts = read_to_string(self.path.join("rpc/receipts.json"))?;
        Ok(serde_json::from_str(&receipts)?)
    }

    async fn send_raw_transaction(&self, _bytes: &[u8]) -> Result<SendRawTxResponse> {
        let tx = read_to_string(self.path.join("rpc/transaction.json"))?;
        let tx = serde_json::from_str::<N::TransactionResponse>(&tx)?;
        Ok(SendRawTxResponse { hash: tx.tx_hash() })
    }
}
