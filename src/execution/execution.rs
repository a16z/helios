use ethers::prelude::{Address, H256, U256};
use ethers::utils::keccak256;
use eyre::Result;

use super::proof::{encode_account, verify_proof};
use super::rpc::Rpc;
use crate::consensus::types::ExecutionPayload;

pub struct ExecutionClient {
    rpc: Rpc,
}

impl ExecutionClient {
    pub fn new(rpc: &str) -> Self {
        let rpc = Rpc::new(rpc);
        ExecutionClient { rpc }
    }

    pub async fn get_account(
        &self,
        address: &Address,
        payload: &ExecutionPayload,
    ) -> Result<Account> {
        let proof = self.rpc.get_proof(&address, payload.block_number).await?;

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

    pub async fn get_code(&self, address: &Address, payload: &ExecutionPayload) -> Result<Vec<u8>> {
        let account = self.get_account(address, payload).await?;
        let code = self.rpc.get_code(address, payload.block_number).await?;

        let code_hash = keccak256(&code).into();

        if account.code_hash != code_hash {
            eyre::bail!("Invalid Proof");
        }

        Ok(code)
    }
}

pub struct Account {
    pub balance: U256,
    pub nonce: U256,
    pub code_hash: H256,
    pub storage_hash: H256,
}
