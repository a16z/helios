use std::collections::{HashMap, HashSet};

use alloy::{
    consensus::BlockHeader,
    eips::BlockId,
    network::{
        primitives::HeaderResponse, BlockResponse, ReceiptResponse, TransactionBuilder,
        TransactionResponse,
    },
    primitives::{Address, Bytes, B256},
    providers::{Provider, ProviderBuilder, RootProvider},
    rpc::{
        client::ClientBuilder,
        types::{AccessListItem, Filter, Log},
    },
    transports::layers::RetryBackoffLayer,
};
use alloy_trie::{TrieAccount, KECCAK_EMPTY};
use async_trait::async_trait;
use eyre::{eyre, Result};
use futures::future::join_all;
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
    proof::{
        verify_account_proof, verify_block_receipts, verify_code_hash_proof, verify_storage_proof,
    },
    providers::historical::HistoricalBlockProvider,
};

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
}

impl<N: NetworkSpec, B: BlockProvider<N>, H: HistoricalBlockProvider<N>> ExecutionProvider<N>
    for RpcExecutionProvider<N, B, H>
{
}

impl<N: NetworkSpec, B: BlockProvider<N>, H: HistoricalBlockProvider<N>>
    RpcExecutionProvider<N, B, H>
{
    pub fn new(rpc_url: Url, block_provider: B) -> RpcExecutionProvider<N, B, ()> {
        let client = ClientBuilder::default()
            .layer(RetryBackoffLayer::new(100, 50, 300))
            .http(rpc_url);

        let provider = ProviderBuilder::<_, _, N>::default().connect_client(client);

        RpcExecutionProvider {
            provider,
            block_provider,
            historical_provider: None,
        }
    }

    pub fn with_historical_provider(
        rpc_url: Url,
        block_provider: B,
        historical_provider: H,
    ) -> Self {
        let client = ClientBuilder::default()
            .layer(RetryBackoffLayer::new(100, 50, 300))
            .http(rpc_url);

        let provider = ProviderBuilder::<_, _, N>::default().connect_client(client);

        Self {
            provider,
            block_provider,
            historical_provider: Some(historical_provider),
        }
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

        verify_account_proof(&proof, block.header().state_root())?;
        verify_storage_proof(&proof)?;

        let code = if with_code {
            if proof.code_hash == KECCAK_EMPTY || proof.code_hash == B256::ZERO {
                Some(Bytes::new())
            } else {
                let code = self.provider.get_code_at(address).await?;
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
            Ok(txs.iter().find(|v| v.tx_hash() == tx.tx_hash()).cloned())
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
        Ok(txs.get(index as usize).cloned())
    }

    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<B256> {
        let tx = self.provider.send_raw_transaction(bytes).await?;
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
            .get_block(block_hash.into(), false)
            .await?
            .ok_or(eyre!("block not found"))?;

        let receipts = self
            .provider
            .get_block_receipts(block_hash.into())
            .await?
            .ok_or(eyre!("block not found"))?;

        verify_block_receipts::<N>(&receipts, &block)?;
        Ok(receipts
            .iter()
            .find(|receipt| receipt.transaction_hash() == hash)
            .cloned())
    }

    async fn get_block_receipts(
        &self,
        block_id: BlockId,
    ) -> Result<Option<Vec<N::ReceiptResponse>>> {
        let Some(block) = self.get_block(block_id, false).await? else {
            return Ok(None);
        };

        let receipts = self
            .provider
            .get_block_receipts(block.header().hash().into())
            .await?
            .ok_or(eyre!("receipt fetch failed"))?;

        verify_block_receipts::<N>(&receipts, &block)?;
        Ok(Some(receipts))
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N: NetworkSpec, B: BlockProvider<N>, H: HistoricalBlockProvider<N>> LogProvider<N>
    for RpcExecutionProvider<N, B, H>
{
    async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>> {
        super::utils::get_verified_logs::<N, _>(self, filter).await
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
