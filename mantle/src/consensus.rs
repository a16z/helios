use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use url::Url;

use alloy::consensus::proofs::{calculate_transaction_root, calculate_withdrawals_root};
use alloy::consensus::transaction::SignerRecoverable;
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
use tracing::{debug, error, warn};

use helios_consensus_core::consensus_spec::MainnetConsensusSpec;
use helios_core::consensus::Consensus;
use helios_core::execution::proof::{verify_account_proof, verify_mpt_proof};
use helios_core::time::{interval, SystemTime, UNIX_EPOCH};
use helios_ethereum::consensus::ConsensusClient as EthConsensusClient;

use helios_ethereum::database::ConfigDB;
use helios_ethereum::rpc::http_rpc::HttpRpc;

use crate::{config::Config, types::ExecutionPayload, SequencerCommitment};

// Storage slot containing the unsafe signer address in all OP Stack system config contracts
// (including Mantle's SystemConfig)
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
            server_url: config.consensus_rpc.clone(),
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
                    error!(target: "helios::mantle", "failed to advance: {}", e);
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
        // Mantle consensus doesn't have a sync process, so immediately return Ok
        Ok(())
    }
}

#[allow(dead_code)]
struct Inner {
    server_url: Url,
    unsafe_signer: Arc<Mutex<Address>>,
    chain_id: u64,
    latest_block: Option<u64>,
    block_send: Sender<Block<Transaction>>,
    finalized_block_send: watch::Sender<Option<Block<Transaction>>>,
}

impl Inner {
    pub async fn advance(&mut self) -> Result<()> {
        let url = self
            .server_url
            .join("latest")
            .map_err(|e| eyre!("Failed to construct latest URL: {}", e))?;
        let commitment = reqwest::get(url)
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
                    .unwrap_or_default();

                let timestamp = Duration::from_secs(payload.timestamp);
                let age = now.saturating_sub(timestamp);
                let number = payload.block_number;

                if let Ok(block) = payload_to_block(payload) {
                    self.latest_block = Some(block.header.number);
                    _ = self.block_send.send(block).await;

                    tracing::debug!(
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

            let consensus_rpc = eth_config
                .consensus_rpc
                .as_ref()
                .ok_or_else(|| eyre!("missing consensus rpc"))?
                .clone();

            let mut eth_consensus =
                EthConsensusClient::<MainnetConsensusSpec, HttpRpc, ConfigDB>::new(
                    &consensus_rpc,
                    Arc::new(eth_config.into()),
                )?;

            let block = eth_consensus
                .block_recv()
                .unwrap()
                .recv()
                .await
                .ok_or_eyre("failed to receive block")?;

            // Query proof from consensus server
            let url = config
                .consensus_rpc
                .join(&format!("unsafe_signer_proof/{}", block.header.hash))
                .map_err(|e| eyre!("Failed to construct proof URL: {}", e))?;
            let proof = reqwest::get(url)
                .await?
                .json::<EIP1186AccountProofResponse>()
                .await?;

            // Verify unsafe signer
            // with account proof
            if verify_account_proof(&proof, block.header.state_root).is_err() {
                warn!(target: "helios::mantle", "account proof invalid");
                return Err(eyre!("account proof invalid"));
            }

            // with storage proof
            let storage_proof = proof.storage_proof[0].clone();
            let key = storage_proof.key.as_b256();
            if key != B256::from_str(UNSAFE_SIGNER_SLOT)? {
                warn!(target: "helios::mantle", "account proof invalid");
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
                warn!(target: "helios::mantle", "storage proof invalid");
                return Err(eyre!("storage proof invalid"));
            }

            // Replace unsafe signer if different
            let verified_signer =
                Address::from_slice(&storage_proof.value.to_be_bytes::<32>()[12..32]);
            {
                let mut curr_signer = signer.lock().map_err(|_| eyre!("failed to lock signer"))?;
                if verified_signer != *curr_signer {
                    debug!(target: "helios::mantle", "unsafe signer updated: {}", verified_signer);
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

pub(crate) fn payload_to_block(value: ExecutionPayload) -> Result<Block<Transaction>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers;
    use alloy::primitives::{address, U256};
    use ssz_types::VariableList;

    fn make_test_payload() -> ExecutionPayload {
        test_helpers::test_payload()
    }

    fn make_payload_with_block_number(block_number: u64) -> ExecutionPayload {
        let mut p = make_test_payload();
        p.block_number = block_number;
        p
    }

    // ── payload_to_block unit tests ──

    #[test]
    fn test_payload_to_block_header_fields() {
        let payload = make_test_payload();
        let block = payload_to_block(payload.clone()).unwrap();

        assert_eq!(block.header.number, payload.block_number);
        assert_eq!(block.header.inner.parent_hash, payload.parent_hash);
        assert_eq!(
            block.header.inner.beneficiary,
            Address::from(*payload.fee_recipient)
        );
        assert_eq!(block.header.inner.state_root, payload.state_root);
        assert_eq!(block.header.inner.receipts_root, payload.receipts_root);
        assert_eq!(block.header.inner.gas_limit, payload.gas_limit);
        assert_eq!(block.header.inner.gas_used, payload.gas_used);
        assert_eq!(block.header.inner.timestamp, payload.timestamp);
        assert_eq!(block.header.inner.mix_hash, payload.prev_randao);
        assert_eq!(block.header.hash, payload.block_hash);
    }

    #[test]
    fn test_payload_to_block_constants() {
        let payload = make_test_payload();
        let block = payload_to_block(payload).unwrap();

        let empty_uncle_hash =
            b256!("1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347");
        assert_eq!(block.header.inner.ommers_hash, empty_uncle_hash);
        assert_eq!(block.header.inner.nonce, fixed_bytes!("0000000000000000"));
        assert_eq!(block.header.inner.difficulty, U256::ZERO);
        assert_eq!(block.header.total_difficulty, Some(U256::ZERO));
        assert_eq!(block.header.size, Some(U256::ZERO));
    }

    #[test]
    fn test_payload_to_block_empty_txs() {
        let payload = make_test_payload();
        let block = payload_to_block(payload).unwrap();

        match &block.transactions {
            BlockTransactions::Full(txs) => assert_eq!(txs.len(), 0),
            _ => panic!("expected Full transactions"),
        }
    }

    #[test]
    fn test_payload_to_block_empty_withdrawals() {
        let payload = make_test_payload();
        let block = payload_to_block(payload).unwrap();

        let withdrawals = block.withdrawals.unwrap();
        assert_eq!(withdrawals.len(), 0);
    }

    #[test]
    fn test_payload_to_block_with_withdrawals() {
        let mut payload = make_test_payload();
        let w = test_helpers::test_withdrawal(
            1,
            2,
            address!("cccccccccccccccccccccccccccccccccccccccc"),
            500,
        );
        payload.withdrawals = VariableList::from(vec![w]);

        let block = payload_to_block(payload).unwrap();
        let withdrawals = block.withdrawals.unwrap();
        assert_eq!(withdrawals.len(), 1);
        assert_eq!(withdrawals[0].index, 1);
        assert_eq!(withdrawals[0].validator_index, 2);
        assert_eq!(
            withdrawals[0].address,
            address!("cccccccccccccccccccccccccccccccccccccccc")
        );
        assert_eq!(withdrawals[0].amount, 500);
    }

    #[test]
    fn test_payload_to_block_base_fee() {
        let mut payload = make_test_payload();
        payload.base_fee_per_gas = U256::from(42_000_000_000u64);
        let block = payload_to_block(payload).unwrap();
        assert_eq!(block.header.inner.base_fee_per_gas, Some(42_000_000_000u64));
    }

    #[test]
    fn test_payload_to_block_blob_gas_fields() {
        let mut payload = make_test_payload();
        payload.blob_gas_used = 131072;
        payload.excess_blob_gas = 65536;
        let block = payload_to_block(payload).unwrap();
        assert_eq!(block.header.inner.blob_gas_used, Some(131072));
        assert_eq!(block.header.inner.excess_blob_gas, Some(65536));
    }

    #[test]
    fn test_payload_to_block_extra_data() {
        let mut payload = make_test_payload();
        payload.extra_data = VariableList::from(vec![0xAA, 0xBB, 0xCC]);
        let block = payload_to_block(payload).unwrap();
        assert_eq!(block.header.inner.extra_data.as_ref(), &[0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn test_payload_to_block_no_parent_beacon_root() {
        let payload = make_test_payload();
        let block = payload_to_block(payload).unwrap();
        assert!(block.header.inner.parent_beacon_block_root.is_none());
    }

    #[test]
    fn test_payload_to_block_withdrawals_root_computed() {
        let payload = make_test_payload();
        let block = payload_to_block(payload).unwrap();
        // For empty withdrawals, the root should still be computed
        assert!(block.header.inner.withdrawals_root.is_some());
    }

    // ── Inner::advance() mock tests ──

    #[tokio::test]
    async fn test_advance_valid_commitment() {
        let signer = test_helpers::test_signing_key();
        let signer_address = signer.address();
        let chain_id = 5000u64;
        let payload = make_payload_with_block_number(42);

        let compressed = test_helpers::create_signed_commitment(&payload, &signer, chain_id);
        let commitment = crate::SequencerCommitment::new(&compressed).unwrap();

        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/latest"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_json(&commitment),
            )
            .mount(&mock_server)
            .await;

        let server_url = url::Url::parse(&mock_server.uri()).unwrap();
        let (block_send, mut block_recv) = tokio::sync::mpsc::channel(256);
        let (finalized_block_send, _finalized_block_recv) = tokio::sync::watch::channel(None);

        let mut inner = Inner {
            server_url,
            unsafe_signer: std::sync::Arc::new(std::sync::Mutex::new(signer_address)),
            chain_id,
            latest_block: None,
            block_send,
            finalized_block_send,
        };

        inner.advance().await.unwrap();

        // Should have received a block
        let block = block_recv.try_recv().unwrap();
        assert_eq!(block.header.number, 42);
        assert_eq!(inner.latest_block, Some(42));
    }

    #[tokio::test]
    async fn test_advance_duplicate_block_ignored() {
        let signer = test_helpers::test_signing_key();
        let signer_address = signer.address();
        let chain_id = 5000u64;
        let payload = make_payload_with_block_number(42);

        let compressed = test_helpers::create_signed_commitment(&payload, &signer, chain_id);
        let commitment = crate::SequencerCommitment::new(&compressed).unwrap();

        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/latest"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_json(&commitment),
            )
            .expect(2)
            .mount(&mock_server)
            .await;

        let server_url = url::Url::parse(&mock_server.uri()).unwrap();
        let (block_send, mut block_recv) = tokio::sync::mpsc::channel(256);
        let (finalized_block_send, _) = tokio::sync::watch::channel(None);

        let mut inner = Inner {
            server_url,
            unsafe_signer: std::sync::Arc::new(std::sync::Mutex::new(signer_address)),
            chain_id,
            latest_block: None,
            block_send,
            finalized_block_send,
        };

        // First advance processes the block
        inner.advance().await.unwrap();
        let _block = block_recv.try_recv().unwrap();
        assert_eq!(inner.latest_block, Some(42));

        // Second advance with same block number should be a no-op
        inner.advance().await.unwrap();
        assert!(block_recv.try_recv().is_err());
        assert_eq!(inner.latest_block, Some(42));
    }

    #[tokio::test]
    async fn test_advance_newer_block_accepted() {
        let signer = test_helpers::test_signing_key();
        let signer_address = signer.address();
        let chain_id = 5000u64;

        let payload1 = make_payload_with_block_number(42);
        let compressed1 = test_helpers::create_signed_commitment(&payload1, &signer, chain_id);
        let commitment1 = crate::SequencerCommitment::new(&compressed1).unwrap();

        let payload2 = make_payload_with_block_number(43);
        let compressed2 = test_helpers::create_signed_commitment(&payload2, &signer, chain_id);
        let commitment2 = crate::SequencerCommitment::new(&compressed2).unwrap();

        let mock_server = wiremock::MockServer::start().await;

        // First request returns block 42
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/latest"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_json(&commitment1),
            )
            .up_to_n_times(1)
            .mount(&mock_server)
            .await;

        let server_url = url::Url::parse(&mock_server.uri()).unwrap();
        let (block_send, mut block_recv) = tokio::sync::mpsc::channel(256);
        let (finalized_block_send, _) = tokio::sync::watch::channel(None);

        let mut inner = Inner {
            server_url: server_url.clone(),
            unsafe_signer: std::sync::Arc::new(std::sync::Mutex::new(signer_address)),
            chain_id,
            latest_block: None,
            block_send,
            finalized_block_send,
        };

        inner.advance().await.unwrap();
        let block1 = block_recv.try_recv().unwrap();
        assert_eq!(block1.header.number, 42);

        // Reset mock to return block 43
        mock_server.reset().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/latest"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_json(&commitment2),
            )
            .mount(&mock_server)
            .await;

        inner.advance().await.unwrap();
        let block2 = block_recv.try_recv().unwrap();
        assert_eq!(block2.header.number, 43);
        assert_eq!(inner.latest_block, Some(43));
    }

    #[tokio::test]
    async fn test_advance_invalid_signature_ignored() {
        let signer = test_helpers::test_signing_key();
        let chain_id = 5000u64;
        let payload = make_payload_with_block_number(42);

        let compressed = test_helpers::create_signed_commitment(&payload, &signer, chain_id);
        let commitment = crate::SequencerCommitment::new(&compressed).unwrap();

        let mock_server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/latest"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_json(&commitment),
            )
            .mount(&mock_server)
            .await;

        let server_url = url::Url::parse(&mock_server.uri()).unwrap();
        let (block_send, mut block_recv) = tokio::sync::mpsc::channel(256);
        let (finalized_block_send, _) = tokio::sync::watch::channel(None);

        // Use a WRONG signer address
        let wrong_address = address!("0000000000000000000000000000000000000001");
        let mut inner = Inner {
            server_url,
            unsafe_signer: std::sync::Arc::new(std::sync::Mutex::new(wrong_address)),
            chain_id,
            latest_block: None,
            block_send,
            finalized_block_send,
        };

        // advance() should succeed (no error) but not send any block
        inner.advance().await.unwrap();
        assert!(block_recv.try_recv().is_err());
        assert!(inner.latest_block.is_none());
    }

    // ── ConsensusClient trait tests ──

    #[test]
    fn test_consensus_client_chain_id() {
        let client = ConsensusClient {
            block_recv: None,
            finalized_block_recv: None,
            chain_id: 5000,
        };
        assert_eq!(Consensus::<Block<Transaction>>::chain_id(&client), 5000);
    }

    #[test]
    fn test_consensus_client_shutdown() {
        let client = ConsensusClient {
            block_recv: None,
            finalized_block_recv: None,
            chain_id: 5000,
        };
        assert!(Consensus::<Block<Transaction>>::shutdown(&client).is_ok());
    }

    #[test]
    fn test_consensus_client_checkpoint_recv_none() {
        let client = ConsensusClient {
            block_recv: None,
            finalized_block_recv: None,
            chain_id: 5000,
        };
        assert!(Consensus::<Block<Transaction>>::checkpoint_recv(&client).is_none());
    }

    #[test]
    fn test_consensus_client_expected_highest_block() {
        let client = ConsensusClient {
            block_recv: None,
            finalized_block_recv: None,
            chain_id: 5000,
        };
        assert_eq!(
            Consensus::<Block<Transaction>>::expected_highest_block(&client),
            u64::MAX
        );
    }

    #[test]
    fn test_consensus_client_block_recv_take() {
        let (_send, recv) = tokio::sync::mpsc::channel(1);
        let mut client = ConsensusClient {
            block_recv: Some(recv),
            finalized_block_recv: None,
            chain_id: 5000,
        };
        // First take returns Some
        assert!(Consensus::<Block<Transaction>>::block_recv(&mut client).is_some());
        // Second take returns None (already taken)
        assert!(Consensus::<Block<Transaction>>::block_recv(&mut client).is_none());
    }

    #[test]
    fn test_consensus_client_finalized_block_recv_take() {
        let (_send, recv) = tokio::sync::watch::channel(None);
        let mut client = ConsensusClient {
            block_recv: None,
            finalized_block_recv: Some(recv),
            chain_id: 5000,
        };
        assert!(Consensus::<Block<Transaction>>::finalized_block_recv(&mut client).is_some());
        assert!(Consensus::<Block<Transaction>>::finalized_block_recv(&mut client).is_none());
    }

    #[tokio::test]
    async fn test_consensus_client_wait_synced() {
        let client = ConsensusClient {
            block_recv: None,
            finalized_block_recv: None,
            chain_id: 5000,
        };
        // Mantle wait_synced returns immediately
        assert!(client.wait_synced().await.is_ok());
    }
}
