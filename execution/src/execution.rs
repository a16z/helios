use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;

use ethers::abi::AbiEncode;
use ethers::prelude::{Address, U256};
use ethers::types::{Filter, Log, Transaction, TransactionReceipt, H256};
use ethers::utils::keccak256;
use ethers::utils::rlp::{encode, Encodable, RlpStream};
use eyre::Result;

use common::utils::hex_str_to_bytes;
use consensus::types::ExecutionPayload;
use futures::future::join_all;
use revm::KECCAK_EMPTY;
use triehash_ethereum::ordered_trie_root;

use crate::errors::ExecutionError;
use crate::rpc::WsRpc;
use crate::types::Transactions;

use super::proof::{encode_account, verify_proof};
use super::rpc::ExecutionRpc;
use super::types::{Account, ExecutionBlock};

// We currently limit the max number of logs to fetch,
// to avoid blocking the client for too long.
const MAX_SUPPORTED_LOGS_NUMBER: usize = 5;

#[derive(Clone)]
pub struct ExecutionClient<R: ExecutionRpc> {
    pub rpc: R,
}

impl ExecutionClient<WsRpc> {
    pub fn new_with_ws(rpc: &str) -> Result<Self> {
        let rpc = WsRpc::new(rpc)?;
        Ok(Self { rpc })
    }
}

impl<R: ExecutionRpc> ExecutionClient<R> {
    pub fn new(rpc: &str) -> Result<Self> {
        let rpc = ExecutionRpc::new(rpc)?;
        Ok(ExecutionClient { rpc })
    }

    pub async fn get_account(
        &self,
        address: &Address,
        slots: Option<&[H256]>,
        payload: &ExecutionPayload,
    ) -> Result<Account> {
        let slots = slots.unwrap_or(&[]);

        let proof = self
            .rpc
            .get_proof(address, slots, payload.block_number)
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
            return Err(ExecutionError::InvalidAccountProof(*address).into());
        }

        let mut slot_map = HashMap::new();

        for storage_proof in proof.storage_proof {
            let key = hex_str_to_bytes(&storage_proof.key.encode_hex())?;
            let value = encode(&storage_proof.value).to_vec();

            let key_hash = keccak256(key);

            let is_valid = verify_proof(
                &storage_proof.proof,
                proof.storage_hash.as_bytes(),
                &key_hash.to_vec(),
                &value,
            );

            if !is_valid {
                return Err(
                    ExecutionError::InvalidStorageProof(*address, storage_proof.key).into(),
                );
            }

            slot_map.insert(storage_proof.key, storage_proof.value);
        }

        let code = if proof.code_hash == KECCAK_EMPTY {
            Vec::new()
        } else {
            let code = self.rpc.get_code(address, payload.block_number).await?;
            let code_hash = keccak256(&code).into();

            if proof.code_hash != code_hash {
                return Err(ExecutionError::CodeHashMismatch(
                    *address,
                    code_hash.to_string(),
                    proof.code_hash.to_string(),
                )
                .into());
            }

            code
        };

        Ok(Account {
            balance: proof.balance,
            nonce: proof.nonce.as_u64(),
            code,
            code_hash: proof.code_hash,
            storage_hash: proof.storage_hash,
            slots: slot_map,
        })
    }

    pub async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<H256> {
        self.rpc.send_raw_transaction(bytes).await
    }

    pub async fn get_block(
        &self,
        payload: &ExecutionPayload,
        full_tx: bool,
    ) -> Result<ExecutionBlock> {
        let empty_nonce = "0x0000000000000000".to_string();
        let empty_uncle_hash = "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347";

        let tx_hashes = payload
            .transactions
            .iter()
            .map(|tx| H256::from_slice(&keccak256(tx)))
            .collect::<Vec<H256>>();

        let txs = if full_tx {
            let txs_fut = tx_hashes.iter().map(|hash| async move {
                let mut payloads = BTreeMap::new();
                payloads.insert(payload.block_number, payload.clone());
                let tx = self
                    .get_transaction(hash, &payloads)
                    .await?
                    .ok_or(eyre::eyre!("not reachable"))?;

                Ok(tx)
            });

            let txs = join_all(txs_fut)
                .await
                .into_iter()
                .collect::<Result<Vec<_>>>()?;
            Transactions::Full(txs)
        } else {
            Transactions::Hashes(tx_hashes)
        };

        Ok(ExecutionBlock {
            number: payload.block_number,
            base_fee_per_gas: U256::from_little_endian(&payload.base_fee_per_gas.to_bytes_le()),
            difficulty: U256::from(0),
            extra_data: payload.extra_data.to_vec(),
            gas_limit: payload.gas_limit,
            gas_used: payload.gas_used,
            hash: H256::from_slice(&payload.block_hash),
            logs_bloom: payload.logs_bloom.to_vec(),
            miner: Address::from_slice(&payload.fee_recipient),
            parent_hash: H256::from_slice(&payload.parent_hash),
            receipts_root: H256::from_slice(&payload.receipts_root),
            state_root: H256::from_slice(&payload.state_root),
            timestamp: payload.timestamp,
            total_difficulty: 0,
            transactions: txs,
            mix_hash: H256::from_slice(&payload.prev_randao),
            nonce: empty_nonce,
            sha3_uncles: H256::from_str(empty_uncle_hash)?,
            size: 0,
            transactions_root: H256::default(),
            uncles: vec![],
        })
    }

    pub async fn get_transaction_receipt(
        &self,
        tx_hash: &H256,
        payloads: &BTreeMap<u64, ExecutionPayload>,
    ) -> Result<Option<TransactionReceipt>> {
        let receipt = self.rpc.get_transaction_receipt(tx_hash).await?;
        if receipt.is_none() {
            return Ok(None);
        }

        let receipt = receipt.unwrap();
        let block_number = receipt.block_number.unwrap().as_u64();
        let payload = payloads.get(&block_number);
        if payload.is_none() {
            return Ok(None);
        }

        let payload = payload.unwrap();

        let tx_hashes = payload
            .transactions
            .iter()
            .map(|tx| H256::from_slice(&keccak256(tx)))
            .collect::<Vec<H256>>();

        let receipts_fut = tx_hashes.iter().map(|hash| async move {
            let receipt = self.rpc.get_transaction_receipt(hash).await;
            receipt?.ok_or(eyre::eyre!("not reachable"))
        });

        let receipts = join_all(receipts_fut).await;
        let receipts = receipts.into_iter().collect::<Result<Vec<_>>>()?;

        let receipts_encoded: Vec<Vec<u8>> = receipts.iter().map(encode_receipt).collect();

        let expected_receipt_root = ordered_trie_root(receipts_encoded);
        let expected_receipt_root = H256::from_slice(&expected_receipt_root.to_fixed_bytes());
        let payload_receipt_root = H256::from_slice(&payload.receipts_root);

        if expected_receipt_root != payload_receipt_root || !receipts.contains(&receipt) {
            return Err(ExecutionError::ReceiptRootMismatch(tx_hash.to_string()).into());
        }

        Ok(Some(receipt))
    }

    pub async fn get_transaction(
        &self,
        hash: &H256,
        payloads: &BTreeMap<u64, ExecutionPayload>,
    ) -> Result<Option<Transaction>> {
        let tx = self.rpc.get_transaction(hash).await?;
        if tx.is_none() {
            return Ok(None);
        }

        let tx = tx.unwrap();

        let block_number = tx.block_number;
        if block_number.is_none() {
            return Ok(None);
        }

        let block_number = block_number.unwrap().as_u64();
        let payload = payloads.get(&block_number);
        if payload.is_none() {
            return Ok(None);
        }

        let payload = payload.unwrap();

        let tx_encoded = tx.rlp().to_vec();
        let txs_encoded = payload
            .transactions
            .iter()
            .map(|tx| tx.to_vec())
            .collect::<Vec<_>>();

        if !txs_encoded.contains(&tx_encoded) {
            return Err(ExecutionError::MissingTransaction(hash.to_string()).into());
        }

        Ok(Some(tx))
    }

    pub async fn get_logs(
        &self,
        filter: &Filter,
        payloads: &BTreeMap<u64, ExecutionPayload>,
    ) -> Result<Vec<Log>> {
        let logs = self.rpc.get_logs(filter).await?;
        if logs.len() > MAX_SUPPORTED_LOGS_NUMBER {
            return Err(
                ExecutionError::TooManyLogsToProve(logs.len(), MAX_SUPPORTED_LOGS_NUMBER).into(),
            );
        }

        for (_pos, log) in logs.iter().enumerate() {
            // For every log
            // Get the hash of the tx that generated it
            let tx_hash = log
                .transaction_hash
                .ok_or(eyre::eyre!("tx hash not found in log"))?;
            // Get its proven receipt
            let receipt = self
                .get_transaction_receipt(&tx_hash, payloads)
                .await?
                .ok_or(ExecutionError::NoReceiptForTransaction(tx_hash.to_string()))?;

            // Check if the receipt contains the desired log
            // Encoding logs for comparison
            let receipt_logs_encoded = receipt
                .logs
                .iter()
                .map(|log| log.rlp_bytes())
                .collect::<Vec<_>>();

            let log_encoded = log.rlp_bytes();

            if !receipt_logs_encoded.contains(&log_encoded) {
                return Err(ExecutionError::MissingLog(
                    tx_hash.to_string(),
                    log.log_index.unwrap(),
                )
                .into());
            }
        }
        Ok(logs)
    }
}

fn encode_receipt(receipt: &TransactionReceipt) -> Vec<u8> {
    let mut stream = RlpStream::new();
    stream.begin_list(4);
    stream.append(&receipt.status.unwrap());
    stream.append(&receipt.cumulative_gas_used);
    stream.append(&receipt.logs_bloom);
    stream.append_list(&receipt.logs);

    let legacy_receipt_encoded = stream.out();
    let tx_type = receipt.transaction_type.unwrap().as_u64();

    match tx_type {
        0 => legacy_receipt_encoded.to_vec(),
        _ => [&tx_type.to_be_bytes()[7..8], &legacy_receipt_encoded].concat(),
    }
}
