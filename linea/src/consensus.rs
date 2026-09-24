use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use alloy::{
    eips::BlockNumberOrTag,
    primitives::{Address, B256},
    providers::{Provider, ProviderBuilder},
    rpc::types::{Block, Transaction},
    signers::Signature,
    transports::http::reqwest::Url,
};

use tokio::sync::{
    mpsc::{channel, Receiver, Sender},
    watch,
};

use helios_core::{
    consensus::Consensus,
    time::{interval, SystemTime, UNIX_EPOCH},
};

use eyre::{eyre, Result};
use tracing::error;

use crate::config::Config;

pub struct ConsensusClient {
    block_recv: Option<Receiver<Block<Transaction>>>,
    finalized_block_recv: Option<watch::Receiver<Option<Block<Transaction>>>>,
    chain_id: u64,
}

impl ConsensusClient {
    pub fn new(config: &Config) -> Self {
        let (block_send, block_recv) = channel(256);
        let (finalized_block_send, finalized_block_recv) = watch::channel(None);

        let mut inner = Inner {
            server_url: config.execution_rpc.to_string(),
            unsafe_signer: Arc::new(Mutex::new(config.chain.unsafe_signer)),
            chain_id: config.chain.chain_id,
            latest_block: None,
            block_send,
            finalized_block_send,
        };

        #[cfg(not(target_arch = "wasm32"))]
        let run = tokio::spawn;

        #[cfg(target_arch = "wasm32")]
        let run = wasm_bindgen_futures::spawn_local;

        run(async move {
            let mut interval = interval(Duration::from_secs(1));
            loop {
                if let Err(e) = inner.advance().await {
                    error!(target: "helios::linea", "failed to advance: {}", e);
                }
                interval.tick().await;
            }
        });

        Self {
            block_recv: Some(block_recv),
            finalized_block_recv: Some(finalized_block_recv),
            chain_id: config.chain.chain_id,
        }
    }
}

#[async_trait::async_trait]
impl Consensus<Block<Transaction>> for ConsensusClient {
    fn chain_id(&self) -> u64 {
        self.chain_id
    }

    fn shutdown(&self) -> eyre::Result<()> {
        Ok(())
    }

    fn block_recv(&mut self) -> Option<Receiver<Block<Transaction>>> {
        self.block_recv.take()
    }

    fn finalized_block_recv(&mut self) -> Option<watch::Receiver<Option<Block<Transaction>>>> {
        self.finalized_block_recv.take()
    }

    fn checkpoint_recv(&self) -> Option<watch::Receiver<Option<B256>>> {
        None
    }

    fn expected_highest_block(&self) -> u64 {
        u64::MAX
    }

    async fn wait_synced(&self) -> eyre::Result<()> {
        // Linea consensus doesn't have a sync process, so immediately return Ok
        Ok(())
    }
}

#[allow(dead_code)]
struct Inner {
    server_url: String,
    unsafe_signer: Arc<Mutex<Address>>,
    chain_id: u64,
    latest_block: Option<u64>,
    block_send: Sender<Block<Transaction>>,
    finalized_block_send: watch::Sender<Option<Block<Transaction>>>,
}

impl Inner {
    pub async fn advance(&mut self) -> Result<()> {
        let rpc_url = Url::parse(self.server_url.as_str())?;
        let provider = ProviderBuilder::new().connect_http(rpc_url);

        let mut block = provider
            .get_block_by_number(BlockNumberOrTag::Latest)
            .full()
            .await?
            .ok_or_else(|| eyre!("latest block not found"))?;

        let curr_signer = *self
            .unsafe_signer
            .lock()
            .map_err(|_| eyre!("failed to lock signer"))?;
        if verify_block(curr_signer, &mut block).is_ok() {
            let number = block.header.number;
            if self
                .latest_block
                .map(|latest| number > latest)
                .unwrap_or(true)
            {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default();

                let timestamp = Duration::from_secs(block.header.timestamp);
                let age = now.saturating_sub(timestamp);

                self.latest_block = Some(number);
                _ = self.block_send.send(block).await;

                tracing::debug!(
                    "unsafe head updated: block={} age={}s",
                    number,
                    age.as_secs()
                );
            }
        }

        Ok(())
    }
}

pub fn verify_block(curr_signer: Address, block: &mut Block<Transaction>) -> Result<()> {
    use crate::spec::Linea;
    use alloy::{
        consensus::{
            proofs::calculate_withdrawals_root, transaction::SignerRecoverable, Transaction as _,
        },
        eips::Encodable2718,
        primitives::keccak256,
        rpc::types::BlockTransactions,
    };
    use helios_common::network_spec::NetworkSpec;

    if !Linea::is_hash_valid(block) {
        eyre::bail!("invalid block hash or body");
    }
    let withdrawals_root = block
        .withdrawals
        .as_ref()
        .map(|w| calculate_withdrawals_root(w));
    if withdrawals_root != block.header.withdrawals_root || !block.uncles.is_empty() {
        eyre::bail!("invalid block body");
    }
    let BlockTransactions::Full(txs) = &mut block.transactions else {
        eyre::bail!("missing full transactions");
    };
    for (index, tx) in txs.iter_mut().enumerate() {
        if keccak256(tx.inner.encoded_2718()) != *tx.inner.tx_hash()
            || tx.inner.inner().recover_signer().ok() != Some(tx.inner.signer())
            || tx.block_hash != Some(block.header.hash)
            || tx.block_number != Some(block.header.number)
            || tx.transaction_index != Some(index as u64)
        {
            eyre::bail!("invalid transaction metadata");
        }
        tx.effective_gas_price = Some(tx.effective_gas_price(block.header.base_fee_per_gas));
    }
    let extra_data = &block.header.extra_data;
    let prefix_length = extra_data
        .len()
        .checked_sub(65)
        .ok_or_else(|| eyre!("missing sequencer signature"))?;
    let signature_bytes = &extra_data[prefix_length..];
    if signature_bytes[64] > 1 {
        eyre::bail!("invalid signature recovery id");
    }
    let signature = Signature::from_scalars_and_parity(
        signature_bytes[..32].try_into()?,
        signature_bytes[32..64].try_into()?,
        signature_bytes[64] == 1,
    );
    let mut header = block.header.inner.clone();
    header.extra_data = extra_data.slice(..prefix_length);
    let recovered_signer = signature.recover_address_from_prehash(&header.hash_slow())?;
    if curr_signer != recovered_signer {
        eyre::bail!("invalid signer");
    }
    block.header.size = None;
    block.header.total_difficulty = None;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::{
        consensus::proofs::calculate_transaction_root,
        primitives::{Signature, U256},
        rpc::types::BlockTransactions,
    };

    fn signed_block() -> (Address, Block<Transaction>) {
        let mut block: Block<Transaction> = Block::default();
        block.transactions = BlockTransactions::Full(vec![]);
        block.header.transactions_root =
            calculate_transaction_root::<alloy::consensus::TxEnvelope>(&[]);
        let signature = Signature::new(U256::from(1), U256::from(2), false);
        let signer = signature
            .recover_address_from_prehash(&block.header.hash_slow())
            .unwrap();
        let mut signature_bytes = signature.as_bytes();
        signature_bytes[64] = 0;
        block.header.extra_data = signature_bytes.to_vec().into();
        block.header.hash = block.header.hash_slow();
        (signer, block)
    }

    #[test]
    fn authenticates_the_body_and_cached_hash() {
        let (signer, block) = signed_block();
        verify_block(signer, &mut block.clone()).unwrap();
        let mut forged = block.clone();
        forged.header.hash = B256::ZERO;
        assert!(verify_block(signer, &mut forged).is_err());
        let mut forged = block.clone();
        forged.transactions = BlockTransactions::Hashes(vec![B256::ZERO]);
        assert!(verify_block(signer, &mut forged).is_err());
        let mut forged = block.clone();
        forged.withdrawals = Some(vec![Default::default()].into());
        assert!(verify_block(signer, &mut forged).is_err());
        let mut forged = block;
        forged.uncles.push(B256::ZERO);
        assert!(verify_block(signer, &mut forged).is_err());
    }

    #[test]
    fn malformed_signatures_return_errors() {
        let (signer, mut block) = signed_block();
        for extra in [vec![], vec![0; 64], vec![0; 65], vec![2; 65]] {
            block.header.extra_data = extra.into();
            block.header.hash = block.header.hash_slow();
            assert!(verify_block(signer, &mut block).is_err());
        }
    }
}
