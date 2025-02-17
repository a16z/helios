use std::collections::HashMap;

use alloy::consensus::BlockHeader;
use alloy::eips::BlockId;
use alloy::network::{BlockResponse, ReceiptResponse};
use alloy::primitives::{keccak256, Address, B256, U256};
use alloy::rlp;
use alloy::rpc::types::{EIP1186AccountProofResponse, Filter, FilterChanges, Log};
use alloy_trie::KECCAK_EMPTY;
use async_trait::async_trait;
use eyre::Result;

use helios_common::{
    network_spec::NetworkSpec,
    types::{Account, BlockTag},
};
use helios_verifiable_api_client::{types::*, VerifiableApi};

use crate::execution::errors::ExecutionError;
use crate::execution::proof::{verify_account_proof, verify_receipt_proof, verify_storage_proof};
use crate::execution::rpc::ExecutionRpc;
use crate::execution::state::State;
use crate::execution::verified_client::VerifiableMethods;

#[derive(Clone)]
pub struct VerifiableMethodsApi<N: NetworkSpec, R: ExecutionRpc<N>, A: VerifiableApi<N>> {
    api: A,
    state: State<N, R>,
}

#[async_trait]
impl<N: NetworkSpec, R: ExecutionRpc<N>, A: VerifiableApi<N>> VerifiableMethods<N, R>
    for VerifiableMethodsApi<N, R, A>
{
    fn new(url: &str, state: State<N, R>) -> Result<Self> {
        let api: A = VerifiableApi::new(url);
        Ok(Self { api, state })
    }

    async fn get_account(
        &self,
        address: Address,
        slots: Option<&[B256]>,
        tag: BlockTag,
    ) -> Result<Account> {
        let block = self
            .state
            .get_block(tag)
            .await
            .ok_or(ExecutionError::BlockNotFound(tag))?;
        let block_id = BlockId::number(block.header().number());
        let slots = slots
            .unwrap_or(&[])
            .into_iter()
            .map(|s| (*s).into())
            .collect::<Vec<_>>();

        let account_response = self
            .api
            .get_account(address, &slots, Some(block_id))
            .await?;

        self.verify_account_response(address, account_response, &block)
    }

    async fn get_transaction_receipt(&self, tx_hash: B256) -> Result<Option<N::ReceiptResponse>> {
        let tx_receipt_response = self.api.get_transaction_receipt(tx_hash).await?;
        if tx_receipt_response.is_none() {
            return Ok(None);
        }
        let tx_receipt_response = tx_receipt_response.unwrap();

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
}

impl<N: NetworkSpec, R: ExecutionRpc<N>, A: VerifiableApi<N>> VerifiableMethodsApi<N, R, A> {
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
        // Verify the code hash
        let code = if proof.code_hash == KECCAK_EMPTY || proof.code_hash == B256::ZERO {
            Vec::new()
        } else {
            let code_hash = keccak256(&account.code);

            if proof.code_hash != code_hash {
                return Err(
                    ExecutionError::CodeHashMismatch(address, code_hash, proof.code_hash).into(),
                );
            }

            account.code.into()
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
        for log in logs {
            let tx_hash = log.transaction_hash.unwrap();

            let TransactionReceiptResponse {
                receipt,
                receipt_proof: _,
            } = receipt_proofs
                .get(&tx_hash)
                .ok_or(ExecutionError::NoReceiptForTransaction(tx_hash))?;

            let encoded_log = rlp::encode(&log.inner);

            if N::receipt_logs(receipt)
                .into_iter()
                .map(|l| rlp::encode(&l.inner))
                .find(|el| *el == encoded_log)
                .is_none()
            {
                return Err(ExecutionError::MissingLog(
                    tx_hash,
                    U256::from(log.log_index.unwrap()),
                )
                .into());
            }
        }

        self.verify_receipt_proofs(&receipt_proofs.values().collect::<Vec<_>>())
            .await?;

        Ok(())
    }

    async fn verify_receipt_proofs(
        &self,
        receipt_proofs: &[&TransactionReceiptResponse<N>],
    ) -> Result<()> {
        let mut blocks: HashMap<u64, N::BlockResponse> = HashMap::new();

        for TransactionReceiptResponse {
            receipt,
            receipt_proof,
        } in receipt_proofs
        {
            let block_num = receipt.block_number().unwrap();
            let block = if let Some(block) = blocks.get(&block_num) {
                block.clone()
            } else {
                let tag = BlockTag::Number(block_num);
                let block = self
                    .state
                    .get_block(tag)
                    .await
                    .ok_or(ExecutionError::BlockNotFound(tag))?;
                blocks.insert(block_num, block.clone());
                block
            };

            verify_receipt_proof::<N>(receipt, block.header().receipts_root(), receipt_proof)
                .map_err(|_| ExecutionError::ReceiptRootMismatch(receipt.transaction_hash()))?;
        }

        Ok(())
    }
}
