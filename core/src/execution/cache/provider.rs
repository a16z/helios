use std::{collections::HashMap, sync::Arc};

use alloy::{
    eips::BlockId,
    network::{primitives::HeaderResponse, BlockResponse},
    primitives::{Address, B256},
    rpc::types::{EIP1186AccountProofResponse, Filter, Log},
};
use async_trait::async_trait;
use eyre::{eyre, Result};

use helios_common::{
    execution_provider::{
        AccountProvider, BlockProvider, ExecutionHintProvider, ExecutionProvider, LogProvider,
        ReceiptProvider, TransactionProvider,
    },
    network_spec::NetworkSpec,
    types::Account,
};

use super::Cache;

pub struct CachingProvider<P> {
    inner: P,
    cache: Arc<Cache>,
}

impl<P> CachingProvider<P> {
    pub fn new(inner: P) -> Self {
        Self {
            inner,
            cache: Arc::new(Cache::new()),
        }
    }
}

impl<N, P> ExecutionProvider<N> for CachingProvider<P>
where
    N: NetworkSpec,
    P: ExecutionProvider<N>,
{
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N, P> AccountProvider<N> for CachingProvider<P>
where
    N: NetworkSpec,
    P: ExecutionProvider<N>,
{
    async fn get_account(
        &self,
        address: Address,
        slots: &[B256],
        with_code: bool,
        block_id: BlockId,
    ) -> Result<Account> {
        let block_hash = match block_id {
            BlockId::Hash(hash) => {
                let hash: B256 = hash.into();
                hash
            }
            block_id => {
                let block = self
                    .inner
                    .get_block(block_id, false)
                    .await?
                    .ok_or(eyre!("block not found"))?;
                block.header().hash()
            }
        };

        let slots_left_to_fetch = match self.cache.get_account(address, slots, block_hash) {
            Some((cached, missing_slots)) => {
                if missing_slots.is_empty() {
                    if with_code && cached.code.is_none() {
                        // All account data is cached, but code is missing - fetch code directly
                        let expected = cached.account.code_hash;
                        let acc_with_code = self
                            .inner
                            .get_account(address, &[], true, block_hash.into())
                            .await?;
                        let code = acc_with_code
                            .code
                            .ok_or_else(|| eyre!("provider did not return code"))?;
                        if acc_with_code.account.code_hash != expected {
                            return Err(eyre!("code_hash changed while fetching code"));
                        }
                        self.cache.insert_code(expected, code.clone());
                        return Ok(Account {
                            code: Some(code),
                            ..cached
                        });
                    }
                    return Ok(cached);
                }
                missing_slots
            }
            None => slots.to_vec(),
        };

        // Always fetch account proof without full code because it might be already in cache
        let account = self
            .inner
            .get_account(address, &slots_left_to_fetch, false, block_hash.into())
            .await?;

        let response = EIP1186AccountProofResponse {
            address,
            balance: account.account.balance,
            code_hash: account.account.code_hash,
            nonce: account.account.nonce,
            storage_hash: account.account.storage_root,
            account_proof: account.account_proof.clone(),
            storage_proof: account.storage_proof.clone(),
        };

        self.cache.insert(response, None, block_hash);

        // Retrieve from cache again to get the full merged result
        let (mut full_account, missing_after) = self
            .cache
            .get_account(address, slots, block_hash)
            .ok_or(eyre!("Cache inconsistency after write"))?;

        if !missing_after.is_empty() {
            return Err(eyre!("Failed to fetch all slots"));
        }

        // If code is still missing, fetch via get_account with with_code=true
        if with_code && full_account.code.is_none() {
            let expected = full_account.account.code_hash;
            let acc_with_code = self
                .inner
                .get_account(address, &[], true, block_hash.into())
                .await?;
            let code = acc_with_code
                .code
                .ok_or_else(|| eyre!("provider did not return code"))?;
            if acc_with_code.account.code_hash != expected {
                return Err(eyre!("code_hash changed while fetching code"));
            }
            self.cache.insert_code(expected, code.clone());
            full_account.code = Some(code);
        }

        Ok(full_account)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N, P> BlockProvider<N> for CachingProvider<P>
where
    N: NetworkSpec,
    P: ExecutionProvider<N>,
{
    async fn push_block(&self, block: N::BlockResponse, block_id: BlockId) {
        self.inner.push_block(block, block_id).await
    }

    async fn get_block(
        &self,
        block_id: BlockId,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        self.inner.get_block(block_id, full_tx).await
    }

    async fn get_untrusted_block(
        &self,
        block_id: BlockId,
        full_tx: bool,
    ) -> Result<Option<N::BlockResponse>> {
        self.inner.get_untrusted_block(block_id, full_tx).await
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N, P> TransactionProvider<N> for CachingProvider<P>
where
    N: NetworkSpec,
    P: ExecutionProvider<N>,
{
    async fn send_raw_transaction(&self, bytes: &[u8]) -> Result<B256> {
        self.inner.send_raw_transaction(bytes).await
    }

    async fn get_transaction(&self, hash: B256) -> Result<Option<N::TransactionResponse>> {
        self.inner.get_transaction(hash).await
    }

    async fn get_transaction_by_location(
        &self,
        block_id: BlockId,
        index: u64,
    ) -> Result<Option<N::TransactionResponse>> {
        self.inner
            .get_transaction_by_location(block_id, index)
            .await
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N, P> ReceiptProvider<N> for CachingProvider<P>
where
    N: NetworkSpec,
    P: ExecutionProvider<N>,
{
    async fn get_receipt(&self, hash: B256) -> Result<Option<N::ReceiptResponse>> {
        self.inner.get_receipt(hash).await
    }

    async fn get_block_receipts(&self, block: BlockId) -> Result<Option<Vec<N::ReceiptResponse>>> {
        self.inner.get_block_receipts(block).await
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N, P> LogProvider<N> for CachingProvider<P>
where
    N: NetworkSpec,
    P: ExecutionProvider<N>,
{
    async fn get_logs(&self, filter: &Filter) -> Result<Vec<Log>> {
        self.inner.get_logs(filter).await
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<N, P> ExecutionHintProvider<N> for CachingProvider<P>
where
    N: NetworkSpec,
    P: ExecutionProvider<N>,
{
    async fn get_execution_hint(
        &self,
        call: &N::TransactionRequest,
        validate: bool,
        block_id: BlockId,
    ) -> Result<HashMap<Address, Account>> {
        self.inner
            .get_execution_hint(call, validate, block_id)
            .await
    }
}
