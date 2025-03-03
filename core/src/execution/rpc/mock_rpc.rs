use std::{fs::read_to_string, path::PathBuf, str::FromStr};

use alloy::network::TransactionResponse;
use alloy::primitives::{Address, B256, U256};
use alloy::rpc::types::{
    AccessList, BlockId, BlockTransactionsKind, EIP1186AccountProofResponse, FeeHistory, Filter,
    FilterChanges, Log,
};
use async_trait::async_trait;
use eyre::{Ok, Result};

use helios_common::{network_spec::NetworkSpec, types::Account};

use crate::execution::errors::ExecutionError;

use super::ExecutionRpc;

#[derive(Clone)]
pub struct MockRpc {
    path: PathBuf,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec> ExecutionRpc<N> for MockRpc {
    fn new(rpc: &str) -> Result<Self> {
        let path = PathBuf::from(rpc);
        Ok(MockRpc { path })
    }

    async fn get_proof(
        &self,
        address: Address,
        _slots: &[B256],
        _block: BlockId,
    ) -> Result<EIP1186AccountProofResponse> {
        let proof: EIP1186AccountProofResponse =
            serde_json::from_str(&read_to_string(self.path.join("proof.json"))?)?;
        let block_miner_proof: EIP1186AccountProofResponse =
            serde_json::from_str(&read_to_string(self.path.join("block_miner_proof.json"))?)?;
        match address {
            address if address == proof.address => Ok(proof),
            address if address == block_miner_proof.address => Ok(block_miner_proof),
            _ => Err(ExecutionError::InvalidAccountProof(address).into()),
        }
    }

    async fn create_access_list(
        &self,
        _tx: &N::TransactionRequest,
        _block: BlockId,
    ) -> Result<AccessList> {
        let access_list = read_to_string(self.path.join("access_list.json"))?;
        Ok(serde_json::from_str(&access_list)?)
    }

    async fn get_code(&self, address: Address, _block: BlockId) -> Result<Vec<u8>> {
        let proof: EIP1186AccountProofResponse =
            serde_json::from_str(&read_to_string(self.path.join("proof.json"))?)?;
        let block_miner_proof: EIP1186AccountProofResponse =
            serde_json::from_str(&read_to_string(self.path.join("block_miner_proof.json"))?)?;
        let account: Account =
            serde_json::from_str(&read_to_string(self.path.join("account.json"))?)?;
        let block_miner_account: Account =
            serde_json::from_str(&read_to_string(self.path.join("block_miner_account.json"))?)?;

        let code = match address {
            address if address == proof.address => Ok(account.code.unwrap()),
            address if address == block_miner_proof.address => {
                Ok(block_miner_account.code.unwrap())
            }
            _ => Err(ExecutionError::InvalidAccountProof(address).into()),
        }?;

        Ok(code.into())
    }

    async fn send_raw_transaction(&self, _bytes: &[u8]) -> Result<B256> {
        let tx = read_to_string(self.path.join("transaction.json"))?;
        let tx = serde_json::from_str::<N::TransactionResponse>(&tx)?;
        Ok(tx.tx_hash())
    }

    async fn get_transaction_receipt(&self, _tx_hash: B256) -> Result<Option<N::ReceiptResponse>> {
        let receipt = read_to_string(self.path.join("receipt.json"))?;
        Ok(serde_json::from_str(&receipt)?)
    }

    async fn get_block_receipts(&self, _block: BlockId) -> Result<Option<Vec<N::ReceiptResponse>>> {
        let receipts = read_to_string(self.path.join("receipts.json"))?;
        Ok(serde_json::from_str(&receipts)?)
    }

    async fn get_transaction(&self, _tx_hash: B256) -> Result<Option<N::TransactionResponse>> {
        let tx = read_to_string(self.path.join("transaction.json"))?;
        Ok(serde_json::from_str(&tx)?)
    }

    async fn get_logs(&self, _filter: &Filter) -> Result<Vec<Log>> {
        let logs = read_to_string(self.path.join("logs.json"))?;
        Ok(serde_json::from_str(&logs)?)
    }

    async fn get_filter_changes(&self, filter_id: U256) -> Result<FilterChanges> {
        let filter_id_logs =
            U256::from_str(&read_to_string(self.path.join("filter_id_logs.txt"))?)?;
        let filter_id_blocks =
            U256::from_str(&read_to_string(self.path.join("filter_id_blocks.txt"))?)?;
        let filter_id_txs = U256::from_str(&read_to_string(self.path.join("filter_id_txs.txt"))?)?;

        match filter_id {
            id if id == filter_id_logs => {
                let logs = read_to_string(self.path.join("logs.json"))?;
                Ok(FilterChanges::Logs(serde_json::from_str(&logs)?))
            }
            id if id == filter_id_blocks => {
                let hashes = read_to_string(self.path.join("block_hashes.json"))?;
                Ok(FilterChanges::Hashes(serde_json::from_str(&hashes)?))
            }
            id if id == filter_id_txs => {
                let hashes = read_to_string(self.path.join("tx_hashes.json"))?;
                Ok(FilterChanges::Hashes(serde_json::from_str(&hashes)?))
            }
            _ => Err(ExecutionError::FilterNotFound(filter_id).into()),
        }
    }

    async fn get_filter_logs(&self, filter_id: U256) -> Result<Vec<Log>> {
        let filter_id_logs =
            U256::from_str(&read_to_string(self.path.join("filter_id_logs.txt"))?)?;

        if filter_id == filter_id_logs {
            let logs = read_to_string(self.path.join("logs.json"))?;
            Ok(serde_json::from_str(&logs)?)
        } else {
            Err(ExecutionError::FilterNotFound(filter_id).into())
        }
    }

    async fn uninstall_filter(&self, filter_id: U256) -> Result<bool> {
        let id = read_to_string(self.path.join("filter_id_logs.txt"))?;
        let id = U256::from_str(&id)?;
        Ok(id == filter_id)
    }

    async fn new_filter(&self, _filter: &Filter) -> Result<U256> {
        let id = read_to_string(self.path.join("filter_id_logs.txt"))?;
        Ok(U256::from_str(&id)?)
    }

    async fn new_block_filter(&self) -> Result<U256> {
        let id = read_to_string(self.path.join("filter_id_blocks.txt"))?;
        Ok(U256::from_str(&id)?)
    }

    async fn new_pending_transaction_filter(&self) -> Result<U256> {
        let id = read_to_string(self.path.join("filter_id_txs.txt"))?;
        Ok(U256::from_str(&id)?)
    }

    async fn chain_id(&self) -> Result<u64> {
        let id = read_to_string(self.path.join("chain_id.txt"))?;
        Ok(u64::from_str(&id)?)
    }

    async fn get_block(
        &self,
        _block_id: BlockId,
        _txs_kind: BlockTransactionsKind,
    ) -> Result<Option<N::BlockResponse>> {
        let block = read_to_string(self.path.join("block.json"))?;
        let block = serde_json::from_str(&block)?;
        Ok(Some(block))
    }

    async fn get_fee_history(
        &self,
        _block_count: u64,
        _last_block: u64,
        _reward_percentiles: &[f64],
    ) -> Result<FeeHistory> {
        let fee_history = read_to_string(self.path.join("fee_history.json"))?;
        Ok(serde_json::from_str(&fee_history)?)
    }
}
