use std::sync::RwLock;

use alloy::{
    consensus::TrieAccount,
    primitives::{Address, Bytes, B256},
    rpc::types::{EIP1186AccountProofResponse, EIP1186StorageProof},
};
use schnellru::{ByLength, LruMap};

use helios_common::types::Account;

// Cache capacities
// High turnover due to block updates.
const ACCOUNTS_CACHE_SIZE: u32 = 128;

// Each entry is an LRU of slots per storage root.
const STORAGE_CACHE_SIZE: u32 = 64;
const STORAGE_SLOTS_PER_ROOT_CACHE_SIZE: u32 = 256;

// Code: Static and can be shared. Most valuable cache.
const CODE_CACHE_SIZE: u32 = 256;

pub struct Cache {
    /// Storage proofs: content-addressed by storage_hash
    /// storage_hash -> slot -> full storage proof
    storage: RwLock<LruMap<B256, LruMap<B256, EIP1186StorageProof>>>,

    /// Code: content-addressed by code_hash
    /// code_hash -> bytecode
    code: RwLock<LruMap<B256, Bytes>>,

    /// Account proofs: block-specific
    /// (address, block_hash) -> account proof response (with empty storage_proof)
    accounts: RwLock<LruMap<(Address, B256), EIP1186AccountProofResponse>>,
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

impl Cache {
    pub fn new() -> Self {
        Self {
            storage: RwLock::new(LruMap::new(ByLength::new(STORAGE_CACHE_SIZE))),
            code: RwLock::new(LruMap::new(ByLength::new(CODE_CACHE_SIZE))),
            accounts: RwLock::new(LruMap::new(ByLength::new(ACCOUNTS_CACHE_SIZE))),
        }
    }

    /// Insert a VERIFIED account proof response into the cache.
    ///
    /// This method distributes data to appropriate caches:
    /// - Account proof (with empty storage_proof) goes to accounts cache
    /// - Storage proofs go to the content-addressed storage cache
    /// - Code (if provided) goes to the content-addressed code cache
    ///
    /// # Arguments
    /// * `response` - The account proof response from RPC
    /// * `code` - Optional contract code (if it was fetched)
    /// * `block_hash` - The block hash this proof is valid for
    pub(crate) fn insert(
        &self,
        response: EIP1186AccountProofResponse,
        code: Option<Bytes>,
        block_hash: B256,
    ) {
        let storage_hash = response.storage_hash;

        if !response.storage_proof.is_empty() {
            let mut storage = self.storage.write().unwrap_or_else(|e| e.into_inner());

            if storage.peek(&storage_hash).is_none() {
                storage.insert(
                    storage_hash,
                    LruMap::new(ByLength::new(STORAGE_SLOTS_PER_ROOT_CACHE_SIZE)),
                );
            }

            if let Some(storage_map) = storage.get(&storage_hash) {
                for proof in &response.storage_proof {
                    storage_map.insert(proof.key.as_b256(), proof.clone());
                }
            }
        }

        if let Some(code) = code {
            let mut code_cache = self.code.write().unwrap_or_else(|e| e.into_inner());
            code_cache.insert(response.code_hash, code);
        }

        let account_response = EIP1186AccountProofResponse {
            storage_proof: vec![],
            ..response
        };

        let mut accounts = self.accounts.write().unwrap_or_else(|e| e.into_inner());
        accounts.insert((account_response.address, block_hash), account_response);
    }

    /// Get code by code hash. Hash MUST come from a verified account proof.
    pub fn get_code(&self, code_hash: B256) -> Option<Bytes> {
        let mut code = self.code.write().unwrap_or_else(|e| e.into_inner());
        code.get(&code_hash).cloned()
    }

    /// Insert code into the cache by code hash. Hash MUST come from a verified account proof.
    pub fn insert_code(&self, code_hash: B256, code: Bytes) {
        let mut code_cache = self.code.write().unwrap_or_else(|e| e.into_inner());
        code_cache.insert(code_hash, code);
    }

    /// Get an account proof response with requested storage slots.
    ///
    /// # Arguments
    /// * `address` - The address of the account
    /// * `slots` - The storage slots to get
    /// * `block_hash` - The block hash this proof is valid for
    ///
    /// # Returns
    /// * `(EIP1186AccountProofResponse, Vec<B256>)` - The account proof response and the missing slots
    pub fn get_account_proof(
        &self,
        address: Address,
        slots: &[B256],
        block_hash: B256,
    ) -> Option<(EIP1186AccountProofResponse, Vec<B256>)> {
        let account = {
            let mut accounts = self.accounts.write().unwrap_or_else(|e| e.into_inner());
            accounts.get(&(address, block_hash)).cloned()?
        };

        let storage_hash = account.storage_hash;
        let mut storage_proofs = Vec::new();
        let mut missing_slots = Vec::new();

        {
            let mut storage = self.storage.write().unwrap_or_else(|e| e.into_inner());
            if let Some(storage_map) = storage.get(&storage_hash) {
                for slot in slots {
                    if let Some(proof) = storage_map.get(slot) {
                        storage_proofs.push(proof.clone());
                    } else {
                        missing_slots.push(*slot);
                    }
                }
            } else if !slots.is_empty() {
                // No storage cached for this storage_hash, all slots are missing
                missing_slots.extend_from_slice(slots);
            }
        }

        let response = EIP1186AccountProofResponse {
            storage_proof: storage_proofs,
            ..account
        };

        Some((response, missing_slots))
    }

    /// Get an Account with requested storage slots.
    pub fn get_account(
        &self,
        address: Address,
        slots: &[B256],
        block_hash: B256,
    ) -> Option<(Account, Vec<B256>)> {
        let (response, missing_slots) = self.get_account_proof(address, slots, block_hash)?;

        let code = self.get_code(response.code_hash);

        let account = Account {
            account: TrieAccount {
                nonce: response.nonce,
                balance: response.balance,
                storage_root: response.storage_hash,
                code_hash: response.code_hash,
            },
            code,
            account_proof: response.account_proof,
            storage_proof: response.storage_proof,
        };

        Some((account, missing_slots))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::{address, b256, U256};

    impl Cache {
        fn get_storage(&self, address: Address, slot: B256, block_hash: B256) -> Option<U256> {
            let storage_hash = {
                let mut accounts = self.accounts.write().unwrap_or_else(|e| e.into_inner());
                let account = accounts.get(&(address, block_hash))?;
                account.storage_hash
            };

            let mut storage = self.storage.write().unwrap_or_else(|e| e.into_inner());
            let storage_map = storage.get(&storage_hash)?;
            let proof = storage_map.get(&slot)?;

            Some(proof.value)
        }
    }

    fn mock_account_proof_response(
        address: Address,
        storage_hash: B256,
        code_hash: B256,
        storage_proofs: Vec<EIP1186StorageProof>,
    ) -> EIP1186AccountProofResponse {
        EIP1186AccountProofResponse {
            address,
            balance: U256::from(1000),
            code_hash,
            nonce: 1,
            storage_hash,
            account_proof: vec![],
            storage_proof: storage_proofs,
        }
    }

    fn mock_storage_proof(slot: B256, value: U256) -> EIP1186StorageProof {
        EIP1186StorageProof {
            key: slot.into(),
            value,
            proof: vec![],
        }
    }

    #[test]
    fn test_insert_and_get_storage() {
        let cache = Cache::new();

        let address = address!("0000000000000000000000000000000000000001");
        let block_hash = b256!("0000000000000000000000000000000000000000000000000000000000000001");
        let storage_hash =
            b256!("0000000000000000000000000000000000000000000000000000000000000002");
        let code_hash = b256!("0000000000000000000000000000000000000000000000000000000000000003");
        let slot = b256!("0000000000000000000000000000000000000000000000000000000000000004");
        let value = U256::from(42);

        let response = mock_account_proof_response(
            address,
            storage_hash,
            code_hash,
            vec![mock_storage_proof(slot, value)],
        );

        cache.insert(response, None, block_hash);

        // Should be able to retrieve storage
        assert_eq!(cache.get_storage(address, slot, block_hash), Some(value));

        // Non-existent slot should return None
        let other_slot = b256!("0000000000000000000000000000000000000000000000000000000000000005");
        assert_eq!(cache.get_storage(address, other_slot, block_hash), None);
    }

    #[test]
    fn test_content_addressed_storage_sharing() {
        let cache = Cache::new();

        let address1 = address!("0000000000000000000000000000000000000001");
        let address2 = address!("0000000000000000000000000000000000000002");
        let block_hash1 = b256!("0000000000000000000000000000000000000000000000000000000000000001");
        let block_hash2 = b256!("0000000000000000000000000000000000000000000000000000000000000002");
        let storage_hash =
            b256!("0000000000000000000000000000000000000000000000000000000000000003"); // Same!
        let code_hash = b256!("0000000000000000000000000000000000000000000000000000000000000004");
        let slot = b256!("0000000000000000000000000000000000000000000000000000000000000005");
        let value = U256::from(100);

        // Insert for address1 at block1 with storage
        let response1 = mock_account_proof_response(
            address1,
            storage_hash,
            code_hash,
            vec![mock_storage_proof(slot, value)],
        );
        cache.insert(response1, None, block_hash1);

        // Insert for address2 at block2 with SAME storage_hash but NO storage proofs
        let response2 = mock_account_proof_response(address2, storage_hash, code_hash, vec![]);
        cache.insert(response2, None, block_hash2);

        // address2 should be able to get the storage value because storage_hash matches!
        assert_eq!(cache.get_storage(address2, slot, block_hash2), Some(value));
    }

    #[test]
    fn test_get_account_proof_with_missing_slots() {
        let cache = Cache::new();

        let address = address!("0000000000000000000000000000000000000001");
        let block_hash = b256!("0000000000000000000000000000000000000000000000000000000000000001");
        let storage_hash =
            b256!("0000000000000000000000000000000000000000000000000000000000000002");
        let code_hash = b256!("0000000000000000000000000000000000000000000000000000000000000003");
        let slot1 = b256!("0000000000000000000000000000000000000000000000000000000000000004");
        let slot2 = b256!("0000000000000000000000000000000000000000000000000000000000000005");
        let slot3 = b256!("0000000000000000000000000000000000000000000000000000000000000006");

        // Insert with only slot1 cached
        let response = mock_account_proof_response(
            address,
            storage_hash,
            code_hash,
            vec![mock_storage_proof(slot1, U256::from(1))],
        );
        cache.insert(response, None, block_hash);

        // Request slot1, slot2, slot3
        let (result, missing) = cache
            .get_account_proof(address, &[slot1, slot2, slot3], block_hash)
            .unwrap();

        // Should have slot1 in response
        assert_eq!(result.storage_proof.len(), 1);
        assert_eq!(result.storage_proof[0].key.as_b256(), slot1);

        // Should report slot2 and slot3 as missing
        assert_eq!(missing.len(), 2);
        assert!(missing.contains(&slot2));
        assert!(missing.contains(&slot3));
    }

    #[test]
    fn test_code_caching() {
        let cache = Cache::new();

        let address = address!("0000000000000000000000000000000000000001");
        let block_hash = b256!("0000000000000000000000000000000000000000000000000000000000000001");
        let storage_hash =
            b256!("0000000000000000000000000000000000000000000000000000000000000002");
        let code_hash = b256!("0000000000000000000000000000000000000000000000000000000000000003");
        let code = Bytes::from_static(&[0x60, 0x80, 0x60, 0x40]);

        let response = mock_account_proof_response(address, storage_hash, code_hash, vec![]);
        cache.insert(response, Some(code.clone()), block_hash);

        // Should be able to retrieve code
        assert_eq!(cache.get_code(code_hash), Some(code.clone()));

        // get_account should include the code
        let (account, _) = cache.get_account(address, &[], block_hash).unwrap();
        assert_eq!(account.code, Some(code));
    }
}
