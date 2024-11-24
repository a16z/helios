use std::{fs::read_to_string, path::PathBuf};

use alloy::primitives::{Address, B256, U256};
use alloy::rpc::types::{
    AccessList, BlockId, EIP1186AccountProofResponse, FeeHistory, Filter, Log,
};
use async_trait::async_trait;
use eyre::{eyre, Result};

use super::ExecutionRpc;
use crate::network_spec::NetworkSpec;
use crate::types::{Block, BlockTag};

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
        _address: Address,
        _slots: &[B256],
        _block: BlockId,
    ) -> Result<EIP1186AccountProofResponse> {
        let proof = read_to_string(self.path.join("proof.json"))?;
        Ok(serde_json::from_str(&proof)?)
    }

    async fn create_access_list(
        &self,
        _opts: &N::TransactionRequest,
        _block: BlockTag,
    ) -> Result<AccessList> {
        Err(eyre!("not implemented"))
    }

    async fn get_code(&self, _address: Address, _block: u64) -> Result<Vec<u8>> {
        let code = read_to_string(self.path.join("code.json"))?;
        Ok(hex::decode(&code[2..code.len() - 1])?)
    }

    async fn send_raw_transaction(&self, _bytes: &[u8]) -> Result<B256> {
        Err(eyre!("not implemented"))
    }

    async fn get_transaction_receipt(&self, _tx_hash: B256) -> Result<Option<N::ReceiptResponse>> {
        let receipt = read_to_string(self.path.join("receipt.json"))?;
        Ok(serde_json::from_str(&receipt)?)
    }

    async fn get_block_receipts(
        &self,
        _block: BlockTag,
    ) -> Result<Option<Vec<N::ReceiptResponse>>> {
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

    async fn get_filter_changes(&self, _filter_id: U256) -> Result<Vec<Log>> {
        let logs = read_to_string(self.path.join("logs.json"))?;
        Ok(serde_json::from_str(&logs)?)
    }

    async fn get_filter_logs(&self, _filter_id: U256) -> Result<Vec<Log>> {
        let logs = read_to_string(self.path.join("logs.json"))?;
        Ok(serde_json::from_str(&logs)?)
    }

    async fn uninstall_filter(&self, _filter_id: U256) -> Result<bool> {
        Err(eyre!("not implemented"))
    }

    async fn get_new_filter(&self, _filter: &Filter) -> Result<U256> {
        Err(eyre!("not implemented"))
    }

    async fn get_new_block_filter(&self) -> Result<U256> {
        Err(eyre!("not implemented"))
    }

    async fn get_new_pending_transaction_filter(&self) -> Result<U256> {
        Err(eyre!("not implemented"))
    }

    async fn chain_id(&self) -> Result<u64> {
        Err(eyre!("not implemented"))
    }

    async fn get_block(&self, _hash: B256) -> Result<Block<N::TransactionResponse>> {
        Err(eyre!("not implemented"))
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
