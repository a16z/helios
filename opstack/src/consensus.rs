use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use alloy::consensus::proofs::{calculate_transaction_root, calculate_withdrawals_root};
use alloy::consensus::{Header as ConsensusHeader, Transaction as TxTrait};
use alloy::eips::eip4895::{Withdrawal, Withdrawals};
use alloy::primitives::{b256, fixed_bytes, Address, Bloom, BloomInput, B256, U256};
use alloy::rlp::Decodable;
use alloy::rpc::types::{
    Block, EIP1186AccountProofResponse, Header, Transaction as EthTransaction,
};
use eyre::{eyre, OptionExt, Result};
use op_alloy_consensus::OpTxEnvelope;
use op_alloy_network::primitives::BlockTransactions;
use op_alloy_rpc_types::Transaction;
use tokio::sync::mpsc::Sender;
use tokio::sync::{
    mpsc::{channel, Receiver},
    watch,
};
use tracing::{error, info, warn};

use helios_consensus_core::consensus_spec::MainnetConsensusSpec;
use helios_core::consensus::Consensus;
use helios_core::execution::proof::{verify_account_proof, verify_mpt_proof};
use helios_core::time::{interval, SystemTime, UNIX_EPOCH};
use helios_ethereum::consensus::ConsensusClient as EthConsensusClient;

use helios_ethereum::database::ConfigDB;
use helios_ethereum::rpc::http_rpc::HttpRpc;

use crate::{config::Config, types::ExecutionPayload, SequencerCommitment};

// Storage slot containing the unsafe signer address in all superchain system config contracts
const UNSAFE_SIGNER_SLOT: &str =
    "0x65a7ed542fb37fe237fdfbdd70b31598523fe5b32879e307bae27a0bd9581c08";

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
            server_url: config.consensus_rpc.to_string(),
            unsafe_signer: Arc::new(Mutex::new(config.chain.unsafe_signer)),
            chain_id: config.chain.chain_id,
            latest_block: None,
            block_send,
            finalized_block_send,
        };

        if config.verify_unsafe_signer {
            verify_unsafe_signer(config.clone(), inner.unsafe_signer.clone());
        }

        #[cfg(not(target_arch = "wasm32"))]
        let run = tokio::spawn;

        #[cfg(target_arch = "wasm32")]
        let run = wasm_bindgen_futures::spawn_local;

        run(async move {
            let mut interval = interval(Duration::from_secs(1));
            loop {
                if let Err(e) = inner.advance().await {
                    error!(target: "helios::opstack", "failed to advance: {}", e);
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

    fn expected_highest_block(&self) -> u64 {
        u64::MAX
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
        let req = format!("{}latest", self.server_url);
        let commitment = reqwest::get(req)
            .await?
            .json::<SequencerCommitment>()
            .await?;

        let curr_signer = *self
            .unsafe_signer
            .lock()
            .map_err(|_| eyre!("failed to lock signer"))?;
        if commitment.verify(curr_signer, self.chain_id).is_ok() {
            let payload = ExecutionPayload::try_from(&commitment)?;
            if self
                .latest_block
                .map(|latest| payload.block_number > latest)
                .unwrap_or(true)
            {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_else(|_| panic!("unreachable"));

                let timestamp = Duration::from_secs(payload.timestamp);
                let age = now.saturating_sub(timestamp);
                let number = payload.block_number;

                if let Ok(block) = payload_to_block(payload) {
                    self.latest_block = Some(block.header.number);
                    _ = self.block_send.send(block).await;

                    tracing::info!(
                        "unsafe head updated: block={} age={}s",
                        number,
                        age.as_secs()
                    );
                } else {
                    tracing::warn!("invalid block received");
                }
            }
        }

        Ok(())
    }
}

fn verify_unsafe_signer(config: Config, signer: Arc<Mutex<Address>>) {
    #[cfg(not(target_arch = "wasm32"))]
    let run = tokio::spawn;

    #[cfg(target_arch = "wasm32")]
    let run = wasm_bindgen_futures::spawn_local;

    run(async move {
        let fut = async move {
            let mut eth_config = config.chain.eth_network.to_base_config();
            eth_config.load_external_fallback = config.load_external_fallback.unwrap_or(false);

            if let Some(checkpoint) = config.checkpoint {
                eth_config.default_checkpoint = checkpoint;
            }

            let mut eth_consensus =
                EthConsensusClient::<MainnetConsensusSpec, HttpRpc, ConfigDB>::new(
                    &eth_config
                        .consensus_rpc
                        .clone()
                        .ok_or_else(|| eyre!("missing consensus rpc"))?,
                    Arc::new(eth_config.into()),
                )?;

            let block = eth_consensus
                .block_recv()
                .unwrap()
                .recv()
                .await
                .ok_or_eyre("failed to receive block")?;

            // Query proof from op consensus server
            let req = format!(
                "{}unsafe_signer_proof/{}",
                config.consensus_rpc, block.header.hash
            );
            let proof = reqwest::get(req)
                .await?
                .json::<EIP1186AccountProofResponse>()
                .await?;

            // Verify unsafe signer
            // with account proof
            if verify_account_proof(&proof, block.header.state_root).is_err() {
                warn!(target: "helios::opstack", "account proof invalid");
                return Err(eyre!("account proof invalid"));
            }

            // with storage proof
            let storage_proof = proof.storage_proof[0].clone();
            let key = storage_proof.key.as_b256();
            if key != B256::from_str(UNSAFE_SIGNER_SLOT)? {
                warn!(target: "helios::opstack", "account proof invalid");
                return Err(eyre!("account proof invalid"));
            }

            if verify_mpt_proof(
                proof.storage_hash,
                key,
                storage_proof.value,
                &storage_proof.proof,
            )
            .is_err()
            {
                warn!(target: "helios::opstack", "storage proof invalid");
                return Err(eyre!("storage proof invalid"));
            }

            // Replace unsafe signer if different
            let verified_signer =
                Address::from_slice(&storage_proof.value.to_be_bytes::<32>()[12..32]);
            {
                let mut curr_signer = signer.lock().map_err(|_| eyre!("failed to lock signer"))?;
                if verified_signer != *curr_signer {
                    info!(target: "helios::opstack", "unsafe signer updated: {}", verified_signer);
                    *curr_signer = verified_signer;
                }
            }

            // Shutdown eth consensus client
            eth_consensus.shutdown()?;

            Ok(())
        };

        _ = fut.await;
    });
}

fn payload_to_block(value: ExecutionPayload) -> Result<Block<Transaction>> {
    let empty_nonce = fixed_bytes!("0000000000000000");
    let empty_uncle_hash =
        b256!("1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347");

    let txs = value
        .transactions
        .iter()
        .enumerate()
        .map(|(i, tx_bytes)| {
            let tx_bytes = tx_bytes.to_vec();
            let mut tx_bytes_slice = tx_bytes.as_slice();
            let tx_envelope = OpTxEnvelope::decode(&mut tx_bytes_slice).unwrap();
            let base_fee = tx_envelope.effective_gas_price(Some(value.base_fee_per_gas.to()));
            let recovered = tx_envelope.try_into_recovered()?;

            let inner_tx = EthTransaction {
                inner: recovered,
                block_hash: Some(value.block_hash),
                block_number: Some(value.block_number),
                transaction_index: Some(i as u64),
                effective_gas_price: Some(base_fee),
            };

            Ok(match inner_tx.inner.inner() {
                OpTxEnvelope::Legacy(_)
                | OpTxEnvelope::Eip2930(_)
                | OpTxEnvelope::Eip1559(_)
                | OpTxEnvelope::Eip7702(_) => Transaction {
                    inner: inner_tx,
                    deposit_nonce: None,
                    deposit_receipt_version: None,
                },
                OpTxEnvelope::Deposit(inner) => {
                    let deposit_nonce = Some(inner.inner().nonce());

                    Transaction {
                        inner: inner_tx,
                        deposit_nonce,
                        deposit_receipt_version: None,
                    }
                }
            })
        })
        .collect::<Result<Vec<Transaction>>>()?;
    let tx_envelopes = txs
        .iter()
        .map(|tx| tx.inner.inner.clone())
        .collect::<Vec<_>>();
    let txs_root = calculate_transaction_root(&tx_envelopes);

    let withdrawals: Vec<Withdrawal> = value.withdrawals.into_iter().map(|w| w.into()).collect();
    let withdrawals_root = calculate_withdrawals_root(&withdrawals);

    let logs_bloom: Bloom = Bloom::from(BloomInput::Raw(&value.logs_bloom));

    let consensus_header = ConsensusHeader {
        parent_hash: value.parent_hash,
        ommers_hash: empty_uncle_hash,
        beneficiary: Address::from(*value.fee_recipient),
        state_root: value.state_root,
        transactions_root: txs_root,
        receipts_root: value.receipts_root,
        withdrawals_root: Some(withdrawals_root),
        difficulty: U256::ZERO,
        number: value.block_number,
        gas_limit: value.gas_limit,
        gas_used: value.gas_used,
        timestamp: value.timestamp,
        mix_hash: value.prev_randao,
        nonce: empty_nonce,
        base_fee_per_gas: Some(value.base_fee_per_gas.to::<u64>()),
        blob_gas_used: Some(value.blob_gas_used),
        excess_blob_gas: Some(value.excess_blob_gas),
        parent_beacon_block_root: None,
        extra_data: value.extra_data.to_vec().into(),
        requests_hash: None,
        logs_bloom,
    };

    let header = Header {
        hash: value.block_hash,
        inner: consensus_header,
        total_difficulty: Some(U256::ZERO),
        size: Some(U256::ZERO),
    };

    Ok(Block::new(header, BlockTransactions::Full(txs))
        .with_withdrawals(Some(Withdrawals::new(withdrawals))))
}
