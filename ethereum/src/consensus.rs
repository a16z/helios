use std::marker::PhantomData;
use std::process;
use std::sync::Arc;

use alloy::consensus::{Transaction as TxTrait, TxEnvelope};
use alloy::primitives::{b256, fixed_bytes, B256, U256, U64};
use alloy::rlp::{encode, Decodable};
use alloy::rpc::types::{Parity, Signature, Transaction};
use chrono::Duration;
use eyre::eyre;
use eyre::Result;
use futures::future::join_all;
use tracing::{debug, error, info, warn};
use tree_hash::TreeHash;
use triehash_ethereum::ordered_trie_root;

use tokio::sync::mpsc::channel;
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;
use tokio::sync::watch;

use helios_consensus_core::{
    apply_bootstrap, apply_finality_update, apply_update, calc_sync_period,
    consensus_spec::ConsensusSpec,
    errors::ConsensusError,
    expected_current_slot, get_bits,
    types::{ExecutionPayload, FinalityUpdate, LightClientStore, Update},
    verify_bootstrap, verify_finality_update, verify_update,
};
use helios_core::consensus::Consensus;
use helios_core::time::{interval_at, Instant, SystemTime, UNIX_EPOCH};
use helios_core::types::{Block, Transactions};

use crate::config::checkpoints::CheckpointFallback;
use crate::config::networks::Network;
use crate::config::Config;
use crate::constants::MAX_REQUEST_LIGHT_CLIENT_UPDATES;
use crate::database::Database;
use crate::rpc::ConsensusRpc;

pub struct ConsensusClient<S: ConsensusSpec, R: ConsensusRpc<S>, DB: Database> {
    pub block_recv: Option<Receiver<Block<Transaction>>>,
    pub finalized_block_recv: Option<watch::Receiver<Option<Block<Transaction>>>>,
    pub checkpoint_recv: watch::Receiver<Option<B256>>,
    genesis_time: u64,
    db: DB,
    config: Arc<Config>,
    phantom: PhantomData<(S, R)>,
}

#[derive(Debug)]
pub struct Inner<S: ConsensusSpec, R: ConsensusRpc<S>> {
    pub rpc: R,
    pub store: LightClientStore<S>,
    last_checkpoint: Option<B256>,
    block_send: Sender<Block<Transaction>>,
    finalized_block_send: watch::Sender<Option<Block<Transaction>>>,
    checkpoint_send: watch::Sender<Option<B256>>,
    pub config: Arc<Config>,
    phantom: PhantomData<S>,
}

impl<S: ConsensusSpec, R: ConsensusRpc<S>, DB: Database> Consensus<Transaction>
    for ConsensusClient<S, R, DB>
{
    fn block_recv(&mut self) -> Option<Receiver<Block<Transaction>>> {
        self.block_recv.take()
    }

    fn finalized_block_recv(&mut self) -> Option<watch::Receiver<Option<Block<Transaction>>>> {
        self.finalized_block_recv.take()
    }

    fn expected_highest_block(&self) -> u64 {
        u64::MAX
    }

    fn chain_id(&self) -> u64 {
        self.config.chain.chain_id
    }

    fn shutdown(&self) -> Result<()> {
        let checkpoint = self.checkpoint_recv.borrow();
        if let Some(checkpoint) = checkpoint.as_ref() {
            self.db.save_checkpoint(*checkpoint)?;
        }

        Ok(())
    }
}

impl<S: ConsensusSpec, R: ConsensusRpc<S>, DB: Database> ConsensusClient<S, R, DB> {
    pub fn new(rpc: &str, config: Arc<Config>) -> Result<ConsensusClient<S, R, DB>> {
        let (block_send, block_recv) = channel(256);
        let (finalized_block_send, finalized_block_recv) = watch::channel(None);
        let (checkpoint_send, checkpoint_recv) = watch::channel(None);

        let config_clone = config.clone();
        let rpc = rpc.to_string();
        let genesis_time = config.chain.genesis_time;
        let db = DB::new(&config)?;
        let initial_checkpoint = config
            .checkpoint
            .unwrap_or_else(|| db.load_checkpoint().unwrap_or(config.default_checkpoint));

        #[cfg(not(target_arch = "wasm32"))]
        let run = tokio::spawn;

        #[cfg(target_arch = "wasm32")]
        let run = wasm_bindgen_futures::spawn_local;

        run(async move {
            let mut inner = Inner::<S, R>::new(
                &rpc,
                block_send,
                finalized_block_send,
                checkpoint_send,
                config.clone(),
            );

            let res = inner.sync(initial_checkpoint).await;
            if let Err(err) = res {
                if config.load_external_fallback {
                    let res = sync_all_fallbacks(&mut inner, config.chain.chain_id).await;
                    if let Err(err) = res {
                        error!(target: "helios::consensus", err = %err, "sync failed");
                        process::exit(1);
                    }
                } else if let Some(fallback) = &config.fallback {
                    let res = sync_fallback(&mut inner, fallback).await;
                    if let Err(err) = res {
                        error!(target: "helios::consensus", err = %err, "sync failed");
                        process::exit(1);
                    }
                } else {
                    error!(target: "helios::consensus", err = %err, "sync failed");
                    process::exit(1);
                }
            }

            _ = inner.send_blocks().await;

            let start = Instant::now() + inner.duration_until_next_update().to_std().unwrap();
            let mut interval = interval_at(start, std::time::Duration::from_secs(12));

            loop {
                interval.tick().await;

                let res = inner.advance().await;
                if let Err(err) = res {
                    warn!(target: "helios::consensus", "advance error: {}", err);
                    continue;
                }

                let res = inner.send_blocks().await;
                if let Err(err) = res {
                    warn!(target: "helios::consensus", "send error: {}", err);
                    continue;
                }
            }
        });

        Ok(ConsensusClient {
            block_recv: Some(block_recv),
            finalized_block_recv: Some(finalized_block_recv),
            checkpoint_recv,
            genesis_time,
            db,
            config: config_clone,
            phantom: PhantomData,
        })
    }

    pub fn expected_current_slot(&self) -> u64 {
        let now = SystemTime::now();

        expected_current_slot(now, self.genesis_time)
    }
}

async fn sync_fallback<S: ConsensusSpec, R: ConsensusRpc<S>>(
    inner: &mut Inner<S, R>,
    fallback: &str,
) -> Result<()> {
    let checkpoint = CheckpointFallback::fetch_checkpoint_from_api(fallback).await?;
    inner.sync(checkpoint).await
}

async fn sync_all_fallbacks<S: ConsensusSpec, R: ConsensusRpc<S>>(
    inner: &mut Inner<S, R>,
    chain_id: u64,
) -> Result<()> {
    let network = Network::from_chain_id(chain_id)?;
    let checkpoint = CheckpointFallback::new()
        .build()
        .await?
        .fetch_latest_checkpoint(&network)
        .await?;

    inner.sync(checkpoint).await
}

impl<S: ConsensusSpec, R: ConsensusRpc<S>> Inner<S, R> {
    pub fn new(
        rpc: &str,
        block_send: Sender<Block<Transaction>>,
        finalized_block_send: watch::Sender<Option<Block<Transaction>>>,
        checkpoint_send: watch::Sender<Option<B256>>,
        config: Arc<Config>,
    ) -> Inner<S, R> {
        let rpc = R::new(rpc);

        Inner {
            rpc,
            store: LightClientStore::default(),
            last_checkpoint: None,
            block_send,
            finalized_block_send,
            checkpoint_send,
            config,
            phantom: PhantomData,
        }
    }

    pub async fn check_rpc(&self) -> Result<()> {
        let chain_id = self.rpc.chain_id().await?;

        if chain_id != self.config.chain.chain_id {
            Err(ConsensusError::IncorrectRpcNetwork.into())
        } else {
            Ok(())
        }
    }

    pub async fn get_execution_payload(&self, slot: &Option<u64>) -> Result<ExecutionPayload<S>> {
        let slot = slot.unwrap_or(self.store.optimistic_header.beacon().slot);
        let block = self.rpc.get_block(slot).await?;
        let block_hash = block.tree_hash_root();

        let latest_slot = self.store.optimistic_header.beacon().slot;
        let finalized_slot = self.store.finalized_header.beacon().slot;

        let verified_block_hash = if slot == latest_slot {
            self.store.optimistic_header.beacon().tree_hash_root()
        } else if slot == finalized_slot {
            self.store.finalized_header.beacon().tree_hash_root()
        } else {
            return Err(ConsensusError::PayloadNotFound(slot).into());
        };

        if verified_block_hash != block_hash {
            Err(ConsensusError::InvalidHeaderHash(block_hash, verified_block_hash).into())
        } else {
            Ok(block.body.execution_payload().clone())
        }
    }

    pub async fn get_payloads(
        &self,
        start_slot: u64,
        end_slot: u64,
    ) -> Result<Vec<ExecutionPayload<S>>> {
        let payloads_fut = (start_slot..end_slot)
            .rev()
            .map(|slot| self.rpc.get_block(slot));

        let mut prev_parent_hash: B256 = *self
            .rpc
            .get_block(end_slot)
            .await?
            .body
            .execution_payload()
            .parent_hash();

        let mut payloads: Vec<ExecutionPayload<S>> = Vec::new();
        for result in join_all(payloads_fut).await {
            if result.is_err() {
                continue;
            }
            let payload = result.unwrap().body.execution_payload().clone();
            if payload.block_hash() != &prev_parent_hash {
                warn!(
                    target: "helios::consensus",
                    error = %ConsensusError::InvalidHeaderHash(
                        prev_parent_hash,
                        *payload.parent_hash(),
                    ),
                    "error while backfilling blocks"
                );
                break;
            }
            prev_parent_hash = *payload.parent_hash();
            payloads.push(payload);
        }
        Ok(payloads)
    }

    pub async fn sync(&mut self, checkpoint: B256) -> Result<()> {
        self.store = LightClientStore::default();
        self.last_checkpoint = None;

        self.bootstrap(checkpoint).await?;

        let current_period = calc_sync_period::<S>(self.store.finalized_header.beacon().slot);
        let updates = self
            .rpc
            .get_updates(current_period, MAX_REQUEST_LIGHT_CLIENT_UPDATES)
            .await?;

        for update in updates {
            self.verify_update(&update)?;
            self.apply_update(&update);
        }

        let finality_update = self.rpc.get_finality_update().await?;
        self.verify_finality_update(&finality_update)?;
        self.apply_finality_update(&finality_update);

        info!(
            target: "helios::consensus",
            "consensus client in sync with checkpoint: 0x{}",
            hex::encode(checkpoint)
        );

        Ok(())
    }

    pub async fn advance(&mut self) -> Result<()> {
        let finality_update = self.rpc.get_finality_update().await?;
        self.verify_finality_update(&finality_update)?;
        self.apply_finality_update(&finality_update);

        if self.store.next_sync_committee.is_none() {
            debug!(target: "helios::consensus", "checking for sync committee update");
            let current_period = calc_sync_period::<S>(self.store.finalized_header.beacon().slot);
            let mut updates = self.rpc.get_updates(current_period, 1).await?;

            if updates.len() == 1 {
                let update = updates.get_mut(0).unwrap();
                let res = self.verify_update(update);

                if res.is_ok() {
                    info!(target: "helios::consensus", "updating sync committee");
                    self.apply_update(update);
                }
            }
        }

        Ok(())
    }

    pub async fn send_blocks(&self) -> Result<()> {
        let slot = self.store.optimistic_header.beacon().slot;
        let payload = self.get_execution_payload(&Some(slot)).await?;
        let finalized_slot = self.store.finalized_header.beacon().slot;
        let finalized_payload = self.get_execution_payload(&Some(finalized_slot)).await?;

        self.block_send.send(payload_to_block(payload)).await?;
        self.finalized_block_send
            .send(Some(payload_to_block(finalized_payload)))?;
        self.checkpoint_send.send(self.last_checkpoint)?;

        Ok(())
    }

    /// Gets the duration until the next update
    /// Updates are scheduled for 4 seconds into each slot
    pub fn duration_until_next_update(&self) -> Duration {
        let current_slot = self.expected_current_slot();
        let next_slot = current_slot + 1;
        let next_slot_timestamp = self.slot_timestamp(next_slot);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| panic!("unreachable"))
            .as_secs();

        let time_to_next_slot = next_slot_timestamp - now;
        let next_update = time_to_next_slot + 4;

        Duration::try_seconds(next_update as i64).unwrap()
    }

    pub async fn bootstrap(&mut self, checkpoint: B256) -> Result<()> {
        let bootstrap = self
            .rpc
            .get_bootstrap(checkpoint)
            .await
            .map_err(|err| eyre!("could not fetch bootstrap: {}", err))?;

        let is_valid = self.is_valid_checkpoint(bootstrap.header().beacon().slot);

        if !is_valid {
            if self.config.strict_checkpoint_age {
                return Err(ConsensusError::CheckpointTooOld.into());
            } else {
                warn!(target: "helios::consensus", "checkpoint too old, consider using a more recent block");
            }
        }

        verify_bootstrap(&bootstrap, checkpoint, &self.config.forks)?;
        apply_bootstrap(&mut self.store, &bootstrap);

        Ok(())
    }

    pub fn verify_update(&self, update: &Update<S>) -> Result<()> {
        verify_update::<S>(
            update,
            self.expected_current_slot(),
            &self.store,
            self.config.chain.genesis_root,
            &self.config.forks,
        )
    }

    fn verify_finality_update(&self, update: &FinalityUpdate<S>) -> Result<()> {
        verify_finality_update::<S>(
            update,
            self.expected_current_slot(),
            &self.store,
            self.config.chain.genesis_root,
            &self.config.forks,
        )
    }

    pub fn apply_update(&mut self, update: &Update<S>) {
        let new_checkpoint = apply_update::<S>(&mut self.store, update);
        if new_checkpoint.is_some() {
            self.last_checkpoint = new_checkpoint;
        }
    }

    fn apply_finality_update(&mut self, update: &FinalityUpdate<S>) {
        let prev_finalized_slot = self.store.finalized_header.beacon().slot;
        let prev_optimistic_slot = self.store.optimistic_header.beacon().slot;
        let new_checkpoint = apply_finality_update::<S>(&mut self.store, update);
        let new_finalized_slot = self.store.finalized_header.beacon().slot;
        let new_optimistic_slot = self.store.optimistic_header.beacon().slot;
        if new_checkpoint.is_some() {
            self.last_checkpoint = new_checkpoint;
        }
        if new_finalized_slot != prev_finalized_slot {
            self.log_finality_update(update);
        }
        if new_optimistic_slot != prev_optimistic_slot {
            self.log_optimistic_update(update)
        }
    }

    fn log_finality_update(&self, update: &FinalityUpdate<S>) {
        let size = S::sync_commitee_size() as f32;
        let participation =
            get_bits::<S>(&update.sync_aggregate().sync_committee_bits) as f32 / size * 100f32;
        let decimals = if participation == 100.0 { 1 } else { 2 };
        let age = self.age(self.store.finalized_header.beacon().slot);

        info!(
            target: "helios::consensus",
            "finalized slot             slot={}  confidence={:.decimals$}%  age={:02}:{:02}:{:02}:{:02}",
            self.store.finalized_header.beacon().slot,
            participation,
            age.num_days(),
            age.num_hours() % 24,
            age.num_minutes() % 60,
            age.num_seconds() % 60,
        );
    }

    fn log_optimistic_update(&self, update: &FinalityUpdate<S>) {
        let size = S::sync_commitee_size() as f32;
        let participation =
            get_bits::<S>(&update.sync_aggregate().sync_committee_bits) as f32 / size * 100f32;
        let decimals = if participation == 100.0 { 1 } else { 2 };
        let age = self.age(self.store.optimistic_header.beacon().slot);

        info!(
            target: "helios::consensus",
            "updated head               slot={}  confidence={:.decimals$}%  age={:02}:{:02}:{:02}:{:02}",
            self.store.optimistic_header.beacon().slot,
            participation,
            age.num_days(),
            age.num_hours() % 24,
            age.num_minutes() % 60,
            age.num_seconds() % 60,
        );
    }

    fn age(&self, slot: u64) -> Duration {
        let expected_time = self.slot_timestamp(slot);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| panic!("unreachable"));

        let delay = now - std::time::Duration::from_secs(expected_time);
        chrono::Duration::from_std(delay).unwrap()
    }

    pub fn expected_current_slot(&self) -> u64 {
        let now = SystemTime::now();

        expected_current_slot(now, self.config.chain.genesis_time)
    }

    fn slot_timestamp(&self, slot: u64) -> u64 {
        slot * 12 + self.config.chain.genesis_time
    }

    // Determines blockhash_slot age and returns true if it is less than 14 days old
    fn is_valid_checkpoint(&self, blockhash_slot: u64) -> bool {
        let current_slot = self.expected_current_slot();
        let current_slot_timestamp = self.slot_timestamp(current_slot);
        let blockhash_slot_timestamp = self.slot_timestamp(blockhash_slot);

        let slot_age = current_slot_timestamp
            .checked_sub(blockhash_slot_timestamp)
            .unwrap_or_default();

        slot_age < self.config.max_checkpoint_age
    }
}

fn payload_to_block<S: ConsensusSpec>(value: ExecutionPayload<S>) -> Block<Transaction> {
    let empty_nonce = fixed_bytes!("0000000000000000");
    let empty_uncle_hash =
        b256!("1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347");

    let txs = value
        .transactions()
        .iter()
        .enumerate()
        .map(|(i, tx_bytes)| {
            let tx_bytes = tx_bytes.inner.to_vec();
            let mut tx_bytes_slice = tx_bytes.as_slice();
            let tx_envelope = TxEnvelope::decode(&mut tx_bytes_slice).unwrap();

            let mut tx = Transaction {
                hash: *tx_envelope.tx_hash(),
                nonce: tx_envelope.nonce(),
                block_hash: Some(*value.block_hash()),
                block_number: Some(*value.block_number()),
                transaction_index: Some(i as u64),
                to: tx_envelope.to().to().cloned(),
                value: tx_envelope.value(),
                gas_price: tx_envelope.gas_price(),
                gas: tx_envelope.gas_limit(),
                input: tx_envelope.input().to_vec().into(),
                chain_id: tx_envelope.chain_id(),
                transaction_type: Some(tx_envelope.tx_type().into()),
                ..Default::default()
            };

            match tx_envelope {
                TxEnvelope::Legacy(inner) => {
                    tx.from = inner.recover_signer().unwrap();
                    tx.signature = Some(Signature {
                        r: inner.signature().r(),
                        s: inner.signature().s(),
                        v: U256::from(inner.signature().v().to_u64()),
                        y_parity: None,
                    });
                }
                TxEnvelope::Eip2930(inner) => {
                    tx.from = inner.recover_signer().unwrap();
                    tx.signature = Some(Signature {
                        r: inner.signature().r(),
                        s: inner.signature().s(),
                        v: U256::from(inner.signature().v().to_u64()),
                        y_parity: Some(Parity(inner.signature().v().to_u64() == 1)),
                    });
                    tx.access_list = Some(inner.tx().access_list.clone());
                }
                TxEnvelope::Eip1559(inner) => {
                    tx.from = inner.recover_signer().unwrap();
                    tx.signature = Some(Signature {
                        r: inner.signature().r(),
                        s: inner.signature().s(),
                        v: U256::from(inner.signature().v().to_u64()),
                        y_parity: Some(Parity(inner.signature().v().to_u64() == 1)),
                    });

                    let tx_inner = inner.tx();
                    tx.access_list = Some(tx_inner.access_list.clone());
                    tx.max_fee_per_gas = Some(tx_inner.max_fee_per_gas);
                    tx.max_priority_fee_per_gas = Some(tx_inner.max_priority_fee_per_gas);

                    tx.gas_price = Some(gas_price(
                        tx_inner.max_fee_per_gas,
                        tx_inner.max_priority_fee_per_gas,
                        value.base_fee_per_gas().to(),
                    ));
                }
                TxEnvelope::Eip4844(inner) => {
                    tx.from = inner.recover_signer().unwrap();
                    tx.signature = Some(Signature {
                        r: inner.signature().r(),
                        s: inner.signature().s(),
                        v: U256::from(inner.signature().v().to_u64()),
                        y_parity: Some(Parity(inner.signature().v().to_u64() == 1)),
                    });

                    let tx_inner = inner.tx().tx();
                    tx.access_list = Some(tx_inner.access_list.clone());
                    tx.max_fee_per_gas = Some(tx_inner.max_fee_per_gas);
                    tx.max_priority_fee_per_gas = Some(tx_inner.max_priority_fee_per_gas);
                    tx.max_fee_per_blob_gas = Some(tx_inner.max_fee_per_blob_gas);
                    tx.gas_price = Some(tx_inner.max_fee_per_gas);
                    tx.blob_versioned_hashes = Some(tx_inner.blob_versioned_hashes.clone());

                    tx.gas_price = Some(gas_price(
                        tx_inner.max_fee_per_gas,
                        tx_inner.max_priority_fee_per_gas,
                        value.base_fee_per_gas().to(),
                    ));
                }
                _ => todo!(),
            }

            tx
        })
        .collect::<Vec<Transaction>>();

    let raw_txs = value.transactions().iter().map(|tx| tx.inner.to_vec());
    let txs_root = ordered_trie_root(raw_txs);

    let withdrawals = value.withdrawals().unwrap().iter().map(encode);
    let withdrawals_root = ordered_trie_root(withdrawals);

    Block {
        number: U64::from(*value.block_number()),
        base_fee_per_gas: *value.base_fee_per_gas(),
        difficulty: U256::ZERO,
        extra_data: value.extra_data().inner.to_vec().into(),
        gas_limit: U64::from(*value.gas_limit()),
        gas_used: U64::from(*value.gas_used()),
        hash: *value.block_hash(),
        logs_bloom: value.logs_bloom().inner.to_vec().into(),
        miner: *value.fee_recipient(),
        parent_hash: *value.parent_hash(),
        receipts_root: *value.receipts_root(),
        state_root: *value.state_root(),
        timestamp: U64::from(*value.timestamp()),
        total_difficulty: U64::ZERO,
        transactions: Transactions::Full(txs),
        mix_hash: *value.prev_randao(),
        nonce: empty_nonce,
        sha3_uncles: empty_uncle_hash,
        size: U64::ZERO,
        transactions_root: B256::from_slice(txs_root.as_bytes()),
        uncles: vec![],
        blob_gas_used: value.blob_gas_used().map(|v| U64::from(*v)).ok(),
        excess_blob_gas: value.excess_blob_gas().map(|v| U64::from(*v)).ok(),
        withdrawals_root: B256::from_slice(withdrawals_root.as_bytes()),
        parent_beacon_block_root: Some(*value.parent_hash()),
    }
}

fn gas_price(max_fee: u128, max_prio_fee: u128, base_fee: u128) -> u128 {
    u128::min(max_fee, max_prio_fee + base_fee)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use alloy::primitives::b256;
    use tokio::sync::{mpsc::channel, watch};

    use helios_consensus_core::errors::ConsensusError;
    use helios_consensus_core::types::bls::{PublicKey, Signature};
    use helios_consensus_core::types::LightClientHeader;

    use crate::{
        config::{networks, Config},
        consensus::calc_sync_period,
        consensus::Inner,
        constants::MAX_REQUEST_LIGHT_CLIENT_UPDATES,
        rpc::{mock_rpc::MockRpc, ConsensusRpc},
    };

    async fn get_client(strict_checkpoint_age: bool, sync: bool) -> Inner<MockRpc> {
        let base_config = networks::mainnet();
        let config = Config {
            consensus_rpc: String::new(),
            execution_rpc: String::new(),
            chain: base_config.chain,
            forks: base_config.forks,
            strict_checkpoint_age,
            ..Default::default()
        };

        let checkpoint = b256!("5afc212a7924789b2bc86acad3ab3a6ffb1f6e97253ea50bee7f4f51422c9275");

        let (block_send, _) = channel(256);
        let (finalized_block_send, _) = watch::channel(None);
        let (channel_send, _) = watch::channel(None);

        let mut client = Inner::new(
            "testdata/",
            block_send,
            finalized_block_send,
            channel_send,
            Arc::new(config),
        );

        if sync {
            client.sync(checkpoint).await.unwrap()
        } else {
            client.bootstrap(checkpoint).await.unwrap();
        }

        client
    }

    #[tokio::test]
    async fn test_verify_update() {
        let client = get_client(false, false).await;
        let period = calc_sync_period(client.store.finalized_header.beacon.slot.into());
        let updates = client
            .rpc
            .get_updates(period, MAX_REQUEST_LIGHT_CLIENT_UPDATES)
            .await
            .unwrap();

        let update = updates[0].clone();
        client.verify_update(&update).unwrap();
    }

    #[tokio::test]
    async fn test_verify_update_invalid_committee() {
        let client = get_client(false, false).await;
        let period = calc_sync_period(client.store.finalized_header.beacon.slot.into());
        let updates = client
            .rpc
            .get_updates(period, MAX_REQUEST_LIGHT_CLIENT_UPDATES)
            .await
            .unwrap();

        let mut update = updates[0].clone();
        update.next_sync_committee.pubkeys[0] = PublicKey::default();

        let err = client.verify_update(&update).err().unwrap();
        assert_eq!(
            err.to_string(),
            ConsensusError::InvalidNextSyncCommitteeProof.to_string()
        );
    }

    #[tokio::test]
    async fn test_verify_update_invalid_finality() {
        let client = get_client(false, false).await;
        let period = calc_sync_period(client.store.finalized_header.beacon.slot.into());
        let updates = client
            .rpc
            .get_updates(period, MAX_REQUEST_LIGHT_CLIENT_UPDATES)
            .await
            .unwrap();

        let mut update = updates[0].clone();
        update.finalized_header = LightClientHeader::default();

        let err = client.verify_update(&update).err().unwrap();
        assert_eq!(
            err.to_string(),
            ConsensusError::InvalidFinalityProof.to_string()
        );
    }

    #[tokio::test]
    async fn test_verify_update_invalid_sig() {
        let client = get_client(false, false).await;
        let period = calc_sync_period(client.store.finalized_header.beacon.slot.into());
        let updates = client
            .rpc
            .get_updates(period, MAX_REQUEST_LIGHT_CLIENT_UPDATES)
            .await
            .unwrap();

        let mut update = updates[0].clone();
        update.sync_aggregate.sync_committee_signature = Signature::default();

        let err = client.verify_update(&update).err().unwrap();
        assert_eq!(
            err.to_string(),
            ConsensusError::InvalidSignature.to_string()
        );
    }

    #[tokio::test]
    async fn test_verify_finality() {
        let client = get_client(false, true).await;

        let update = client.rpc.get_finality_update().await.unwrap();

        client.verify_finality_update(&update).unwrap();
    }

    #[tokio::test]
    async fn test_verify_finality_invalid_finality() {
        let client = get_client(false, true).await;

        let mut update = client.rpc.get_finality_update().await.unwrap();
        update.finalized_header = LightClientHeader::default();

        let err = client.verify_finality_update(&update).err().unwrap();
        assert_eq!(
            err.to_string(),
            ConsensusError::InvalidFinalityProof.to_string()
        );
    }

    #[tokio::test]
    async fn test_verify_finality_invalid_sig() {
        let client = get_client(false, true).await;

        let mut update = client.rpc.get_finality_update().await.unwrap();
        update.sync_aggregate.sync_committee_signature = Signature::default();

        let err = client.verify_finality_update(&update).err().unwrap();
        assert_eq!(
            err.to_string(),
            ConsensusError::InvalidSignature.to_string()
        );
    }

    #[tokio::test]
    #[should_panic]
    async fn test_verify_checkpoint_age_invalid() {
        get_client(true, false).await;
    }
}
