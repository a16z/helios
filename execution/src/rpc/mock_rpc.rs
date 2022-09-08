use std::{fs::read_to_string, path::PathBuf};

use async_trait::async_trait;
use common::utils::hex_str_to_bytes;
use ethers::types::{Address, EIP1186ProofResponse, Transaction, TransactionReceipt, H256};
use eyre::Result;

use super::Rpc;

#[derive(Clone)]
pub struct MockRpc {
    path: PathBuf,
}

#[async_trait]
impl Rpc for MockRpc {
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

    async fn get_code(&self, _address: &Address, _block: u64) -> Result<Vec<u8>> {
        let code = read_to_string(self.path.join("code.json"))?;
        hex_str_to_bytes(&code[0..code.len() - 1])
    }

    async fn send_raw_transaction(&self, _bytes: &Vec<u8>) -> Result<H256> {
        Err(eyre::eyre!("not implemented"))
    }

    async fn get_transaction_receipt(&self, _tx_hash: &H256) -> Result<Option<TransactionReceipt>> {
        let receipt = read_to_string(self.path.join("receipt.json"))?;
        Ok(serde_json::from_str(&receipt)?)
    }

    async fn get_transaction(&self, _tx_hash: &H256) -> Result<Option<Transaction>> {
        let tx = read_to_string(self.path.join("transaction.json"))?;
        Ok(serde_json::from_str(&tx)?)
    }
}
