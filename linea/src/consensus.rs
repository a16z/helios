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

        let block = provider
            .get_block_by_number(BlockNumberOrTag::Latest)
            .full()
            .await?
            .unwrap();

        let curr_signer = *self
            .unsafe_signer
            .lock()
            .map_err(|_| eyre!("failed to lock signer"))?;
        if verify_block(curr_signer, &block).is_ok() {
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

pub fn verify_block(curr_signer: Address, block: &Block<Transaction>) -> Result<()> {
    let extra_data = block.header.inner.extra_data.clone();

    let length = extra_data.len();
    let prefix = extra_data.slice(0..length - 65);
    let signature_bytes = extra_data.slice(length - 65..length);
    let r = signature_bytes[..32]
        .try_into()
        .expect("Failed to extract r component from signature");
    let s = signature_bytes[32..64]
        .try_into()
        .expect("Failed to extract s component from signature");
    let p = signature_bytes[64];

    let signature = Signature::from_scalars_and_parity(r, s, p == 1);

    let mut header = block.header.inner.clone();
    header.extra_data = prefix;

    let sighash = header.hash_slow();

    let pk = signature
        .recover_from_prehash(&sighash)
        .expect("Failed to recover public key from signature");
    let recovered_signer = Address::from_public_key(&pk);

    if curr_signer != recovered_signer {
        eyre::bail!("invalid signer");
    }

    Ok(())
}
