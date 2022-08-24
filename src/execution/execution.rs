use std::collections::HashMap;

use ethers::abi::AbiEncode;
use ethers::prelude::{Address, U256};
use ethers::utils::keccak256;
use ethers::utils::rlp::encode;
use eyre::Result;

use super::proof::{encode_account, verify_proof};
use super::rpc::Rpc;
use super::types::Account;
use crate::common::utils::hex_str_to_bytes;
use crate::consensus::types::ExecutionPayload;

#[derive(Clone)]
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
        slots: Option<&[U256]>,
        payload: &ExecutionPayload,
    ) -> Result<Account> {
        let slots = slots.unwrap_or(&[]);
        let proof = self
            .rpc
            .get_proof(&address, slots, payload.block_number)
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

        let mut slot_map = HashMap::new();

        for storage_proof in proof.storage_proof {
            let key = hex_str_to_bytes(&storage_proof.key.encode_hex())?;
            let value = encode(&storage_proof.value).to_vec();

            let key_hash = keccak256(key);

            let is_valid = verify_proof(
                &storage_proof.proof,
                &proof.storage_hash.as_bytes().to_vec(),
                &key_hash.to_vec(),
                &value,
            );

            if !is_valid {
                eyre::bail!("Invalid Proof");
            }

            slot_map.insert(storage_proof.key, storage_proof.value);
        }

        Ok(Account {
            balance: proof.balance,
            nonce: proof.nonce,
            code_hash: proof.code_hash,
            storage_hash: proof.storage_hash,
            slots: slot_map,
        })
    }

    pub async fn get_code(&self, address: &Address, payload: &ExecutionPayload) -> Result<Vec<u8>> {
        let account = self.get_account(address, None, payload).await?;
        let code = self.rpc.get_code(address, payload.block_number).await?;

        let code_hash = keccak256(&code).into();

        if account.code_hash != code_hash {
            eyre::bail!("Invalid Proof");
        }

        Ok(code)
    }
}
