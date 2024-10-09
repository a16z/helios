use std::time::Duration;

use alloy::consensus::Transaction as TxTrait;
use alloy::primitives::{b256, fixed_bytes, keccak256, Address, B256, U256, U64};
use alloy::rlp::Decodable;
use alloy::rpc::types::{Parity, Signature, Transaction};
use alloy_rlp::encode;
use eyre::Result;
use op_alloy_consensus::OpTxEnvelope;
use tokio::sync::mpsc::Sender;
use tokio::sync::{
    mpsc::{channel, Receiver},
    watch,
};
use triehash_ethereum::ordered_trie_root;
use zduny_wasm_timer::{Delay, SystemTime, UNIX_EPOCH};

use helios_core::consensus::Consensus;
use helios_core::types::{Block, Transactions};

use crate::{config::Config, types::ExecutionPayload, SequencerCommitment};

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
            unsafe_signer: config.chain.unsafe_signer,
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
            loop {
                _ = inner.advance().await;
                Delay::new(Duration::from_secs(1)).await.unwrap();
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
    unsafe_signer: Address,
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

        if commitment.verify(self.unsafe_signer, self.chain_id).is_ok() {
            let payload = ExecutionPayload::try_from(&commitment)?;
            if self
                .latest_block
                .map(|latest| payload.block_number > latest)
                .unwrap_or(true)
            {
                let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
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
