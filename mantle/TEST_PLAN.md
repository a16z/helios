# Mantle Helios - Test Plan

## Overview

This document describes the unit test and mock test strategy for the `helios-mantle` crate.
All tests are scoped within `helios/mantle/` and do not pollute other crates or directories.

---

## 1. Unit Tests (UT)

Unit tests validate individual functions and data structures in isolation, with no external
dependencies (no network, no disk I/O, no spawned tasks).

### 1.1 `config.rs` — Network Configuration

| Test | Target | Validates |
|------|--------|-----------|
| `test_network_from_str_mainnet` | `Network::from_str("mantle")` | Parses to `MantleMainnet` |
| `test_network_from_str_sepolia` | `Network::from_str("mantle-sepolia")` | Parses to `MantleSepolia` |
| `test_network_from_str_invalid` | `Network::from_str("unknown")` | Returns `Err` |
| `test_network_display_mainnet` | `Network::MantleMainnet.to_string()` | Displays `"mantle"` |
| `test_network_display_sepolia` | `Network::MantleSepolia.to_string()` | Displays `"mantle-sepolia"` |
| `test_network_config_mainnet` | `NetworkConfig::from(MantleMainnet)` | chain_id=5000, correct signer, correct system_config_contract |
| `test_network_config_sepolia` | `NetworkConfig::from(MantleSepolia)` | chain_id=5003, correct signer, correct system_config_contract |
| `test_mainnet_fork_schedule` | `MantleForkSchedule::mainnet()` | Bedrock/Regolith=0, Canyon–Isthmus=SkadiTime, Jovian=LimbTime |
| `test_sepolia_fork_schedule` | `MantleForkSchedule::sepolia()` | Bedrock/Regolith=0, Canyon–Isthmus=SkadiTime, Jovian=LimbTime |
| `test_fork_schedule_defaults` | Non-set fields | Remaining fields use `u64::MAX` (not activated) |

### 1.2 `evm.rs` — Fork-to-SpecId Mapping

| Test | Target | Validates |
|------|--------|-----------|
| `test_spec_id_bedrock` | `get_spec_id_for_block_timestamp(0, ...)` | Returns `OpSpecId::BEDROCK` |
| `test_spec_id_regolith` | timestamp in Regolith range | Returns `OpSpecId::REGOLITH` |
| `test_spec_id_canyon` | timestamp at Canyon | Returns `OpSpecId::CANYON` |
| `test_spec_id_ecotone` | timestamp at Ecotone | Returns `OpSpecId::ECOTONE` |
| `test_spec_id_fjord` | timestamp at Fjord | Returns `OpSpecId::FJORD` |
| `test_spec_id_granite` | timestamp at Granite | Returns `OpSpecId::GRANITE` |
| `test_spec_id_holocene` | timestamp at Holocene | Returns `OpSpecId::HOLOCENE` |
| `test_spec_id_isthmus` | timestamp at Isthmus | Returns `OpSpecId::ISTHMUS` |
| `test_spec_id_boundary_cases` | Exact boundary timestamps | Correct fork selected at boundary |
| `test_spec_id_mantle_mainnet_schedule` | Mainnet fork schedule | SkadiTime→ISTHMUS, pre-Skadi→BEDROCK |
| `test_spec_id_mantle_sepolia_schedule` | Sepolia fork schedule | SkadiTime→ISTHMUS, pre-Skadi→BEDROCK |

### 1.3 `types.rs` — SSZ Types & Conversions

| Test | Target | Validates |
|------|--------|-----------|
| `test_withdrawal_into_alloy` | `Withdrawal::from → alloy::Withdrawal` | All fields transferred correctly |
| `test_execution_payload_ssz_roundtrip` | Encode→Decode | Payload survives SSZ roundtrip |
| `test_execution_payload_fields` | Direct construction | All fields accessible and correct |

### 1.4 `lib.rs` — SequencerCommitment

| Test | Target | Validates |
|------|--------|-----------|
| `test_signature_msg_deterministic` | `signature_msg(data, chain_id)` | Same inputs → same hash |
| `test_signature_msg_different_chain_ids` | Different chain_ids | Different hashes for different chain_ids |
| `test_signature_msg_different_data` | Different data | Different hashes for different data |
| `test_sequencer_commitment_new_valid` | `SequencerCommitment::new()` | Parses snap-compressed commitment |
| `test_sequencer_commitment_new_invalid` | Bad input | Returns error |
| `test_sequencer_commitment_verify_valid` | `verify()` with correct signer | Returns `Ok(())` |
| `test_sequencer_commitment_verify_wrong_signer` | `verify()` with wrong signer | Returns `Err` |
| `test_sequencer_commitment_verify_wrong_chain_id` | `verify()` with wrong chain_id | Returns `Err` |
| `test_sequencer_commitment_to_payload` | `ExecutionPayload::try_from(&commitment)` | Extracts payload correctly |

### 1.5 `consensus.rs` — payload_to_block

| Test | Target | Validates |
|------|--------|-----------|
| `test_payload_to_block_header` | `payload_to_block()` header fields | parent_hash, state_root, beneficiary, etc. match payload |
| `test_payload_to_block_constants` | Empty nonce, empty uncle hash | Fixed values set correctly |
| `test_payload_to_block_withdrawals` | Withdrawals conversion | Withdrawals list and root computed |
| `test_payload_to_block_empty_txs` | No transactions | Block with empty tx list, valid root |

---

## 2. Mock Tests

Mock tests validate components that depend on external services (HTTP endpoints), using
in-process mock servers. We use the `wiremock` crate for HTTP mocking.

### 2.1 `consensus.rs` — Inner::advance() Mock Flow

**Purpose**: Verify the consensus polling loop correctly fetches, validates, and processes
sequencer commitments from the consensus RPC server.

**Flow**:
1. Set up a `wiremock::MockServer` that responds to `GET /latest`
2. Create a valid `SequencerCommitment` using a known signing key
3. Configure mock to return the commitment as JSON
4. Create `Inner` pointing to the mock server
5. Call `advance()` and verify:
   - Block is sent through `block_send` channel
   - `latest_block` is updated
   - Block header fields match the payload

**Test Cases**:

| Test | Mock Behavior | Validates |
|------|---------------|-----------|
| `test_advance_valid_commitment` | Return valid commitment | Block sent, latest_block updated |
| `test_advance_duplicate_block` | Return same commitment twice | Second call is no-op (same block number) |
| `test_advance_invalid_signature` | Return commitment with wrong signer | No block sent |
| `test_advance_newer_block` | Return commitment with higher block_number | Block updated |

### 2.2 `builder.rs` — Builder Validation

**Purpose**: Verify the builder correctly validates required fields and reports errors.

**Test Cases**:

| Test | Builder State | Validates |
|------|---------------|-----------|
| `test_builder_missing_network` | No network, no config | Error: "network required" |
| `test_builder_missing_consensus_rpc` | Network set, no consensus_rpc | Error: "consensus rpc required" |
| `test_builder_with_config_overrides_network` | Config provided directly | Config used as-is |

---

## 3. Test Infrastructure

### Dependencies (dev-dependencies in Cargo.toml)
- `wiremock` — HTTP mock server for consensus RPC mocking
- `alloy` (with `signers` + `k256` features) — Generate test signing keys
- `tokio` (with `test-util` + `macros`) — Async test runtime

### Test Helpers (in `tests/helpers.rs`)
- `create_test_signing_key()` — Generate a deterministic signing key
- `create_test_payload()` — Build a minimal valid `ExecutionPayload`
- `create_test_commitment()` — Build a signed, snap-compressed `SequencerCommitment`

### File Organization
```
mantle/
├── src/
│   ├── lib.rs          ← #[cfg(test)] mod tests { ... }
│   ├── config.rs       ← #[cfg(test)] mod tests { ... }
│   ├── consensus.rs    ← #[cfg(test)] mod tests { ... }
│   ├── evm.rs          ← #[cfg(test)] mod tests { ... }
│   ├── types.rs        ← #[cfg(test)] mod tests { ... }
│   └── ...
└── tests/
    ├── helpers.rs       ← shared test utilities
    └── mock_consensus.rs ← mock integration tests for Inner::advance()
```

---

## 4. Running Tests

```bash
# Run all Mantle tests
cargo test -p helios-mantle

# Run specific test module
cargo test -p helios-mantle -- config::tests
cargo test -p helios-mantle -- consensus::tests

# Run mock tests
cargo test -p helios-mantle --test mock_consensus
```
