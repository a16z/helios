use alloy::consensus::Transaction as TxTrait;
use alloy::primitives::{b256, fixed_bytes, keccak256, Address, B256, U256, U64};
use alloy::rlp::Decodable;
use alloy::rpc::types::{EIP1186AccountProofResponse, Parity, Signature, Transaction};
use alloy_rlp::encode;
use eyre::{eyre, OptionExt, Result};
use op_alloy_consensus::OpTxEnvelope;
use std::str::FromStr;
use std::time::Duration;
use tokio::sync::mpsc::Sender;
use tokio::sync::{
    mpsc::{channel, Receiver},
    watch,
};
use triehash_ethereum::ordered_trie_root;

use helios_consensus_core::consensus_spec::MainnetConsensusSpec;
use helios_core::consensus::Consensus;
use helios_core::time::{interval, SystemTime, UNIX_EPOCH};
use helios_core::types::{Block, Transactions};
use helios_ethereum::consensus::ConsensusClient as EthConsensusClient;
use std::sync::{Arc, Mutex};

use crate::{config::Config, types::ExecutionPayload, SequencerCommitment};

use helios_core::execution::proof::{encode_account, verify_proof};
use helios_ethereum::database::ConfigDB;
use helios_ethereum::rpc::http_rpc::HttpRpc;
use tracing::{error, info, warn};

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
        let (finalized_block_send, finalied_block_recv) = watch::channel(None);

        let mut inner = Inner {
            server_url: config.consensus_rpc.to_string(),
            unsafe_signer: Arc::new(Mutex::new(config.chain.unsafe_signer)),
            chain_id: config.chain.chain_id,
            latest_block: None,
            block_send,
            finalized_block_send,
        };

        verify_unsafe_signer(config.clone(), inner.unsafe_signer.clone());

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
            finalized_block_recv: Some(finalied_block_recv),
            chain_id: config.chain.chain_id,
        }
    }
}

impl Consensus<Transaction> for ConsensusClient {
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
                    self.latest_block = Some(block.number.to());
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
        let mut eth_config = config.eth_network.to_base_config();
        eth_config.load_external_fallback = config.load_external_fallback.unwrap_or(false);
        if let Some(checkpoint) = config.checkpoint {
            eth_config.default_checkpoint = checkpoint;
        }
        let mut eth_consensus = EthConsensusClient::<MainnetConsensusSpec, HttpRpc, ConfigDB>::new(
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
        let req = format!("{}unsafe_signer_proof/{}", config.consensus_rpc, block.hash);
        let proof = reqwest::get(req)
            .await?
            .json::<EIP1186AccountProofResponse>()
            .await?;

        // Verify unsafe signer
        // with account proof
        let account_path = keccak256(proof.address).to_vec();
        let account_encoded = encode_account(&proof);
        let is_valid = verify_proof(
            &proof.account_proof,
            block.state_root.as_slice(),
            &account_path,
            &account_encoded,
        );
        if !is_valid {
            warn!(target: "helios::opstack", "account proof invalid");
            return Err(eyre!("account proof invalid"));
        }
        // with storage proof
        let storage_proof = proof.storage_proof[0].clone();
        let key = storage_proof.key.0;
        if key != B256::from_str(UNSAFE_SIGNER_SLOT)? {
            warn!(target: "helios::opstack", "account proof invalid");
            return Err(eyre!("account proof invalid"));
        }
        let key_hash = keccak256(key);
        let value = encode(storage_proof.value);
        let is_valid = verify_proof(
            &storage_proof.proof,
            proof.storage_hash.as_slice(),
            key_hash.as_slice(),
            &value,
        );
        if !is_valid {
            warn!(target: "helios::opstack", "storage proof invalid");
            return Err(eyre!("storage proof invalid"));
        }
        // Replace unsafe signer if different
        let verified_signer = Address::from_slice(&storage_proof.value.to_be_bytes::<32>()[12..32]);
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
            let tx_envelope = OpTxEnvelope::decode(&mut tx_bytes_slice)?;
            let transaction_type = Some(tx_envelope.tx_type().into());

            Ok(match tx_envelope {
                OpTxEnvelope::Legacy(inner) => {
                    let inner_tx = inner.tx();
                    Transaction {
                        hash: *inner.hash(),
                        nonce: inner_tx.nonce,
                        block_hash: Some(value.block_hash),
                        block_number: Some(value.block_number),
                        transaction_index: Some(i as u64),
                        to: inner_tx.to.to().cloned(),
                        value: inner_tx.value,
                        gas_price: Some(inner_tx.gas_price),
                        gas: inner_tx.gas_limit,
                        input: inner_tx.input.to_vec().into(),
                        chain_id: inner_tx.chain_id,
                        transaction_type,
                        from: inner.recover_signer()?,
                        signature: Some(Signature {
                            r: inner.signature().r(),
                            s: inner.signature().s(),
                            v: U256::from(inner.signature().v().to_u64()),
                            y_parity: None,
                        }),
                        ..Default::default()
                    }
                }
                OpTxEnvelope::Eip2930(inner) => {
                    let inner_tx = inner.tx();
                    Transaction {
                        hash: *inner.hash(),
                        nonce: inner_tx.nonce,
                        block_hash: Some(value.block_hash),
                        block_number: Some(value.block_number),
                        transaction_index: Some(i as u64),
                        to: inner_tx.to.to().cloned(),
                        value: inner_tx.value,
                        gas_price: Some(inner_tx.gas_price),
                        gas: inner_tx.gas_limit,
                        input: inner_tx.input.to_vec().into(),
                        chain_id: Some(inner_tx.chain_id),
                        transaction_type,
                        from: inner.recover_signer()?,
                        signature: Some(Signature {
                            r: inner.signature().r(),
                            s: inner.signature().s(),
                            v: U256::from(inner.signature().v().to_u64()),
                            y_parity: Some(Parity(inner.signature().v().to_u64() == 1)),
                        }),
                        access_list: Some(inner.tx().access_list.clone()),
                        ..Default::default()
                    }
                }
                OpTxEnvelope::Eip1559(inner) => {
                    let inner_tx = inner.tx();
                    Transaction {
                        hash: *inner.hash(),
                        nonce: inner_tx.nonce,
                        block_hash: Some(value.block_hash),
                        block_number: Some(value.block_number),
                        transaction_index: Some(i as u64),
                        to: inner_tx.to.to().cloned(),
                        value: inner_tx.value,
                        gas_price: inner_tx.gas_price(),
                        gas: inner_tx.gas_limit,
                        input: inner_tx.input.to_vec().into(),
                        chain_id: Some(inner_tx.chain_id),
                        transaction_type,
                        from: inner.recover_signer()?,
                        signature: Some(Signature {
                            r: inner.signature().r(),
                            s: inner.signature().s(),
                            v: U256::from(inner.signature().v().to_u64()),
                            y_parity: Some(Parity(inner.signature().v().to_u64() == 1)),
                        }),
                        access_list: Some(inner_tx.access_list.clone()),
                        max_fee_per_gas: Some(inner_tx.max_fee_per_gas),
                        max_priority_fee_per_gas: Some(inner_tx.max_priority_fee_per_gas),
                        ..Default::default()
                    }
                }
                OpTxEnvelope::Eip4844(inner) => {
                    let inner_tx = inner.tx();
                    Transaction {
                        hash: *inner.hash(),
                        nonce: inner_tx.nonce(),
                        block_hash: Some(value.block_hash),
                        block_number: Some(value.block_number),
                        transaction_index: Some(i as u64),
                        to: inner_tx.to().to().cloned(),
                        value: inner_tx.value(),
                        gas_price: inner_tx.gas_price(),
                        gas: inner_tx.gas_limit(),
                        input: inner_tx.input().to_vec().into(),
                        chain_id: inner_tx.chain_id(),
                        transaction_type,
                        from: inner.recover_signer()?,
                        signature: Some(Signature {
                            r: inner.signature().r(),
                            s: inner.signature().s(),
                            v: U256::from(inner.signature().v().to_u64()),
                            y_parity: Some(Parity(inner.signature().v().to_u64() == 1)),
                        }),
                        access_list: Some(inner_tx.tx().access_list.clone()),
                        max_fee_per_gas: Some(inner_tx.tx().max_fee_per_gas),
                        max_priority_fee_per_gas: Some(inner_tx.tx().max_priority_fee_per_gas),
                        max_fee_per_blob_gas: Some(inner_tx.tx().max_fee_per_blob_gas),
                        blob_versioned_hashes: Some(inner_tx.tx().blob_versioned_hashes.clone()),
                        ..Default::default()
                    }
                }
                OpTxEnvelope::Deposit(inner) => {
                    let hash =
                        keccak256([&[0x7Eu8], alloy::rlp::encode(&inner).as_slice()].concat());

                    let tx = Transaction {
                        hash,
                        nonce: inner.nonce(),
                        block_hash: Some(value.block_hash),
                        block_number: Some(value.block_number),
                        transaction_index: Some(i as u64),
                        to: inner.to().to().cloned(),
                        value: inner.value(),
                        gas_price: inner.gas_price(),
                        gas: inner.gas_limit(),
                        input: inner.input().to_vec().into(),
                        chain_id: inner.chain_id(),
                        transaction_type,
                        ..Default::default()
                    };

                    tx
                }
                _ => unreachable!("new tx type"),
            })
        })
        .collect::<Result<Vec<Transaction>>>()?;

    let raw_txs = value.transactions.iter().map(|tx| tx.to_vec());
    let txs_root = ordered_trie_root(raw_txs);

    let withdrawals = value.withdrawals.iter().map(|v| encode(v));
    let withdrawals_root = ordered_trie_root(withdrawals);

    Ok(Block {
        number: U64::from(value.block_number),
        base_fee_per_gas: value.base_fee_per_gas,
        difficulty: U256::ZERO,
        extra_data: value.extra_data.to_vec().into(),
        gas_limit: U64::from(value.gas_limit),
        gas_used: U64::from(value.gas_used),
        hash: value.block_hash,
        logs_bloom: value.logs_bloom.to_vec().into(),
        miner: value.fee_recipient,
        parent_hash: value.parent_hash,
        receipts_root: value.receipts_root,
        state_root: value.state_root,
        timestamp: U64::from(value.timestamp),
        total_difficulty: U64::ZERO,
        transactions: Transactions::Full(txs),
        mix_hash: value.prev_randao,
        nonce: empty_nonce,
        sha3_uncles: empty_uncle_hash,
        size: U64::ZERO,
        transactions_root: B256::from_slice(txs_root.as_bytes()),
        withdrawals_root: B256::from_slice(withdrawals_root.as_bytes()),
        uncles: vec![],
        blob_gas_used: Some(U64::from(value.blob_gas_used)),
        excess_blob_gas: Some(U64::from(value.excess_blob_gas)),
        parent_beacon_block_root: None,
    })
}
