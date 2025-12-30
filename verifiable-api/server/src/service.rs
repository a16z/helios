use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use alloy::providers::{Provider, ProviderBuilder, RootProvider};
use alloy::rpc::client::ClientBuilder;
use alloy::transports::layers::RetryBackoffLayer;
use alloy::{
    consensus::{TrieAccount, BlockHeader},
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
use rayon::prelude::*;
use url::Url;

use helios_common::{
    execution_provider::BlockProvider, fork_schedule::ForkSchedule, network_spec::NetworkSpec,
};
use helios_core::execution::proof::create_transaction_proof;
use helios_core::execution::providers::block::block_cache::BlockCache;
use helios_core::execution::providers::rpc::RpcExecutionProvider;

use helios_core::execution::{
    constants::MAX_SUPPORTED_BLOCKS_TO_PROVE_FOR_LOGS, errors::ExecutionError,
    proof::create_receipt_proof,
};
use helios_verifiable_api_client::VerifiableApi;
use helios_verifiable_api_types::{TransactionResponse, *};

#[derive(Clone)]
pub struct ApiService<N: NetworkSpec> {
    rpc_url: String,
    rpc: RootProvider<N>,
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec> VerifiableApi<N> for ApiService<N> {
    fn new(rpc_url: &Url) -> Self {
        let client = ClientBuilder::default()
            .layer(RetryBackoffLayer::new(100, 50, 300))
            .http(rpc_url.to_string().parse().expect("Invalid RPC URL"));

        let provider = ProviderBuilder::<_, _, N>::default().connect_client(client);

        Self {
            rpc_url: rpc_url.to_string(),
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
                        .ok_or_eyre(ExecutionError::BlockNotFound(tag.into()))
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
            Some(self.rpc.get_code_at(address).block_id(block_id).await?)
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
            block.transactions().txns().cloned().collect(),
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
            block.transactions().txns().cloned().collect(),
            tx.transaction_index().unwrap() as usize,
        );

        Ok(Some(TransactionResponse {
            transaction: tx,
            transaction_proof: proof,
        }))
    }

    async fn get_logs(&self, filter: &Filter) -> Result<LogsResponse<N>> {
        let logs = self.rpc.get_logs(filter).await?;
        let receipt_proofs = self.create_receipt_proofs_for_logs(logs.clone()).await?;

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
            .ok_or_eyre(ExecutionError::BlockNotFound(block_id))?;

        let block_id = block.header().hash().into();

        // initialize execution provider for the given block
        let block_provider = BlockCache::<N>::new();
        let provider = RpcExecutionProvider::<N, BlockCache<N>, ()>::new(
            self.rpc_url.parse().unwrap(),
            block_provider,
        );
        provider.push_block(block, block_id).await;

        // call EVM with the transaction, collect accounts and storage keys
        let (.., accounts) = N::transact(
            &tx,
            validate_tx,
            Arc::new(provider),
            self.rpc.get_chain_id().await?,
            ForkSchedule::default(),
            block_id,
            None, // state overrides not supported for execution hints
        )
        .await?;

        Ok(ExtendedAccessListResponse { accounts })
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
        logs: Vec<Log>,
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

        let receipt_proofs = tokio::task::spawn_blocking(move || {
            let blocks_receipts = blocks_receipts.into_iter().collect::<HashMap<_, _>>();
            let mut receipts_to_prove = HashSet::new();
            for log in logs {
                let entry = (
                    log.block_number.unwrap(),
                    log.transaction_hash.unwrap(),
                    log.transaction_index.unwrap(),
                );
                if !receipts_to_prove.contains(&entry) {
                    receipts_to_prove.insert(entry);
                }
            }

            receipts_to_prove
                .par_iter()
                .map(|(block_number, tx_hash, tx_index)| {
                    let receipts = blocks_receipts.get(block_number).unwrap();
                    let receipt = receipts.get(*tx_index as usize).unwrap();

                    let proof = create_receipt_proof::<N>(
                        receipts.to_vec(),
                        receipt.transaction_index().unwrap() as usize,
                    );

                    (
                        *tx_hash,
                        TransactionReceiptResponse {
                            receipt: receipt.clone(),
                            receipt_proof: proof,
                        },
                    )
                })
                .collect::<Vec<_>>()
        })
        .await?;

        let mut receipt_response = HashMap::new();
        for (tx_hash, receipt_proof) in receipt_proofs {
            receipt_response.insert(tx_hash, receipt_proof);
        }

        Ok(receipt_response)
    }
}
