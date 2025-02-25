use std::collections::HashMap;

use alloy::consensus::BlockHeader;
use alloy::eips::BlockId;
use alloy::network::{BlockResponse, ReceiptResponse};
use alloy::primitives::{keccak256, Address, B256, U256};
use alloy::rlp;
use alloy::rpc::types::{EIP1186AccountProofResponse, Filter, FilterChanges, Log};
use async_trait::async_trait;
use eyre::Result;

use helios_common::{
    network_spec::NetworkSpec,
    types::{Account, BlockTag},
};
use helios_verifiable_api_client::{types::*, VerifiableApi};

use crate::execution::client::ExecutionInner;
use crate::execution::errors::ExecutionError;
use crate::execution::proof::{verify_account_proof, verify_receipt_proof, verify_storage_proof};
use crate::execution::state::State;

#[derive(Clone)]
pub struct ExecutionInnerVerifiableApiClient<N: NetworkSpec, A: VerifiableApi<N>> {
    api: A,
    state: State<N>,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec, A: VerifiableApi<N>> ExecutionInner<N>
    for ExecutionInnerVerifiableApiClient<N, A>
{
    fn new(url: &str, state: State<N>) -> Result<Self> {
        let api: A = VerifiableApi::new(url);
        Ok(Self { api, state })
    }

    async fn get_account(
        &self,
        address: Address,
        slots: Option<&[B256]>,
        tag: BlockTag,
        include_code: bool,
    ) -> Result<Account> {
        let block = self
            .state
            .get_block(tag)
            .await
            .ok_or(ExecutionError::BlockNotFound(tag))?;
        let block_id = BlockId::number(block.header().number());
        let slots = slots
            .unwrap_or(&[])
            .iter()
            .map(|s| (*s).into())
            .collect::<Vec<_>>();

        let account_response = self
            .api
            .get_account(address, &slots, Some(block_id), include_code)
            .await?;

        self.verify_account_response(address, account_response, &block)
    }

    async fn get_transaction_receipt(&self, tx_hash: B256) -> Result<Option<N::ReceiptResponse>> {
        let Some(tx_receipt_response) = self.api.get_transaction_receipt(tx_hash).await? else {
            return Ok(None);
        };

        self.verify_receipt_proofs(&[&tx_receipt_response]).await?;

        Ok(Some(tx_receipt_response.receipt))
    }

    async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>> {
        let LogsResponse {
            logs,
            receipt_proofs,
        } = self.api.get_logs(filter).await?;

        self.verify_logs_and_receipts(&logs, receipt_proofs).await?;

        Ok(logs)
    }

    async fn get_filter_changes(&self, filter_id: U256) -> Result<FilterChanges> {
        let filter_changes = self.api.get_filter_changes(filter_id).await?;

        Ok(match filter_changes {
            FilterChangesResponse::Hashes(hashes) => FilterChanges::Hashes(hashes),
            FilterChangesResponse::Logs(FilterLogsResponse {
                logs,
                receipt_proofs,
            }) => {
                self.verify_logs_and_receipts(&logs, receipt_proofs).await?;
                FilterChanges::Logs(logs)
            }
        })
    }

    async fn get_filter_logs(&self, filter_id: U256) -> Result<Vec<Log>> {
        let FilterLogsResponse {
            logs,
            receipt_proofs,
        } = self.api.get_filter_logs(filter_id).await?;

        self.verify_logs_and_receipts(&logs, receipt_proofs).await?;

        Ok(logs)
    }

    async fn create_access_list(
        &self,
        tx: &N::TransactionRequest,
        block: Option<BlockId>,
    ) -> Result<HashMap<Address, Account>> {
        let block_id = block.unwrap_or(BlockId::latest());
        let tag = BlockTag::try_from(block_id)?;
        let block = self
            .state
            .get_block(tag)
            .await
            .ok_or(ExecutionError::BlockNotFound(tag))?;
        let block_id = BlockId::Number(block.header().number().into());

        let AccessListResponse { accounts } = self
            .api
            .create_access_list(tx.clone(), Some(block_id))
            .await?;

        let account_map = accounts
            .into_iter()
            .map(|(address, account_response)| {
                self.verify_account_response(address, account_response, &block)
                    .map(|account| (address, account))
            })
            .collect::<Result<HashMap<_, _>>>()?;

        Ok(account_map)
    }

    async fn chain_id(&self) -> Result<u64> {
        let ChainIdResponse { chain_id } = self.api.chain_id().await?;
        Ok(chain_id)
    }

    async fn get_block(&self, block: BlockId) -> Result<Option<N::BlockResponse>> {
        self.api.get_block(block).await
    }

    async fn get_block_receipts(&self, block: BlockId) -> Result<Option<Vec<N::ReceiptResponse>>> {
        self.api.get_block_receipts(block).await
    }

    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<B256> {
        let SendRawTxResponse { hash } = self.api.send_raw_transaction(bytes).await?;
        Ok(hash)
    }

    async fn new_filter(&self, filter: &Filter) -> Result<U256> {
        let NewFilterResponse { id, .. } = self.api.new_filter(filter).await?;
        Ok(id)
    }

    async fn new_block_filter(&self) -> Result<U256> {
        let NewFilterResponse { id, .. } = self.api.new_block_filter().await?;
        Ok(id)
    }

    async fn new_pending_transaction_filter(&self) -> Result<U256> {
        let NewFilterResponse { id, .. } = self.api.new_pending_transaction_filter().await?;
        Ok(id)
    }

    async fn uninstall_filter(&self, filter_id: U256) -> Result<bool> {
        let UninstallFilterResponse { ok } = self.api.uninstall_filter(filter_id).await?;
        Ok(ok)
    }
}

impl<N: NetworkSpec, A: VerifiableApi<N>> ExecutionInnerVerifiableApiClient<N, A> {
    fn verify_account_response(
        &self,
        address: Address,
        account: AccountResponse,
        block: &N::BlockResponse,
    ) -> Result<Account> {
        let proof = EIP1186AccountProofResponse {
            address,
            balance: account.account.balance,
            code_hash: account.account.code_hash,
            nonce: account.account.nonce,
            storage_hash: account.account.storage_root,
            account_proof: account.account_proof,
            storage_proof: account.storage_proof,
        };
        // Verify the account proof
        verify_account_proof(&proof, block.header().state_root())?;
        // Verify the storage proofs, collecting the slot values
        let slot_map = verify_storage_proof(&proof)?;
        // Verify the code hash (if code is included in the response)
        let code = match account.code {
            Some(code) => {
                let code_hash = keccak256(&code);
                if proof.code_hash != code_hash {
                    return Err(ExecutionError::CodeHashMismatch(
                        address,
                        code_hash,
                        proof.code_hash,
                    )
                    .into());
                }
                Some(code)
            }
            None => None,
        };

        Ok(Account {
            balance: proof.balance,
            nonce: proof.nonce,
            code_hash: proof.code_hash,
            code,
            storage_hash: proof.storage_hash,
            slots: slot_map,
        })
    }

    async fn verify_logs_and_receipts(
        &self,
        logs: &[Log],
        receipt_proofs: HashMap<B256, TransactionReceiptResponse<N>>,
    ) -> Result<()> {
        // Map of tx_hash -> encoded receipt logs to avoid encoding multiple times
        let mut txhash_encodedlogs_map: HashMap<B256, Vec<Vec<u8>>> = HashMap::new();

        // Verify each log entry exists in the corresponding receipt logs
        for log in logs {
            let tx_hash = log.transaction_hash.unwrap();
            let log_encoded = rlp::encode(&log.inner);

            if !txhash_encodedlogs_map.contains_key(&tx_hash) {
                let TransactionReceiptResponse {
                    receipt,
                    receipt_proof: _,
                } = receipt_proofs
                    .get(&tx_hash)
                    .ok_or(ExecutionError::NoReceiptForTransaction(tx_hash))?;
                let encoded_logs = N::receipt_logs(receipt)
                    .iter()
                    .map(|l| rlp::encode(&l.inner))
                    .collect::<Vec<_>>();
                txhash_encodedlogs_map.insert(tx_hash, encoded_logs);
            }
            let receipt_logs_encoded = txhash_encodedlogs_map.get(&tx_hash).unwrap();

            if !receipt_logs_encoded.contains(&log_encoded) {
                return Err(ExecutionError::MissingLog(
                    tx_hash,
                    U256::from(log.log_index.unwrap()),
                )
                .into());
            }
        }

        // Verify all receipts
        self.verify_receipt_proofs(&receipt_proofs.values().collect::<Vec<_>>())
            .await?;

        Ok(())
    }

    async fn verify_receipt_proofs(
        &self,
        receipt_proofs: &[&TransactionReceiptResponse<N>],
    ) -> Result<()> {
        for TransactionReceiptResponse {
            receipt,
            receipt_proof,
        } in receipt_proofs
        {
            let tag = BlockTag::Number(receipt.block_number().unwrap());
            let receipts_root = self
                .state
                .get_receipts_root(tag)
                .await
                .ok_or(ExecutionError::BlockNotFound(tag))?;

            verify_receipt_proof::<N>(receipt, receipts_root, receipt_proof)
                .map_err(|_| ExecutionError::ReceiptRootMismatch(receipt.transaction_hash()))?;
        }

        Ok(())
    }
}
