use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::sync::Arc;

use alloy::{
    consensus::{Account, BlockHeader},
    eips::BlockNumberOrTag,
    network::{BlockResponse, ReceiptResponse, TransactionBuilder},
    primitives::{Address, B256, U256},
    rpc::types::{AccessListItem, BlockId, BlockTransactionsKind, Filter, FilterChanges, Log},
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
            rpc: Arc::new(ExecutionRpc::new(rpc).unwrap()),
            _marker: Default::default(),
        }
    }

    async fn get_account(
        &self,
        address: Address,
        storage_slots: &[U256],
        block: Option<BlockId>,
        include_code: bool,
    ) -> Result<AccountResponse> {
        let block = block.unwrap_or_default();
        // make sure block ID is not tag but a number or hash
        let block = match block {
            BlockId::Number(number_or_tag) => match number_or_tag {
                BlockNumberOrTag::Number(_) => Ok(block),
                tag => Ok(BlockId::Number(
                    self.rpc
                        .get_block_by_number(tag, false.into())
                        .await?
                        .ok_or_eyre(ExecutionError::BlockNotFound(tag.try_into()?))
                        .map(|block| block.header().number())?
                        .into(),
                )),
            },
            BlockId::Hash(_) => Ok(block),
        }?;

        let storage_keys = storage_slots
            .iter()
            .map(|key| (*key).into())
            .collect::<Vec<_>>();
        let proof = self.rpc.get_proof(address, &storage_keys, block).await?;

        let code = if include_code {
            Some(self.rpc.get_code(address, block).await?.into())
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
        let receipt = self.rpc.get_transaction_receipt(tx_hash).await?;
        if receipt.is_none() {
            return Ok(None);
        }
        let receipt = receipt.unwrap();

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
        block: Option<BlockId>,
    ) -> Result<AccessListResponse> {
        let block_id = block.unwrap_or_default();
        let block = match block_id {
            BlockId::Number(number_or_tag) => self
                .rpc
                .get_block_by_number(number_or_tag, BlockTransactionsKind::Hashes)
                .await?
                .ok_or_eyre(ExecutionError::BlockNotFound(number_or_tag.try_into()?)),
            BlockId::Hash(hash) => self.rpc.get_block(hash.into()).await,
        }?;
        let block_id = BlockId::Number(block.header().number().into());

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

    async fn get_block(&self, block: BlockId) -> Result<Option<N::BlockResponse>> {
        Ok(match block {
            BlockId::Number(number_or_tag) => {
                self.rpc
                    .get_block_by_number(number_or_tag, false.into())
                    .await?
            }
            BlockId::Hash(hash) => self.rpc.get_block(hash.into()).await.ok(),
        })
    }

    async fn get_block_receipts(&self, block: BlockId) -> Result<Option<Vec<N::ReceiptResponse>>> {
        self.rpc.get_block_receipts(block).await
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
