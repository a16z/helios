use ethers::prelude::{Address, U256};
use ethers::utils::keccak256;
use eyre::Result;

use crate::consensus_rpc::ExecutionPayload;
use crate::execution_rpc::ExecutionRpc;
use crate::proof::{encode_account, verify_proof};

pub struct ExecutionClient {
    execution_rpc: ExecutionRpc,
}

impl ExecutionClient {
    pub fn new(rpc: &str) -> Self {
        let execution_rpc = ExecutionRpc::new(rpc);
        ExecutionClient { execution_rpc }
    }

    pub async fn get_balance(&self, account: &Address, payload: &ExecutionPayload) -> Result<U256> {
        let proof = self
            .execution_rpc
            .get_proof(&account, payload.block_number)
            .await?;

        let account_path = keccak256(account.as_bytes()).to_vec();
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

        Ok(proof.balance)
    }
}
