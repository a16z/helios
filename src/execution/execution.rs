use ethers::prelude::{Address, U256, H256};
use ethers::utils::keccak256;
use eyre::Result;

use crate::consensus::types::ExecutionPayload;
use super::proof::{encode_account, verify_proof};
use super::execution_rpc::ExecutionRpc;

pub struct ExecutionClient {
    execution_rpc: ExecutionRpc,
}

impl ExecutionClient {
    pub fn new(rpc: &str) -> Self {
        let execution_rpc = ExecutionRpc::new(rpc);
        ExecutionClient { execution_rpc }
    }

    pub async fn get_account(&self, address: &Address, payload: &ExecutionPayload) -> Result<Account> {
        let proof = self
            .execution_rpc
            .get_proof(&address, payload.block_number)
            .await?;

        let account_path = keccak256(address.as_bytes()).to_vec();
        let account_encoded = encode_account(&proof);

        let is_valid = verify_proof(
            &proof.account_proof,
            &payload.state_root,
            &account_path,
            &account_encoded,
        );

        if !is_valid {
            eyre::bail!("Invalid Proof");
        }

        Ok(Account {
            balance: proof.balance,
            nonce: proof.nonce,
            code_hash: proof.code_hash,
            storage_hash: proof.storage_hash,
        })
    }
}

pub struct Account {
    pub balance: U256,
    pub nonce: U256,
    pub code_hash: H256,
    pub storage_hash: H256,
}
