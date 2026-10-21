use helios_common::fork_schedule::ForkSchedule;
use std::collections::{HashMap, HashSet};

use alloy::{
    consensus::BlockHeader,
    eips::{BlockId, BlockNumberOrTag},
    network::{
        primitives::HeaderResponse, BlockResponse, ReceiptResponse, TransactionBuilder,
        TransactionResponse,
    },
    primitives::{Address, Bytes, B256, U256},
    providers::{Provider, ProviderBuilder, RootProvider},
    rpc::{
        client::ClientBuilder,
        types::{AccessListItem, Filter, FilterBlockOption, Log},
    },
    transports::layers::RetryBackoffLayer,
};
use alloy_trie::{TrieAccount, KECCAK_EMPTY};
use async_trait::async_trait;
use eyre::{eyre, Result};
use futures::future::{join_all, try_join_all};
use reqwest::Url;

use helios_common::{
    execution_provider::{
        AccountProvider, BlockProvider, ExecutionHintProvider, ExecutionProvider, LogProvider,
        ReceiptProvider, TransactionProvider,
    },
    network_spec::NetworkSpec,
    types::Account,
};

use crate::execution::{
    constants::PARALLEL_QUERY_BATCH_SIZE,
    errors::ExecutionError,
    proof::{
        verify_account_proof, verify_authenticated_block_receipts, verify_code_hash_proof,
        verify_storage_proof,
    },
    providers::historical::HistoricalBlockProvider,
};

use super::utils::ensure_logs_match_filter;

// Implementation for unit type to provide no historical block support
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec> HistoricalBlockProvider<N> for () {
    async fn get_historical_block<E>(
        &self,
        _block_id: BlockId,
        _full_tx: bool,
        _execution_provider: &E,
    ) -> Result<Option<N::BlockResponse>>
    where
        E: BlockProvider<N> + AccountProvider<N>,
    {
        Ok(None)
    }
}

pub struct RpcExecutionProvider<N: NetworkSpec, B: BlockProvider<N>, H: HistoricalBlockProvider<N>>
{
    provider: RootProvider<N>,
    block_provider: B,
    historical_provider: Option<H>,
    fork_schedule: ForkSchedule,
}

impl<N: NetworkSpec, B: BlockProvider<N>, H: HistoricalBlockProvider<N>> ExecutionProvider<N>
    for RpcExecutionProvider<N, B, H>
{
}

impl<N: NetworkSpec, B: BlockProvider<N>, H: HistoricalBlockProvider<N>>
    RpcExecutionProvider<N, B, H>
{
    pub fn new(
        rpc_url: Url,
        block_provider: B,
        fork_schedule: ForkSchedule,
    ) -> RpcExecutionProvider<N, B, ()> {
        let client = ClientBuilder::default()
            .layer(RetryBackoffLayer::new(100, 50, 300))
            .http(rpc_url);

        let provider = ProviderBuilder::<_, _, N>::default().connect_client(client);

        RpcExecutionProvider {
            provider,
            block_provider,
            historical_provider: None,
            fork_schedule,
        }
    }

    pub fn with_historical_provider(
        rpc_url: Url,
        block_provider: B,
        historical_provider: H,
        fork_schedule: ForkSchedule,
    ) -> Self {
        let client = ClientBuilder::default()
            .layer(RetryBackoffLayer::new(100, 50, 300))
            .http(rpc_url);

        let provider = ProviderBuilder::<_, _, N>::default().connect_client(client);

        Self {
            provider,
            block_provider,
            historical_provider: Some(historical_provider),
            fork_schedule,
        }
    }

    async fn verify_logs(&self, logs: &[Log]) -> Result<()> {
        // get latest block
        let latest = self
            .get_block(BlockId::Number(BlockNumberOrTag::Latest), false)
            .await?
            .ok_or(eyre!("block not found"))?
            .header()
            .number();

        // Only fetch receipts for blocks represented in the returned logs.
        let block_nums = logs
            .iter()
            .map(|log| {
                let number = log
                    .block_number
                    .ok_or_else(|| eyre!("log missing block number"))?;
                if number > latest {
                    return Err(eyre!("log is ahead of the latest verified block"));
                }
                Ok(number)
            })
            .collect::<Result<HashSet<_>>>()?;

        // Collect all (proven) tx receipts for all block numbers
        let blocks_receipts_fut = block_nums.into_iter().map(|block_num| async move {
            self.get_block_receipts_with_timestamp(block_num.into())
                .await
        });

        let blocks_receipts = try_join_all(blocks_receipts_fut).await?;
        let mut receipt_logs = HashMap::new();
        for (receipts, timestamp) in blocks_receipts.into_iter().flatten() {
            for log in receipts.iter().flat_map(N::receipt_logs) {
                // Receipt metadata is authenticated against the block and receipt order.
                receipt_logs.insert((log.block_number, log.log_index), (log, timestamp));
            }
        }

        for log in logs {
            let tx_hash = log
                .transaction_hash
                .ok_or_else(|| eyre!("log missing transaction hash"))?;
            let log_index = log
                .log_index
                .ok_or_else(|| eyre!("log missing log index"))?;
            let missing_log = || ExecutionError::MissingLog(tx_hash, U256::from(log_index));
            let (expected, timestamp) = receipt_logs
                .get(&(log.block_number, log.log_index))
                .ok_or_else(missing_log)?;

            // The lookup binds the block number and log index. Check all remaining
            // fields, allowing either RPC response to omit the optional timestamp.
            if log.inner != expected.inner
                || log.block_hash != expected.block_hash
                || log.transaction_hash != expected.transaction_hash
                || log.transaction_index != expected.transaction_index
                || log.removed != expected.removed
                || log.block_timestamp.is_some_and(|t| t != *timestamp)
            {
                return Err(missing_log().into());
            }
        }
        Ok(())
    }

    async fn get_block_receipts_with_timestamp(
        &self,
        block_id: BlockId,
    ) -> Result<Option<(Vec<N::ReceiptResponse>, u64)>> {
        let Some(block) = self.get_block(block_id, true).await? else {
            return Ok(None);
        };

        let mut receipts = self
            .provider
            .get_block_receipts(block.header().hash().into())
            .await?
            .ok_or(eyre!("receipt fetch failed"))?;

        verify_authenticated_block_receipts::<N>(&receipts, &block, &self.fork_schedule)?;
        receipts.iter_mut().for_each(N::sanitize_receipt);
        Ok(Some((receipts, block.header().timestamp())))
    }

    async fn resolve_block_number(&self, block: Option<BlockNumberOrTag>) -> Result<u64> {
        let tag = match block {
            Some(BlockNumberOrTag::Latest) | None => BlockNumberOrTag::Latest,
            Some(BlockNumberOrTag::Finalized) => BlockNumberOrTag::Finalized,
            Some(BlockNumberOrTag::Number(number)) => return Ok(number),
            _ => return Err(eyre!("block not found")),
        };

        let number = self
            .get_block(BlockId::Number(tag), false)
            .await?
            .ok_or(eyre!("block not found"))?
            .header()
            .number();

        Ok(number)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec, B: BlockProvider<N>, H: HistoricalBlockProvider<N>> AccountProvider<N>
    for RpcExecutionProvider<N, B, H>
{
    async fn get_account(
        &self,
        address: Address,
        slots: &[B256],
        with_code: bool,
        block_id: BlockId,
    ) -> Result<Account> {
        let block = self
            .get_block(block_id, false)
            .await?
            .ok_or(eyre!("block not found"))?;

        let proof = self
            .provider
            .get_proof(address, slots.to_vec())
            .block_id(block.header().hash().into())
            .await?;

        if proof.address != address {
            return Err(ExecutionError::InvalidAccountProof(address).into());
        }
        verify_account_proof(&proof, block.header().state_root())?;
        verify_storage_proof(&proof)?;
        for slot in slots {
            if !proof
                .storage_proof
                .iter()
                .any(|entry| entry.key.as_b256() == *slot)
            {
                return Err(eyre!("missing requested storage proof for {slot}"));
            }
        }

        let code = if with_code {
            if proof.code_hash == KECCAK_EMPTY || proof.code_hash == B256::ZERO {
                Some(Bytes::new())
            } else {
                let code = self
                    .provider
                    .get_code_at(address)
                    .block_id(block.header().hash().into())
                    .await?;
                verify_code_hash_proof(&proof, &code)?;
                Some(code)
            }
        } else {
            None
        };

        Ok(Account {
            account: TrieAccount {
                nonce: proof.nonce,
                balance: proof.balance,
                storage_root: proof.storage_hash,
                code_hash: proof.code_hash,
            },
            code,
            account_proof: proof.account_proof,
            storage_proof: proof.storage_proof,
        })
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec, B: BlockProvider<N>, H: HistoricalBlockProvider<N>> BlockProvider<N>
    for RpcExecutionProvider<N, B, H>
{
    async fn get_block(
        &self,
        block_id: BlockId,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        // 1. Try block cache first
        if let Some(block) = self.block_provider.get_block(block_id, full_tx).await? {
            return Ok(Some(block));
        }

        // 2. Try historical provider if available and only for block numbers or hashes (not tags)
        if let Some(historical) = &self.historical_provider {
            if super::utils::should_use_historical_provider(&block_id) {
                if let Some(block) = historical
                    .get_historical_block(block_id, full_tx, self)
                    .await?
                {
                    // Note: Do NOT cache historical blocks to avoid interfering with consistency detection
                    return Ok(Some(block));
                }
            }
        }

        Ok(None)
    }

    async fn get_untrusted_block(
        &self,
        block_id: BlockId,
        full_tx: bool,
    ) -> Result<Option<<N>::BlockResponse>> {
        if full_tx {
            Ok(self.provider.get_block(block_id).full().await?)
        } else {
            Ok(self.provider.get_block(block_id).hashes().await?)
        }
    }

    async fn push_block(&self, block: N::BlockResponse, block_id: BlockId) {
        self.block_provider.push_block(block, block_id).await
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec, B: BlockProvider<N>, H: HistoricalBlockProvider<N>> TransactionProvider<N>
    for RpcExecutionProvider<N, B, H>
{
    async fn get_transaction(&self, hash: B256) -> Result<Option<N::TransactionResponse>> {
        let tx = self.provider.get_transaction_by_hash(hash).await?;
        if let Some(tx) = tx {
            let block_hash = tx.block_hash().ok_or(eyre!("block not found"))?;
            let block = self.get_block(block_hash.into(), true).await?;

            let block = block.ok_or(eyre!("block not found"))?;
            let txs = block.transactions().clone().into_transactions_vec();
            Ok(txs.iter().find(|v| v.tx_hash() == hash).cloned())
        } else {
            Ok(None)
        }
    }

    async fn get_transaction_by_location(
        &self,
        block_id: BlockId,
        index: u64,
    ) -> Result<Option<N::TransactionResponse>> {
        let block = self.get_block(block_id, true).await?;

        let block = block.ok_or(eyre!("block not found"))?;
        let txs = block.transactions().clone().into_transactions_vec();
        Ok(usize::try_from(index)
            .ok()
            .and_then(|index| txs.get(index))
            .cloned())
    }

    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<B256> {
        let tx = self.provider.send_raw_transaction(bytes).await?;
        super::utils::verify_submitted_transaction_hash::<N>(bytes, *tx.tx_hash())?;
        Ok(*tx.tx_hash())
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec, B: BlockProvider<N>, H: HistoricalBlockProvider<N>> ReceiptProvider<N>
    for RpcExecutionProvider<N, B, H>
{
    async fn get_receipt(&self, hash: B256) -> Result<Option<N::ReceiptResponse>> {
        let receipt = self
            .provider
            .get_transaction_receipt(hash)
            .await?
            .ok_or(eyre!("receipt not found"))?;

        let block_hash = receipt.block_hash().ok_or(eyre!("block not found"))?;
        let block = self
            .get_block(block_hash.into(), true)
            .await?
            .ok_or(eyre!("block not found"))?;

        let mut receipts = self
            .provider
            .get_block_receipts(block_hash.into())
            .await?
            .ok_or(eyre!("block not found"))?;

        verify_authenticated_block_receipts::<N>(&receipts, &block, &self.fork_schedule)?;
        receipts.iter_mut().for_each(N::sanitize_receipt);
        Ok(receipts
            .iter()
            .find(|receipt| receipt.transaction_hash() == hash)
            .cloned())
    }

    async fn get_block_receipts(
        &self,
        block_id: BlockId,
    ) -> Result<Option<Vec<N::ReceiptResponse>>> {
        Ok(self
            .get_block_receipts_with_timestamp(block_id)
            .await?
            .map(|(receipts, _)| receipts))
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec, B: BlockProvider<N>, H: HistoricalBlockProvider<N>> LogProvider<N>
    for RpcExecutionProvider<N, B, H>
{
    async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>> {
        let block_option = match filter.block_option {
            FilterBlockOption::Range {
                from_block,
                to_block,
            } => {
                let from = self.resolve_block_number(from_block).await?;
                let to = self.resolve_block_number(to_block).await?;
                FilterBlockOption::Range {
                    from_block: Some(BlockNumberOrTag::Number(from)),
                    to_block: Some(BlockNumberOrTag::Number(to)),
                }
            }
            FilterBlockOption::AtBlockHash(hash) => FilterBlockOption::AtBlockHash(hash),
        };

        let mut filter = filter.clone();
        filter.block_option = block_option;

        let logs = self.provider.get_logs(&filter).await?;
        self.verify_logs(&logs).await?;
        ensure_logs_match_filter(&logs, &filter)?;
        Ok(logs)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec, B: BlockProvider<N>, H: HistoricalBlockProvider<N>> ExecutionHintProvider<N>
    for RpcExecutionProvider<N, B, H>
{
    async fn get_execution_hint(
        &self,
        tx: &N::TransactionRequest,
        _validate: bool,
        block_id: BlockId,
    ) -> Result<HashMap<Address, Account>> {
        let block = self
            .get_block(block_id, false)
            .await?
            .ok_or(eyre!("block not found"))?;

        let mut list = self
            .provider
            .create_access_list(tx)
            .block_id(block_id)
            .await?
            .access_list
            .0;

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

        let mut list_addresses = list.iter().map(|elem| elem.address).collect::<HashSet<_>>();

        if list_addresses.insert(from_access_entry.address) {
            list.push(from_access_entry)
        }
        if list_addresses.insert(to_access_entry.address) {
            list.push(to_access_entry)
        }
        if list_addresses.insert(producer_access_entry.address) {
            list.push(producer_access_entry)
        }

        let mut account_map = HashMap::new();
        for chunk in list.chunks(PARALLEL_QUERY_BATCH_SIZE) {
            let account_chunk_futs = chunk.iter().map(|account| {
                let account_fut =
                    self.get_account(account.address, &account.storage_keys, true, block_id);
                async move { (account.address, account_fut.await) }
            });

            let account_chunk = join_all(account_chunk_futs).await;

            for (address, value) in account_chunk {
                let account = value?;
                account_map.insert(address, account);
            }
        }

        Ok(account_map)
    }
}
