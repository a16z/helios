use alloy::{
    eips::BlockId,
    primitives::{Address, B256, U256},
};
use async_trait::async_trait;
use eyre::{Error, Result};
use reqwest::Client;

use helios_core::network_spec::NetworkSpec;

use crate::types::*;

#[async_trait]
pub trait VerifiableApi<N: NetworkSpec> {
    async fn get_account(
        &self,
        address: Address,
        storage_keys: Vec<B256>,
        block: Option<BlockId>,
    ) -> Result<GetAccountProofResponse, Error>;
    async fn get_balance(
        &self,
        address: Address,
        block: Option<BlockId>,
    ) -> Result<GetBalanceResponse, Error>;
    async fn get_transaction_count(
        &self,
        address: Address,
        block: Option<BlockId>,
    ) -> Result<GetTransactionCountResponse, Error>;
    async fn get_code(
        &self,
        address: Address,
        block: Option<BlockId>,
    ) -> Result<GetCodeResponse, Error>;
    async fn get_storage_at(
        &self,
        address: Address,
        key: U256,
        block: Option<BlockId>,
    ) -> Result<GetStorageAtResponse, Error>;
    async fn get_block_receipts(
        &self,
        block: BlockId,
    ) -> Result<GetBlockReceiptsResponse<N>, Error>;
    async fn get_transaction_receipt(
        &self,
        tx_hash: B256,
    ) -> Result<GetTransactionReceiptResponse<N>, Error>;
    async fn get_filter_logs(&self, filter_id: U256) -> Result<GetFilterLogsResponse<N>, Error>;
    async fn get_filter_changes(
        &self,
        filter_id: U256,
    ) -> Result<GetFilterChangesResponse<N>, Error>;
}

pub struct VerifiableApiClient {
    client: Client,
    base_url: String,
}

impl VerifiableApiClient {
    pub fn new(base_url: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
        }
    }
}

#[async_trait]
impl<N: NetworkSpec> VerifiableApi<N> for VerifiableApiClient {
    async fn get_account(
        &self,
        address: Address,
        storage_keys: Vec<B256>,
        block: Option<BlockId>,
    ) -> Result<GetAccountProofResponse, Error> {
        let url = format!("{}/eth/v1/proof/account/{}", self.base_url, address);
        let response = self
            .client
            .get(&url)
            .query(&[("block", block)])
            .query(&[("storage_keys", &storage_keys)])
            .send()
            .await?;
        let response = response.json::<GetAccountProofResponse>().await?;
        Ok(response)
    }

    async fn get_balance(
        &self,
        address: Address,
        block: Option<BlockId>,
    ) -> Result<GetBalanceResponse, Error> {
        let url = format!("{}/eth/v1/proof/balance/{}", self.base_url, address);
        let response = self
            .client
            .get(&url)
            .query(&[("block", block)])
            .send()
            .await?;
        let response = response.json::<GetBalanceResponse>().await?;
        Ok(response)
    }

    async fn get_transaction_count(
        &self,
        address: Address,
        block: Option<BlockId>,
    ) -> Result<GetTransactionCountResponse, Error> {
        let url = format!(
            "{}/eth/v1/proof/transaction_count/{}",
            self.base_url, address
        );
        let response = self
            .client
            .get(&url)
            .query(&[("block", block)])
            .send()
            .await?;
        let response = response.json::<GetTransactionCountResponse>().await?;
        Ok(response)
    }

    async fn get_code(
        &self,
        address: Address,
        block: Option<BlockId>,
    ) -> Result<GetCodeResponse, Error> {
        let url = format!("{}/eth/v1/proof/code/{}", self.base_url, address);
        let response = self
            .client
            .get(&url)
            .query(&[("block", block)])
            .send()
            .await?;
        let response = response.json::<GetCodeResponse>().await?;
        Ok(response)
    }

    async fn get_storage_at(
        &self,
        address: Address,
        key: U256,
        block: Option<BlockId>,
    ) -> Result<GetStorageAtResponse, Error> {
        let url = format!("{}/eth/v1/proof/storage/{}/{}", self.base_url, address, key);
        let response = self
            .client
            .get(&url)
            .query(&[("block", block)])
            .send()
            .await?;
        let response = response.json::<GetStorageAtResponse>().await?;
        Ok(response)
    }

    async fn get_block_receipts(
        &self,
        block: BlockId,
    ) -> Result<GetBlockReceiptsResponse<N>, Error> {
        let url = format!("{}/eth/v1/proof/block_receipts/{}", self.base_url, block);
        let response = self.client.get(&url).send().await?;
        let response = response.json::<GetBlockReceiptsResponse<N>>().await?;
        Ok(response)
    }

    async fn get_transaction_receipt(
        &self,
        tx_hash: B256,
    ) -> Result<GetTransactionReceiptResponse<N>, Error> {
        let url = format!("{}/eth/v1/proof/tx_receipt/{}", self.base_url, tx_hash);
        let response = self.client.get(&url).send().await?;
        let response = response.json::<GetTransactionReceiptResponse<N>>().await?;
        Ok(response)
    }

    async fn get_filter_logs(&self, filter_id: U256) -> Result<GetFilterLogsResponse<N>, Error> {
        let url = format!("{}/eth/v1/proof/filter_logs/{}", self.base_url, filter_id);
        let response = self.client.get(&url).send().await?;
        let response = response.json::<GetFilterLogsResponse<N>>().await?;
        Ok(response)
    }

    async fn get_filter_changes(
        &self,
        filter_id: U256,
    ) -> Result<GetFilterChangesResponse<N>, Error> {
        let url = format!(
            "{}/eth/v1/proof/filter_changes/{}",
            self.base_url, filter_id
        );
        let response = self.client.get(&url).send().await?;
        let response = response.json::<GetFilterChangesResponse<N>>().await?;
        Ok(response)
    }
}
