use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::sync::Arc;

use alloy::{
    consensus::{Account as TrieAccount, BlockHeader},
    eips::BlockNumberOrTag,
    network::{BlockResponse, ReceiptResponse},
    primitives::{Address, B256, U256},
    rpc::types::{BlockId, Filter, FilterChanges, Log},
};
use async_trait::async_trait;
use eyre::{Ok, OptionExt, Report, Result};
use futures::future::try_join_all;

use helios_common::{fork_schedule::ForkSchedule, network_spec::NetworkSpec, types::BlockTag};
use helios_core::execution::{
    client::{rpc::ExecutionInnerRpcClient, ExecutionInner},
    constants::MAX_SUPPORTED_BLOCKS_TO_PROVE_FOR_LOGS,
    errors::ExecutionError,
    proof::create_receipt_proof,
    rpc::ExecutionRpc,
    state::State,
};
use helios_verifiable_api_client::VerifiableApi;
use helios_verifiable_api_types::*;

#[derive(Clone)]
pub struct ApiService<N: NetworkSpec, R: ExecutionRpc<N>> {
    rpc_url: String,
    rpc: Arc<R>,
    _marker: PhantomData<N>,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec, R: ExecutionRpc<N>> VerifiableApi<N> for ApiService<N, R> {
    fn new(rpc: &str) -> Self {
        Self {
            rpc_url: rpc.to_string(),
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
            account: TrieAccount {
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
        let receipts = self
            .rpc
            .get_block_receipts(block_num.into())
            .await?
            .ok_or(ExecutionError::NoReceiptsForBlock(block_num.into()))?;

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

    async fn create_extended_access_list(
        &self,
        tx: N::TransactionRequest,
        validate_tx: bool,
        block_id: Option<BlockId>,
    ) -> Result<ExtendedAccessListResponse> {
        let block_id = block_id.unwrap_or_default();
        let block = self
            .rpc
            .get_block(block_id, false.into())
            .await?
            .ok_or_eyre(ExecutionError::BlockNotFound(block_id.try_into()?))?;
        let tag = BlockTag::Number(block.header().number());

        // initialise state for given block and ExecutionInnerRpcClient with it
        let state = State::new(1);
        let client = Arc::new(ExecutionInnerRpcClient::<N, R>::new(
            &self.rpc_url,
            state.clone(),
        )?);
        state.push_block(block, client.clone()).await;

        // call EVM with the transaction, collect accounts and storage keys
        let res = N::create_access_list(
            &tx,
            validate_tx,
            client,
            self.rpc.chain_id().await?,
            ForkSchedule {
                prague_timestamp: u64::MAX,
            },
            tag,
        )
        .await?;

        Ok(ExtendedAccessListResponse {
            accounts: res.accounts,
        })
    }

    async fn chain_id(&self) -> Result<ChainIdResponse> {
        Ok(ChainIdResponse {
            chain_id: self.rpc.chain_id().await?,
        })
    }

    async fn get_block(
        &self,
        block_id: BlockId,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        self.rpc.get_block(block_id, full_tx.into()).await
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
                .ok_or::<Report>(ExecutionError::NoReceiptsForBlock(block_num.into()).into())
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

        assert_eq!(response, rpc_account());
    }

    #[tokio::test]
    async fn test_get_account_without_code() {
        let service = get_service();
        let rpc_proof = rpc_proof();

        let response = service
            .get_account(rpc_proof.address, &[], BlockId::latest().into(), false)
            .await
            .unwrap();

        let mut expected_account = rpc_account();
        expected_account.code = None;
        assert_eq!(response, expected_account);
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
    async fn test_create_extended_access_list() {
        let service = get_service();
        let address = rpc_proof().address;
        let block = rpc_block();
        let tx = TransactionRequest::default()
            .from(block.header.beneficiary)
            .to(address)
            .value(U256::from(0));

        let response = service
            .create_extended_access_list(tx, false, BlockId::latest().into())
            .await
            .unwrap();

        assert_eq!(response.accounts.len(), 2);
        assert_eq!(response.accounts.get(&address).unwrap(), &rpc_account());
        assert_eq!(
            response.accounts.get(&block.header.beneficiary).unwrap(),
            &rpc_block_miner_account()
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
        let response = service
            .get_block(BlockId::latest(), false)
            .await
            .unwrap()
            .unwrap();

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
