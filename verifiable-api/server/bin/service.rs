use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::sync::Arc;

use alloy::{
    consensus::{Account, BlockHeader},
    eips::BlockNumberOrTag,
    network::{BlockResponse, ReceiptResponse, TransactionBuilder},
    primitives::{Address, B256, U256},
    rpc::types::{AccessListItem, BlockId, Filter, FilterChanges, Log},
};
use async_trait::async_trait;
use eyre::{Ok, OptionExt, Report, Result};
use futures::future::{join_all, try_join_all};

use helios_common::{network_spec::NetworkSpec, types::BlockTag};
use helios_core::execution::{
    constants::{MAX_SUPPORTED_BLOCKS_TO_PROVE_FOR_LOGS, PARALLEL_QUERY_BATCH_SIZE},
    errors::ExecutionError,
    proof::create_receipt_proof,
    rpc::ExecutionRpc,
};
use helios_verifiable_api_client::VerifiableApi;
use helios_verifiable_api_types::*;

#[derive(Clone)]
pub struct ApiService<N: NetworkSpec, R: ExecutionRpc<N>> {
    rpc: Arc<R>,
    _marker: PhantomData<N>,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec, R: ExecutionRpc<N>> VerifiableApi<N> for ApiService<N, R> {
    fn new(rpc: &str) -> Self {
        Self {
            rpc: Arc::new(R::new(rpc).unwrap()),
            _marker: Default::default(),
        }
    }

    async fn get_account(
        &self,
        address: Address,
        storage_slots: &[U256],
        block_id: Option<BlockId>,
        include_code: bool,
    ) -> Result<AccountResponse> {
        let block_id = block_id.unwrap_or_default();
        // make sure block ID is not tag but a number or hash
        let block_id = match block_id {
            BlockId::Number(number_or_tag) => match number_or_tag {
                BlockNumberOrTag::Number(_) => Ok(block_id),
                tag => Ok(BlockId::Number(
                    self.rpc
                        .get_block(tag.into(), false.into())
                        .await?
                        .ok_or_eyre(ExecutionError::BlockNotFound(tag.try_into()?))
                        .map(|block| block.header().number())?
                        .into(),
                )),
            },
            BlockId::Hash(_) => Ok(block_id),
        }?;

        let storage_keys = storage_slots
            .iter()
            .map(|key| (*key).into())
            .collect::<Vec<_>>();
        let proof = self.rpc.get_proof(address, &storage_keys, block_id).await?;

        let code = if include_code {
            Some(self.rpc.get_code(address, block_id).await?.into())
        } else {
            None
        };

        Ok(AccountResponse {
            account: Account {
                balance: proof.balance,
                nonce: proof.nonce,
                code_hash: proof.code_hash,
                storage_root: proof.storage_hash,
            },
            code,
            account_proof: proof.account_proof,
            storage_proof: proof.storage_proof,
        })
    }

    async fn get_transaction_receipt(
        &self,
        tx_hash: B256,
    ) -> Result<Option<TransactionReceiptResponse<N>>> {
        let Some(receipt) = self.rpc.get_transaction_receipt(tx_hash).await? else {
            return Ok(None);
        };

        let block_num = receipt.block_number().unwrap();
        let receipts = self.rpc.get_block_receipts(block_num.into()).await?.ok_or(
            ExecutionError::NoReceiptsForBlock(BlockTag::Number(block_num)),
        )?;

        let receipt_proof =
            create_receipt_proof::<N>(receipts, receipt.transaction_index().unwrap() as usize);

        Ok(Some(TransactionReceiptResponse {
            receipt,
            receipt_proof,
        }))
    }

    async fn get_logs(&self, filter: &Filter) -> Result<LogsResponse<N>> {
        let logs = self.rpc.get_logs(filter).await?;

        let receipt_proofs = self.create_receipt_proofs_for_logs(&logs).await?;

        Ok(LogsResponse {
            logs,
            receipt_proofs,
        })
    }

    async fn get_filter_logs(&self, filter_id: U256) -> Result<FilterLogsResponse<N>> {
        let logs = self.rpc.get_filter_logs(filter_id).await?;

        let receipt_proofs = self.create_receipt_proofs_for_logs(&logs).await?;

        Ok(FilterLogsResponse {
            logs,
            receipt_proofs,
        })
    }

    async fn get_filter_changes(&self, filter_id: U256) -> Result<FilterChangesResponse<N>> {
        let filter_changes = self.rpc.get_filter_changes(filter_id).await?;

        Ok(match filter_changes {
            FilterChanges::Logs(logs) => {
                let receipt_proofs = self.create_receipt_proofs_for_logs(&logs).await?;
                FilterChangesResponse::Logs(FilterLogsResponse {
                    logs,
                    receipt_proofs,
                })
            }
            FilterChanges::Hashes(hashes) => FilterChangesResponse::Hashes(hashes),
            FilterChanges::Empty => FilterChangesResponse::Hashes(vec![]),
            FilterChanges::Transactions(txs) => {
                FilterChangesResponse::Hashes(txs.into_iter().map(|t| *t.inner.tx_hash()).collect())
            }
        })
    }

    async fn create_access_list(
        &self,
        tx: N::TransactionRequest,
        block_id: Option<BlockId>,
    ) -> Result<AccessListResponse> {
        let block_id = block_id.unwrap_or_default();
        let block = self
            .rpc
            .get_block(block_id, false.into())
            .await?
            .ok_or_eyre(ExecutionError::BlockNotFound(block_id.try_into()?))?;

        let mut list = self.rpc.create_access_list(&tx, block_id).await?.0;

        let from_access_entry = AccessListItem {
            address: tx.from().unwrap_or_default(),
            storage_keys: Vec::default(),
        };
        let to_access_entry = AccessListItem {
            address: tx.to().unwrap_or_default(),
            storage_keys: Vec::default(),
        };
        let producer_access_entry = AccessListItem {
            address: block.header().beneficiary(),
            storage_keys: Vec::default(),
        };

        let list_addresses = list.iter().map(|elem| elem.address).collect::<Vec<_>>();

        if !list_addresses.contains(&from_access_entry.address) {
            list.push(from_access_entry)
        }
        if !list_addresses.contains(&to_access_entry.address) {
            list.push(to_access_entry)
        }
        if !list_addresses.contains(&producer_access_entry.address) {
            list.push(producer_access_entry)
        }

        let mut accounts = HashMap::new();
        for chunk in list.chunks(PARALLEL_QUERY_BATCH_SIZE) {
            let account_chunk_futs = chunk.iter().map(|account| async {
                let slots = account
                    .storage_keys
                    .iter()
                    .map(|key| (*key).into())
                    .collect::<Vec<_>>();
                let account_fut = self.get_account(account.address, &slots, Some(block_id), true);
                (account.address, account_fut.await)
            });

            let account_chunk = join_all(account_chunk_futs).await;

            for (address, value) in account_chunk {
                let account = value?;
                accounts.insert(address, account);
            }
        }

        Ok(AccessListResponse { accounts })
    }

    async fn chain_id(&self) -> Result<ChainIdResponse> {
        Ok(ChainIdResponse {
            chain_id: self.rpc.chain_id().await?,
        })
    }

    async fn get_block(&self, block_id: BlockId) -> Result<Option<N::BlockResponse>> {
        self.rpc.get_block(block_id, false.into()).await
    }

    async fn get_block_receipts(
        &self,
        block_id: BlockId,
    ) -> Result<Option<Vec<N::ReceiptResponse>>> {
        self.rpc.get_block_receipts(block_id).await
    }

    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<SendRawTxResponse> {
        Ok(SendRawTxResponse {
            hash: self.rpc.send_raw_transaction(bytes).await?,
        })
    }

    async fn new_filter(&self, filter: &Filter) -> Result<NewFilterResponse> {
        Ok(NewFilterResponse {
            id: self.rpc.new_filter(filter).await?,
            kind: FilterKind::Logs,
        })
    }

    async fn new_block_filter(&self) -> Result<NewFilterResponse> {
        Ok(NewFilterResponse {
            id: self.rpc.new_block_filter().await?,
            kind: FilterKind::NewBlocks,
        })
    }

    async fn new_pending_transaction_filter(&self) -> Result<NewFilterResponse> {
        Ok(NewFilterResponse {
            id: self.rpc.new_pending_transaction_filter().await?,
            kind: FilterKind::NewPendingTransactions,
        })
    }

    async fn uninstall_filter(&self, filter_id: U256) -> Result<UninstallFilterResponse> {
        Ok(UninstallFilterResponse {
            ok: self.rpc.uninstall_filter(filter_id).await?,
        })
    }
}

impl<N: NetworkSpec, R: ExecutionRpc<N>> ApiService<N, R> {
    async fn create_receipt_proofs_for_logs(
        &self,
        logs: &[Log],
    ) -> Result<HashMap<B256, TransactionReceiptResponse<N>>> {
        let block_nums = logs
            .iter()
            .map(|log| log.block_number.ok_or_eyre("block_number not found in log"))
            .collect::<Result<HashSet<_>, _>>()?;

        if block_nums.len() > MAX_SUPPORTED_BLOCKS_TO_PROVE_FOR_LOGS {
            return Err(ExecutionError::TooManyLogsToProve(
                logs.len(),
                block_nums.len(),
                MAX_SUPPORTED_BLOCKS_TO_PROVE_FOR_LOGS,
            )
            .into());
        }

        let blocks_receipts_fut = block_nums.into_iter().map(|block_num| async move {
            let receipts = self.rpc.get_block_receipts(block_num.into()).await?;
            receipts
                .ok_or::<Report>(
                    ExecutionError::NoReceiptsForBlock(BlockTag::Number(block_num)).into(),
                )
                .map(|receipts| (block_num, receipts))
        });
        let blocks_receipts = try_join_all(blocks_receipts_fut).await?;
        let blocks_receipts = blocks_receipts.into_iter().collect::<HashMap<_, _>>();

        let mut receipt_proofs: HashMap<B256, TransactionReceiptResponse<N>> = HashMap::new();

        for log in logs {
            let tx_hash = log.transaction_hash.unwrap();
            if receipt_proofs.contains_key(&tx_hash) {
                continue;
            }

            let block_num = log.block_number.unwrap();
            let receipts = blocks_receipts.get(&block_num).unwrap();
            let receipt = receipts
                .get(log.transaction_index.unwrap() as usize)
                .unwrap();

            let receipt_proof = create_receipt_proof::<N>(
                receipts.to_vec(),
                receipt.transaction_index().unwrap() as usize,
            );

            receipt_proofs.insert(
                tx_hash,
                TransactionReceiptResponse {
                    receipt: receipt.clone(),
                    receipt_proof,
                },
            );
        }

        Ok(receipt_proofs)
    }
}

#[cfg(test)]
mod tests {
    use alloy::rpc::types::TransactionRequest;
    use helios_core::execution::rpc::mock_rpc::MockRpc;
    use helios_ethereum::spec::Ethereum as EthereumSpec;
    use helios_test_utils::*;

    use super::*;

    fn get_service() -> ApiService<EthereumSpec, MockRpc> {
        let base_path = testdata_dir().join("rpc/");
        ApiService::<EthereumSpec, MockRpc>::new(base_path.to_str().unwrap())
    }

    #[tokio::test]
    async fn test_get_account() {
        let service = get_service();
        let rpc_proof = rpc_proof();

        let response = service
            .get_account(
                rpc_proof.address,
                &[U256::from(1)],
                BlockId::latest().into(),
                true,
            )
            .await
            .unwrap();

        assert_eq!(response, verifiable_api_account_response());
    }

    #[tokio::test]
    async fn test_get_account_without_code() {
        let service = get_service();
        let rpc_proof = rpc_proof();

        let response = service
            .get_account(rpc_proof.address, &[], BlockId::latest().into(), false)
            .await
            .unwrap();

        let mut expected_resp = verifiable_api_account_response();
        expected_resp.code = None;
        assert_eq!(response, expected_resp);
    }

    #[tokio::test]
    async fn test_get_transaction_receipt() {
        let service = get_service();

        let response = service
            .get_transaction_receipt(*rpc_tx().inner.tx_hash())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(response, verifiable_api_tx_receipt_response());
    }

    #[tokio::test]
    async fn test_get_logs() {
        let service = get_service();
        let filter = Filter::default();

        let response = service.get_logs(&filter).await.unwrap();

        let expected_response = verifiable_api_logs_response();
        assert_eq!(response.logs, expected_response.logs);
        assert_eq!(
            response
                .receipt_proofs
                .into_iter()
                .map(|(k, v)| (k, v))
                .collect::<Vec<_>>(),
            expected_response
                .receipt_proofs
                .into_iter()
                .map(|(k, v)| (k, v))
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn test_get_filter_logs() {
        let service = get_service();
        let filter_id = rpc_filter_id_logs();

        let response = service.get_filter_logs(filter_id).await.unwrap();

        let expected_response = verifiable_api_logs_response();
        assert_eq!(response.logs, expected_response.logs);
        assert_eq!(
            response
                .receipt_proofs
                .into_iter()
                .map(|(k, v)| (k, v))
                .collect::<Vec<_>>(),
            expected_response
                .receipt_proofs
                .into_iter()
                .map(|(k, v)| (k, v))
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn test_get_filter_changes_logs() {
        let service = get_service();
        let filter_id = rpc_filter_id_logs();

        let response = service.get_filter_changes(filter_id).await.unwrap();
        let response = match response {
            FilterChangesResponse::Logs(response) => response,
            _ => panic!("Expected FilterChangesResponse::Logs"),
        };

        let expected_response = verifiable_api_logs_response();
        assert_eq!(response.logs, expected_response.logs);
        assert_eq!(
            response
                .receipt_proofs
                .into_iter()
                .map(|(k, v)| (k, v))
                .collect::<Vec<_>>(),
            expected_response
                .receipt_proofs
                .into_iter()
                .map(|(k, v)| (k, v))
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn test_get_filter_changes_blocks() {
        let service = get_service();
        let filter_id = rpc_filter_id_blocks();

        let response = service.get_filter_changes(filter_id).await.unwrap();
        let response = match response {
            FilterChangesResponse::Hashes(response) => response,
            _ => panic!("Expected FilterChangesResponse::Hashes"),
        };

        assert_eq!(response, rpc_filter_block_hashes());
    }

    #[tokio::test]
    async fn test_get_filter_changes_txs() {
        let service = get_service();
        let filter_id = rpc_filter_id_txs();

        let response = service.get_filter_changes(filter_id).await.unwrap();
        let response = match response {
            FilterChangesResponse::Hashes(response) => response,
            _ => panic!("Expected FilterChangesResponse::Hashes"),
        };

        assert_eq!(response, rpc_filter_tx_hashes());
    }

    #[tokio::test]
    async fn test_create_access_list() {
        let service = get_service();
        let address = rpc_proof().address;
        let tx = TransactionRequest::default().from(address).to(address);

        let response = service
            .create_access_list(tx, BlockId::latest().into())
            .await
            .unwrap();

        assert_eq!(response.accounts.len(), 2);
        assert_eq!(
            response.accounts.get(&address).unwrap(),
            &verifiable_api_account_response()
        );
    }

    #[tokio::test]
    async fn test_chain_id() {
        let service = get_service();

        let response = service.chain_id().await.unwrap();

        assert_eq!(response.chain_id, rpc_chain_id());
    }

    #[tokio::test]
    async fn test_get_block() {
        let service = get_service();
        let response = service.get_block(BlockId::latest()).await.unwrap().unwrap();

        assert_eq!(response, rpc_block());
    }

    #[tokio::test]
    async fn test_get_block_receipts() {
        let service = get_service();

        let response = service
            .get_block_receipts(BlockId::latest())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(response, rpc_block_receipts());
    }

    #[tokio::test]
    async fn test_send_raw_transaction() {
        let service = get_service();

        let response = service.send_raw_transaction(&[]).await.unwrap();

        assert_eq!(response.hash, *rpc_tx().inner.tx_hash());
    }

    #[tokio::test]
    async fn test_new_filter() {
        let service = get_service();
        let filter = Filter::default();

        let response = service.new_filter(&filter).await.unwrap();

        assert_eq!(response.id, rpc_filter_id_logs());
        assert_eq!(response.kind, FilterKind::Logs);
    }

    #[tokio::test]
    async fn test_new_block_filter() {
        let service = get_service();

        let response = service.new_block_filter().await.unwrap();

        assert_eq!(response.id, rpc_filter_id_blocks());
        assert_eq!(response.kind, FilterKind::NewBlocks);
    }

    #[tokio::test]
    async fn test_new_pending_transaction_filter() {
        let service = get_service();

        let response = service.new_pending_transaction_filter().await.unwrap();

        assert_eq!(response.id, rpc_filter_id_txs());
        assert_eq!(response.kind, FilterKind::NewPendingTransactions);
    }

    #[tokio::test]
    async fn test_uninstall_filter() {
        let service = get_service();

        let response = service
            .uninstall_filter(rpc_filter_id_logs())
            .await
            .unwrap();

        assert_eq!(response.ok, true);
    }
}
