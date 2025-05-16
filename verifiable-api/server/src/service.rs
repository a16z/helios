use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::rpc::client::ClientBuilder;
use alloy::transports::layers::RetryBackoffLayer;
use alloy::{
    consensus::{Account as TrieAccount, BlockHeader},
    eips::BlockNumberOrTag,
    network::{
        primitives::HeaderResponse, BlockResponse, ReceiptResponse, TransactionResponse as TxTr,
    },
    primitives::{Address, B256, U256},
    rpc::types::{BlockId, Filter, Log},
};
use async_trait::async_trait;
use eyre::{eyre, Ok, OptionExt, Report, Result};
use futures::future::try_join_all;

use helios_common::{fork_schedule::ForkSchedule, network_spec::NetworkSpec};
use helios_core::execution::proof::create_transaction_proof;
use helios_core::execution::providers::block::block_cache::BlockCache;
use helios_core::execution::providers::rpc::RpcExecutionProvider;
use helios_core::execution::providers::BlockProvider;
use helios_core::execution::{
    constants::MAX_SUPPORTED_BLOCKS_TO_PROVE_FOR_LOGS, errors::ExecutionError, evm::Evm,
    proof::create_receipt_proof,
};
use helios_verifiable_api_client::VerifiableApi;
use helios_verifiable_api_types::{TransactionResponse, *};
use tokio::time::Instant;

#[derive(Clone)]
pub struct ApiService<N: NetworkSpec> {
    rpc_url: String,
    rpc: RootProvider<N>,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec> VerifiableApi<N> for ApiService<N> {
    fn new(rpc: &str) -> Self {
        let client = ClientBuilder::default()
            .layer(RetryBackoffLayer::new(100, 50, 300))
            .http(rpc.parse().unwrap());

        let provider = ProviderBuilder::<_, _, N>::default().on_client(client);

        Self {
            rpc_url: rpc.to_string(),
            rpc: provider,
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
                        .get_block(tag.into())
                        .hashes()
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

        let proof = self
            .rpc
            .get_proof(address, storage_keys)
            .block_id(block_id)
            .await?;

        let code = if include_code {
            Some(
                self.rpc
                    .get_code_at(address)
                    .block_id(block_id)
                    .await?
                    .into(),
            )
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

        let block_num = receipt.block_number().ok_or(eyre!("block not found"))?;
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

    async fn get_transaction(&self, tx_hash: B256) -> Result<Option<TransactionResponse<N>>> {
        let Some(tx) = self.rpc.get_transaction_by_hash(tx_hash).await? else {
            return Ok(None);
        };

        let block_hash = tx.block_hash().ok_or(eyre!("block not found"))?;
        let block = self
            .rpc
            .get_block(block_hash.into())
            .full()
            .await?
            .ok_or(eyre!("block not found"))?;

        let proof = create_transaction_proof::<N>(
            block.transactions().txns().map(|v| v.clone()).collect(),
            tx.transaction_index().unwrap() as usize,
        );

        Ok(Some(TransactionResponse {
            transaction: tx,
            transaction_proof: proof,
        }))
    }

    async fn get_transaction_by_location(
        &self,
        block_id: BlockId,
        index: u64,
    ) -> Result<Option<TransactionResponse<N>>> {
        let index = index as usize;
        let tx = match block_id {
            BlockId::Hash(hash) => {
                self.rpc
                    .get_transaction_by_block_hash_and_index(hash.into(), index)
                    .await?
            }
            BlockId::Number(number) => {
                self.rpc
                    .get_transaction_by_block_number_and_index(number, index)
                    .await?
            }
        };

        let Some(tx) = tx else {
            return Ok(None);
        };

        let block_hash = tx.block_hash().ok_or(eyre!("block not found"))?;
        let block = self
            .rpc
            .get_block(block_hash.into())
            .full()
            .await?
            .ok_or(eyre!("block not found"))?;

        let proof = create_transaction_proof::<N>(
            block.transactions().txns().map(|v| v.clone()).collect(),
            tx.transaction_index().unwrap() as usize,
        );

        Ok(Some(TransactionResponse {
            transaction: tx,
            transaction_proof: proof,
        }))
    }

    async fn get_logs(&self, filter: &Filter) -> Result<LogsResponse<N>> {
        let start = Instant::now();
        let logs = self.rpc.get_logs(filter).await?;
        let finish = Instant::now();
        let duration = finish.duration_since(start);
        println!("logs fetch: {}ms", duration.as_millis());

        let receipt_proofs = self.create_receipt_proofs_for_logs(&logs).await?;

        Ok(LogsResponse {
            logs,
            receipt_proofs,
        })
    }

    async fn get_execution_hint(
        &self,
        tx: N::TransactionRequest,
        validate_tx: bool,
        block_id: Option<BlockId>,
    ) -> Result<ExtendedAccessListResponse> {
        let block_id = block_id.unwrap_or_default();
        let block = self
            .rpc
            .get_block(block_id)
            .hashes()
            .await?
            .ok_or_eyre(ExecutionError::BlockNotFound(block_id.try_into()?))?;

        let block_id = block.header().hash().into();

        // initialize exection provider for the given block
        let block_provider = BlockCache::<N>::new();
        let provider = RpcExecutionProvider::new(self.rpc_url.parse().unwrap(), block_provider);
        provider.push_block(block, block_id).await;

        // call EVM with the transaction, collect accounts and storage keys
        let mut evm = Evm::new(
            Arc::new(provider),
            self.rpc.get_chain_id().await?,
            ForkSchedule {
                prague_timestamp: u64::MAX,
            },
            block_id,
        );
        let res = evm.create_access_list(&tx, validate_tx).await?;

        Ok(ExtendedAccessListResponse {
            accounts: res.accounts,
        })
    }

    async fn chain_id(&self) -> Result<ChainIdResponse> {
        Ok(ChainIdResponse {
            chain_id: self.rpc.get_chain_id().await?,
        })
    }

    async fn get_block(
        &self,
        block_id: BlockId,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        if full_tx {
            Ok(self.rpc.get_block(block_id).full().await?)
        } else {
            Ok(self.rpc.get_block(block_id).hashes().await?)
        }
    }

    async fn get_block_receipts(
        &self,
        block_id: BlockId,
    ) -> Result<Option<Vec<N::ReceiptResponse>>> {
        Ok(self.rpc.get_block_receipts(block_id).await?)
    }

    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<SendRawTxResponse> {
        Ok(SendRawTxResponse {
            hash: *self.rpc.send_raw_transaction(bytes).await?.tx_hash(),
        })
    }
}

impl<N: NetworkSpec> ApiService<N> {
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

        let start = Instant::now();
        let blocks_receipts = try_join_all(blocks_receipts_fut).await?;
        let finish = Instant::now();
        let duration = finish.duration_since(start);
        println!("block receipts fetch: {}ms", duration.as_millis());

        let blocks_receipts = blocks_receipts.into_iter().collect::<HashMap<_, _>>();

        let mut receipt_proofs: HashMap<B256, TransactionReceiptResponse<N>> = HashMap::new();

        let start = Instant::now();
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
        let finish = Instant::now();
        let duration = finish.duration_since(start);
        println!("log processing: {}ms", duration.as_millis());

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
