use std::{fs::read_to_string, path::PathBuf};

use async_trait::async_trait;
use common::utils::hex_str_to_bytes;
use ethers::types::{
    transaction::eip2930::AccessList, Address, EIP1186ProofResponse, Filter, Log, Transaction,
    TransactionReceipt, H256,
};
use eyre::{eyre, Result};

use crate::types::CallOpts;

use super::ExecutionRpc;

#[derive(Clone)]
pub struct MockRpc {
    path: PathBuf,
}

#[async_trait(?Send)]
impl ExecutionRpc for MockRpc {
    fn new(rpc: &str) -> Result<Self> {
        let path = PathBuf::from(rpc);
        Ok(MockRpc { path })
    }

    async fn get_proof(
        &self,
        _address: &Address,
        _slots: &[H256],
        _block: u64,
    ) -> Result<EIP1186ProofResponse> {
        let proof = read_to_string(self.path.join("proof.json"))?;
        Ok(serde_json::from_str(&proof)?)
    }

    async fn create_access_list(&self, _opts: &CallOpts, _block: u64) -> Result<AccessList> {
        Err(eyre!("not implemented"))
    }

    async fn get_code(&self, _address: &Address, _block: u64) -> Result<Vec<u8>> {
        let code = read_to_string(self.path.join("code.json"))?;
        hex_str_to_bytes(&code[0..code.len() - 1])
    }

    async fn send_raw_transaction(&self, _bytes: &Vec<u8>) -> Result<H256> {
        Err(eyre!("not implemented"))
    }

    async fn get_transaction_receipt(&self, _tx_hash: &H256) -> Result<Option<TransactionReceipt>> {
        let receipt = read_to_string(self.path.join("receipt.json"))?;
        Ok(serde_json::from_str(&receipt)?)
    }

    async fn get_transaction(&self, _tx_hash: &H256) -> Result<Option<Transaction>> {
        let tx = read_to_string(self.path.join("transaction.json"))?;
        Ok(serde_json::from_str(&tx)?)
    }

    async fn get_logs(&self, _filter: &Filter) -> Result<Vec<Log>> {
        let logs = read_to_string(self.path.join("logs.json"))?;
        Ok(serde_json::from_str(&logs)?)
    }
}
