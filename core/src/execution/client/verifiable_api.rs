use std::collections::{hash_map::Entry, HashMap};

use alloy::consensus::BlockHeader;
use alloy::eips::BlockId;
use alloy::network::{BlockResponse, ReceiptResponse};
use alloy::primitives::{Address, B256, U256};
use alloy::rlp;
use alloy::rpc::types::{EIP1186AccountProofResponse, Filter, FilterChanges, Log};
use async_trait::async_trait;
use eyre::Result;

use helios_common::{
    network_spec::NetworkSpec,
    types::{Account, BlockTag},
};
use helios_verifiable_api_client::{types::*, VerifiableApi};

use crate::execution::errors::ExecutionError;
use crate::execution::proof::{
    verify_account_proof, verify_block_receipts, verify_code_hash_proof, verify_receipt_proof,
    verify_storage_proof,
};
use crate::execution::state::State;

use super::{ExecutionInner, ExecutionSpec};

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
        let api = A::new(url);
        Ok(Self { api, state })
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec, A: VerifiableApi<N>> ExecutionSpec<N>
    for ExecutionInnerVerifiableApiClient<N, A>
{
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

        let account = self
            .api
            .get_account(address, &slots, Some(block_id), include_code)
            .await?;

        self.verify_account(address, &account, &block)?;

        Ok(account)
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

    async fn create_extended_access_list(
        &self,
        tx: &N::TransactionRequest,
        block_id: Option<BlockId>,
    ) -> Result<HashMap<Address, Account>> {
        let block_id = block_id.unwrap_or_default();
        let tag = BlockTag::try_from(block_id)?;
        let block = self
            .state
            .get_block(tag)
            .await
            .ok_or(ExecutionError::BlockNotFound(tag))?;
        let block_id = BlockId::Number(block.header().number().into());

        let ExtendedAccessListResponse { accounts } = self
            .api
            .create_extended_access_list(tx.clone(), Some(block_id))
            .await?;

        for (address, account) in &accounts {
            self.verify_account(*address, account, &block)?;
        }

        Ok(accounts)
    }

    async fn chain_id(&self) -> Result<u64> {
        let ChainIdResponse { chain_id } = self.api.chain_id().await?;
        Ok(chain_id)
    }

    async fn get_block(
        &self,
        block_id: BlockId,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        self.state.get_block_by_id(block_id, full_tx).await
    }

    async fn get_untrusted_block(
        &self,
        block_id: BlockId,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        self.api.get_block(block_id, full_tx).await
    }

    async fn get_block_receipts(
        &self,
        block_id: BlockId,
    ) -> Result<Option<Vec<N::ReceiptResponse>>> {
        let block = self.state.get_block_by_id(block_id, false).await?;
        let Some(block) = block else {
            return Ok(None);
        };
        let block_num = block.header().number();

        let receipts = self
            .api
            .get_block_receipts(block_num.into())
            .await?
            .ok_or(ExecutionError::NoReceiptsForBlock(block_num.into()))?;

        verify_block_receipts::<N>(&receipts, &block)?;

        Ok(Some(receipts))
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
    fn verify_account(
        &self,
        address: Address,
        account: &AccountResponse,
        block: &N::BlockResponse,
    ) -> Result<()> {
        let proof = EIP1186AccountProofResponse {
            address,
            balance: account.account.balance,
            code_hash: account.account.code_hash,
            nonce: account.account.nonce,
            storage_hash: account.account.storage_root,
            account_proof: account.account_proof.clone(),
            storage_proof: account.storage_proof.clone(),
        };
        // Verify the account proof
        verify_account_proof(&proof, block.header().state_root())?;
        // Verify the storage proofs
        verify_storage_proof(&proof)?;
        // Verify the code hash (if code is included in the response)
        if let Some(code) = &account.code {
            verify_code_hash_proof(&proof, code)?;
        }

        Ok(())
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

            if let Entry::Vacant(e) = txhash_encodedlogs_map.entry(tx_hash) {
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
                e.insert(encoded_logs);
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use alloy::rpc::types::TransactionRequest;
    use helios_ethereum::spec::Ethereum as EthereumSpec;
    use helios_test_utils::*;
    use helios_verifiable_api_client::mock::MockVerifiableApi;

    use super::*;

    async fn get_client() -> ExecutionInnerVerifiableApiClient<EthereumSpec, MockVerifiableApi> {
        let state = State::<EthereumSpec>::new(1);
        let client = ExecutionInnerVerifiableApiClient::<EthereumSpec, MockVerifiableApi>::new(
            testdata_dir().to_str().unwrap(),
            state.clone(),
        )
        .unwrap();
        let block = rpc_block();
        state.push_block(block, Arc::new(client.clone())).await;
        client
    }

    #[tokio::test]
    async fn test_get_account() {
        let client = get_client().await;
        let rpc_proof = rpc_proof();
        let block = rpc_block();

        let response = client
            .get_account(
                rpc_proof.address,
                Some(&[rpc_proof.storage_proof[0].key.as_b256()]),
                BlockTag::Number(block.header().number()),
                true,
            )
            .await
            .unwrap();

        assert_eq!(response, rpc_account());
    }

    #[tokio::test]
    async fn test_get_account_without_code() {
        let client = get_client().await;
        let rpc_proof = rpc_proof();
        let block = rpc_block();

        let response = client
            .get_account(
                rpc_proof.address,
                Some(&[rpc_proof.storage_proof[0].key.as_b256()]),
                BlockTag::Number(block.header().number()),
                false,
            )
            .await
            .unwrap();

        let mut expected_account = rpc_account();
        expected_account.code = None;
        assert_eq!(response, expected_account);
    }

    #[tokio::test]
    async fn test_get_account_block_not_in_state() {
        let client = get_client().await;
        let rpc_proof = rpc_proof();

        let response = client
            .get_account(rpc_proof.address, None, BlockTag::Finalized, true)
            .await;

        assert_eq!(
            response.unwrap_err().to_string(),
            ExecutionError::BlockNotFound(BlockTag::Finalized).to_string()
        );
    }

    #[tokio::test]
    async fn test_get_transaction_receipt() {
        let client = get_client().await;
        let rpc_tx_receipt = rpc_tx_receipt();

        let response = client
            .get_transaction_receipt(rpc_tx_receipt.transaction_hash)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(response, rpc_tx_receipt);
    }

    #[tokio::test]
    async fn test_get_logs() {
        let client = get_client().await;
        let filter = Filter::default();

        let response = client.get_logs(&filter).await.unwrap();

        assert_eq!(response, rpc_logs());
    }

    #[tokio::test]
    async fn test_get_filter_changes_logs() {
        let client = get_client().await;
        let filter_id = rpc_filter_id_logs();

        let response = client.get_filter_changes(filter_id).await.unwrap();
        let response = match response {
            FilterChanges::Logs(response) => response,
            _ => panic!("Expected FilterChanges::Logs"),
        };

        assert_eq!(response, rpc_logs());
    }

    #[tokio::test]
    async fn test_get_filter_changes_blocks() {
        let client = get_client().await;
        let filter_id = rpc_filter_id_blocks();

        let response = client.get_filter_changes(filter_id).await.unwrap();
        let response = match response {
            FilterChanges::Hashes(response) => response,
            _ => panic!("Expected FilterChanges::Hashes"),
        };

        assert_eq!(response, rpc_filter_block_hashes());
    }

    #[tokio::test]
    async fn test_get_filter_changes_txs() {
        let client = get_client().await;
        let filter_id = rpc_filter_id_txs();

        let response = client.get_filter_changes(filter_id).await.unwrap();
        let response = match response {
            FilterChanges::Hashes(response) => response,
            _ => panic!("Expected FilterChanges::Hashes"),
        };

        assert_eq!(response, rpc_filter_tx_hashes());
    }

    #[tokio::test]
    async fn test_get_filter_logs() {
        let client = get_client().await;
        let filter_id = rpc_filter_id_logs();

        let response = client.get_filter_logs(filter_id).await.unwrap();

        assert_eq!(response, rpc_logs());
    }

    #[tokio::test]
    async fn test_create_extended_access_list() {
        let client = get_client().await;
        let address = rpc_proof().address;
        let tx = TransactionRequest::default().from(address).to(address);

        let response = client
            .create_extended_access_list(&tx, BlockId::latest().into())
            .await
            .unwrap();

        assert_eq!(response.len(), 1);
        assert_eq!(response.get(&address).unwrap(), &rpc_account());
    }

    #[tokio::test]
    async fn test_chain_id() {
        let client = get_client().await;

        let response = client.chain_id().await.unwrap();

        assert_eq!(response, rpc_chain_id());
    }

    #[tokio::test]
    async fn test_get_block() {
        let client = get_client().await;

        let response = client
            .get_block(BlockId::latest(), false)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(response, rpc_block());
    }

    #[tokio::test]
    async fn test_get_untrusted_block() {
        let client = get_client().await;

        let response = client
            .get_untrusted_block(BlockId::latest(), false)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(response, rpc_block());
    }

    #[tokio::test]
    async fn test_get_block_receipts() {
        let client = get_client().await;

        let response = client
            .get_block_receipts(BlockId::latest())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(response, rpc_block_receipts());
    }

    #[tokio::test]
    async fn test_send_raw_transaction() {
        let client = get_client().await;

        let response = client.send_raw_transaction(&[]).await.unwrap();

        assert_eq!(response, *rpc_tx().inner.tx_hash());
    }

    #[tokio::test]
    async fn test_new_filter() {
        let client = get_client().await;
        let filter = Filter::default();

        let response = client.new_filter(&filter).await.unwrap();

        assert_eq!(response, rpc_filter_id_logs());
    }

    #[tokio::test]
    async fn test_new_block_filter() {
        let client = get_client().await;

        let response = client.new_block_filter().await.unwrap();

        assert_eq!(response, rpc_filter_id_blocks());
    }

    #[tokio::test]
    async fn test_new_pending_transaction_filter() {
        let client = get_client().await;

        let response = client.new_pending_transaction_filter().await.unwrap();

        assert_eq!(response, rpc_filter_id_txs());
    }

    #[tokio::test]
    async fn test_uninstall_filter() {
        let client = get_client().await;

        let response = client.uninstall_filter(rpc_filter_id_logs()).await.unwrap();

        assert_eq!(response, true);
    }
}
