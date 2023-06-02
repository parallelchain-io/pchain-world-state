/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Definition of WorldState and AccountStorageState methods.
//!
//! WorldState implements the opereation to get/set account's state with identified key in world state.
//! AccountStorageState implements Contract Account’s Storage Trie to store arbitrary bit-sequence Keys and values from smart contract.

use pchain_types::cryptography::{PublicAddress, Sha256Hash};
use std::{collections::HashMap, convert::TryInto};

use crate::{
    error::WorldStateError,
    keys::{proof_level, protected_account_data, AppKey, WSKey, WSProofNode},
    storage::{
        Caches, KeyspacedInstrumentedDB, StorageMutations, TrieLevel, Value, WorldStateChanges,
        WorldStateStorage,
    },
    trie::{Mpt, Proof},
};

/// CommitSet consists of a set of operations that immediately commit to the trie.
///
/// Example:
/// ```no_run
/// let world_state = WorldState::initialize(storage);
/// // This is example address. Don't use this for real transaction.
/// let address: pchain_types::cryptography::PublicAddress = [200, 49, 188, 70, 13, 208, 8, 5, 148, 104, 28, 81, 229, 202, 203, 180, 220, 187, 48, 162, 53, 122, 83, 233, 166, 97, 173, 217, 25, 172, 106, 53];
/// world_state.with_commit().set_balance(address, 100);
/// ```
pub struct CommitSet<'a, S>
where
    S: WorldStateStorage + Send + Sync + Clone,
{
    inner: &'a mut WorldState<S>,
}

impl<'a, S> CommitSet<'a, S>
where
    S: WorldStateStorage + Send + Sync + Clone,
{
    /// Update account balance and call `commit()` to apply changes to world state.
    pub fn set_balance(&mut self, address: PublicAddress, balance: u64) {
        self.inner.trie.set_value(
            WSKey::for_protected_account_data(&address, protected_account_data::BALANCE),
            balance.to_le_bytes().to_vec(),
        );
    }

    /// Update account nonce and store and call `commit()` to apply changes to world state.
    pub fn set_nonce(&mut self, address: PublicAddress, nonce: u64) {
        self.inner.trie.set_value(
            WSKey::for_protected_account_data(&address, protected_account_data::NONCE),
            nonce.to_le_bytes().to_vec(),
        );
    }

    /// Update contract code of the account and call `commit()` to apply changes to world state.
    pub fn set_code(&mut self, address: PublicAddress, code: Vec<u8>) {
        self.inner.trie.set_value(
            WSKey::for_protected_account_data(&address, protected_account_data::CONTRACT_CODE),
            code,
        );
    }

    /// Update contract CBI of the account and call `commit()` to apply changes to world state.
    pub fn set_cbi_version(&mut self, address: PublicAddress, version: u32) {
        self.inner.trie.set_value(
            WSKey::for_protected_account_data(&address, protected_account_data::CBI_VERISON),
            version.to_le_bytes().to_vec(),
        );
    }

    /// Set key-value in account storage and call `commit()` to apply changes to world state.
    pub fn set_storage_value(&mut self, address: PublicAddress, key: AppKey, value: Value) {
        let storage_hash = match self.inner.account_storage_hash(&address) {
            Some(hash) => hash,
            None => self.inner.initialize_storage(&address),
        };

        // Update the key-value in storage trie to get the new storage hash and old value.
        let new_storage_hash = {
            let mut acc_storage =
                AccountStorageState::open_from_world_state(self.inner, &address, storage_hash);
            let mut values = HashMap::new();
            values.insert(WSKey::for_public_account_storage_state(&key), value);
            acc_storage.inserts(&values);
            let (storage_mutations, new_storage_hash) = acc_storage.get_cached_changes();
            self.inner.trie.merge_mutations(storage_mutations);

            new_storage_hash
        };
        self.inner.trie.set_value(
            WSKey::for_protected_account_data(&address, protected_account_data::STORAGE_HASH),
            new_storage_hash.to_vec(),
        );
    }
}

/// GetProof consists of set of operations that return world state data with proof.
///
/// Example:
/// ```no_run
/// let world_state = WorldState::initialize(storage);
/// let (proof, balance) = world_state.with_proof().balance(100);
/// ```
pub struct GetProof<'a, S>
where
    S: WorldStateStorage + Send + Sync + Clone,
{
    inner: &'a WorldState<S>,
}

impl<'a, S> GetProof<'a, S>
where
    S: WorldStateStorage + Send + Sync + Clone,
{
    /// Return nonce with proof of given account address,
    /// return (empty vector, 0) if the account address is not found in world state
    pub fn nonce(&self, address: PublicAddress) -> (Proof, u64) {
        let nonce_key = WSKey::for_protected_account_data(&address, protected_account_data::NONCE);
        let (proof, nonce_bs) = self.inner.trie.get_key_with_proof(&nonce_key);
        let proof: Proof = proof
            .into_iter()
            .map(|node| WSProofNode::new(proof_level::WORLDSTATE, node).into())
            .collect();
        let nonce = match nonce_bs {
            Some(n) => u64::from_le_bytes(n.try_into().unwrap()),
            None => 0,
        };
        (proof, nonce)
    }

    /// Return balance with proof of given account address,
    /// return (empty vector, 0) if the account address is not found in world state
    pub fn balance(&self, address: PublicAddress) -> (Proof, u64) {
        let balance_key =
            WSKey::for_protected_account_data(&address, protected_account_data::BALANCE);
        let (proof, balance_bs) = self.inner.trie.get_key_with_proof(&balance_key);
        let proof: Proof = proof
            .into_iter()
            .map(|node| WSProofNode::new(proof_level::WORLDSTATE, node).into())
            .collect();
        let balance = match balance_bs {
            Some(balance) => u64::from_le_bytes(balance.try_into().unwrap()),
            None => 0,
        };

        (proof, balance)
    }

    /// Return contract code with proof of given account address,
    /// return (empty vector, None) the account is External owned account
    pub fn code(&self, address: PublicAddress) -> (Proof, Option<Vec<u8>>) {
        let contract_code_key =
            WSKey::for_protected_account_data(&address, protected_account_data::CONTRACT_CODE);
        let (proof, code) = self.inner.trie.get_key_with_proof(&contract_code_key);
        let proof: Proof = proof
            .into_iter()
            .map(|node| WSProofNode::new(proof_level::WORLDSTATE, node).into())
            .collect();
        (proof, code)
    }

    /// Return Contract Binary Interface version with proof of given account address,
    /// return (empty vector, None) the account is External owned account
    pub fn cbi_version(&self, address: PublicAddress) -> (Proof, Option<u32>) {
        let cbi_key =
            WSKey::for_protected_account_data(&address, protected_account_data::CBI_VERISON);
        let (proof, cbi_version) = self.inner.trie.get_key_with_proof(&cbi_key);
        let proof: Proof = proof
            .into_iter()
            .map(|node| WSProofNode::new(proof_level::WORLDSTATE, node).into())
            .collect();
        (
            proof,
            cbi_version.map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap())),
        )
    }

    /// Get value and proof from account storage with application key
    /// return (empty vector, None) if the key is not set or account storage is not found
    pub fn storage_value(
        &self,
        address: &PublicAddress,
        key: &AppKey,
    ) -> (Sha256Hash, Proof, Option<Value>) {
        let (proof, storage_hash) = self.account_storage_hash(address);
        // Proof of the account storage hash
        let mut proof: Proof = proof
            .into_iter()
            .map(|node| WSProofNode::new(proof_level::WORLDSTATE, node).into())
            .collect();

        match storage_hash {
            Some(hash) => {
                let account_storage =
                    AccountStorageState::open_from_world_state(self.inner, address, hash);
                let ws_key = WSKey::for_public_account_storage_state(key);
                let (storage_proof, value) = account_storage.trie.get_key_with_proof(&ws_key);
                // Proof of the actual storage key
                let storage_proof: Proof = storage_proof
                    .into_iter()
                    .map(|node| WSProofNode::new(proof_level::STORAGE, node).into())
                    .collect();

                proof.extend(storage_proof);
                (hash, proof, value)
            }
            None => ([0_u8; 32], vec![], None),
        }
    }

    /// Return root hash and its proof of storage trie of given account address.
    /// None if no storage data was saved before
    fn account_storage_hash(&self, address: &PublicAddress) -> (Proof, Option<Sha256Hash>) {
        let acc_storage_key =
            WSKey::for_protected_account_data(address, protected_account_data::STORAGE_HASH);

        let (proof, storage_hash) = self.inner.trie.get_key_with_proof(&acc_storage_key);
        match storage_hash {
            Some(hash) => (proof, Some(hash.try_into().unwrap())),
            None => (vec![], None),
        }
    }
}

/// Contains consists of set of operations that checks existence of key in world state.
///
/// Example:
/// ```no_run
/// let world_state = WorldState::initialize(storage);
/// let is_contain_value = world_state.contains().storage_value(address, key);
/// ```
pub struct Contains<'a, S>
where
    S: WorldStateStorage + Send + Sync + Clone,
{
    inner: &'a WorldState<S>,
}

impl<'a, S> Contains<'a, S>
where
    S: WorldStateStorage + Send + Sync + Clone,
{
    /// Check if Contract Binary Interface version is set to world state
    pub fn cbi_version(&self, address: &PublicAddress) -> bool {
        let cbi_key =
            WSKey::for_protected_account_data(address, protected_account_data::CBI_VERISON);
        self.inner.trie.contains_key(&cbi_key)
    }

    /// Check if contract code is set to world state
    pub fn code(&self, address: PublicAddress) -> bool {
        let contract_code_key =
            WSKey::for_protected_account_data(&address, protected_account_data::CONTRACT_CODE);
        self.inner.trie.contains_key(&contract_code_key)
    }

    /// Check if the app key exists in storage
    pub fn storage_value(&self, address: &PublicAddress, key: &AppKey) -> bool {
        if let Some(storage_hash) = self.inner.account_storage_hash(address) {
            let account_storage =
                AccountStorageState::open_from_world_state(self.inner, address, storage_hash);
            let ws_key = WSKey::for_public_account_storage_state(key);
            account_storage.trie.contains_key(&ws_key)
        } else {
            false
        }
    }

    /// Check if the app key exists in storage from account state
    pub fn storage_value_from_account_storage_state(
        &self,
        account_storage_state: &AccountStorageState<S>,
        key: &AppKey,
    ) -> bool {
        let ws_key = WSKey::for_public_account_storage_state(key);
        account_storage_state.trie.contains_key(&ws_key)
    }
}

/// CachedSet consists of Write operations for cached key-value pairs pending to commit
///
/// Example:
/// ```no_run
/// let mut world_state = WorldState::initialize(storage);
/// world_state.cached().set_nonce(address, 1);
/// world_state.commit();
/// ```
pub struct CachedSet<'a, S>
where
    S: WorldStateStorage + Send + Sync + Clone,
{
    inner: &'a mut WorldState<S>,
}

impl<'a, S> CachedSet<'a, S>
where
    S: WorldStateStorage + Send + Sync + Clone,
{
    /// Update account nonce and store in in-memory cache. Pending to apply changes to world state.
    pub fn set_nonce(&mut self, address: PublicAddress, nonce: u64) {
        let nonce_key = WSKey::for_protected_account_data(&address, protected_account_data::NONCE);
        self.inner
            .cached_changes
            .insert_world_state(nonce_key, nonce.to_le_bytes().to_vec());
    }

    /// Update account balance and store in in-memory cache. Pending to apply changes to world state.
    pub fn set_balance(&mut self, address: PublicAddress, balance: u64) {
        let balance_key =
            WSKey::for_protected_account_data(&address, protected_account_data::BALANCE);
        self.inner
            .cached_changes
            .insert_world_state(balance_key, balance.to_le_bytes().to_vec());
    }

    /// Update contract code of the account and store in in-memory cache. Pending to apply changes to world state.
    pub fn set_code(&mut self, address: PublicAddress, code: Vec<u8>) {
        let contract_code_key =
            WSKey::for_protected_account_data(&address, protected_account_data::CONTRACT_CODE);
        self.inner
            .cached_changes
            .insert_world_state(contract_code_key, code);
    }

    /// Update contract CBI of the account and store in in-memory cache. Pending to apply changes to world state.
    pub fn set_cbi_version(&mut self, address: PublicAddress, version: u32) {
        let cbi_key =
            WSKey::for_protected_account_data(&address, protected_account_data::CBI_VERISON);
        self.inner
            .cached_changes
            .insert_world_state(cbi_key, version.to_le_bytes().to_vec());
    }

    /// Set key-value in account storage and store in in-memory cache. Pending to apply changes to world state.
    pub fn set_storage_value(&mut self, address: PublicAddress, key: AppKey, value: Value) {
        let ws_key = WSKey::for_public_account_storage_state(&key);
        self.inner
            .cached_changes
            .insert_storage(address, ws_key, value);
    }
}

/// CachedGet consists of Read operations for cached key-value pairs pending to commit
///
/// Example:
/// ```no_run
/// let mut world_state = WorldState::initialize(storage);
/// let value = world_state.cached_get().storage_value(address, &app_key);
/// ```
pub struct CachedGet<'a, S>
where
    S: WorldStateStorage + Send + Sync + Clone,
{
    inner: &'a WorldState<S>,
}

impl<'a, S> CachedGet<'a, S>
where
    S: WorldStateStorage + Send + Sync + Clone,
{
    /// Get values from in-memory cache given an AppKey.
    pub fn storage_value(&self, address: PublicAddress, key: &AppKey) -> Option<Value> {
        let ws_key = WSKey::for_public_account_storage_state(key);
        if let Some(cache_map) = self.inner.cached_changes.storage().get(&address) {
            if let Some(value) = cache_map.get(&ws_key) {
                return Some(value.clone());
            }
        }
        None
    }
}

/// WorldState is to read and update data stored in trie structure. It builds up `write_set` to contain all of the storage mutations required to atomically
/// persist a new, correct world state into persistent storage. It provides `get`s methods to read world state data from a persistent storage that
/// implemented the trait [WorldStateStorage]. `keyspace` specifies the partition in database to store worldstate data and used as the prefix of every
/// trie node key.
#[derive(Clone)]
pub struct WorldState<S>
where
    S: WorldStateStorage + Send + Sync + Clone,
{
    trie: Mpt<S>,
    cached_changes: Caches,
}

impl<S: WorldStateStorage + Send + Sync + Clone> WorldState<S> {
    /// `initialize` is to create a new Trie with empty state hash. This is a tool to prepares the `write_set` that contains all
    /// storage mutations by genesis transactions. The `write_set` will be used to initialize persistent storage managed by Hotstuff.
    /// This function should be called only once, during the first startup.
    pub fn initialize(storage: S) -> WorldState<S> {
        let db = KeyspacedInstrumentedDB::open(storage, TrieLevel::WorldState);
        let trie = Mpt::new(db);

        WorldState {
            trie,
            cached_changes: Caches::new(),
        }
    }

    /// `open` is to open the world state from given storage source and state_hash.
    /// Returns an error if root_hash does not exist or unable to open the trie.
    pub fn open(storage: S, root_hash: Sha256Hash) -> Result<Self, WorldStateError> {
        let db = KeyspacedInstrumentedDB::open(storage, TrieLevel::WorldState);
        let trie = Mpt::open(db, root_hash)?;

        Ok(WorldState {
            trie,
            cached_changes: Caches::new(),
        })
    }

    /// `with_commit` returns structure [CommitSet] for commit operations
    pub fn with_commit(&mut self) -> CommitSet<S> {
        CommitSet { inner: self }
    }

    /// `with_proof` returns structure [GetProof] for proof getters
    pub fn with_proof(&self) -> GetProof<S> {
        GetProof { inner: self }
    }

    /// `contains` returns structure [Contains] for Key-existence checking
    pub fn contains(&self) -> Contains<S> {
        Contains { inner: self }
    }

    /// `cached` returns structure [CachedSet] for cached write operations
    pub fn cached(&mut self) -> CachedSet<S> {
        CachedSet { inner: self }
    }

    /// `cached_get` returns structure [CachedGet] for cached read operations
    pub fn cached_get(&self) -> CachedGet<S> {
        CachedGet { inner: self }
    }

    /// Return nonce of given account address. 0 if the account address is not found in world state.
    pub fn nonce(&self, address: PublicAddress) -> u64 {
        let nonce_key = WSKey::for_protected_account_data(&address, protected_account_data::NONCE);
        match self.trie.get_key(&nonce_key) {
            Some(nonce) => u64::from_le_bytes(nonce.try_into().unwrap()),
            None => 0,
        }
    }

    /// Return balance of given account address. 0 if the account address is not found in world state.
    pub fn balance(&self, address: PublicAddress) -> u64 {
        let balance_key =
            WSKey::for_protected_account_data(&address, protected_account_data::BALANCE);
        match self.trie.get_key(&balance_key) {
            Some(balance) => u64::from_le_bytes(balance.try_into().unwrap()),
            None => 0,
        }
    }

    /// Return contract code of given account address. None if the account is External owned account.
    pub fn code(&self, address: PublicAddress) -> Option<Vec<u8>> {
        let contract_code_key =
            WSKey::for_protected_account_data(&address, protected_account_data::CONTRACT_CODE);
        self.trie.get_key(&contract_code_key)
    }

    /// Return Contract Binary Interface version of given account address. None if the account is External owned account.
    pub fn cbi_version(&self, address: PublicAddress) -> Option<u32> {
        let cbi_key =
            WSKey::for_protected_account_data(&address, protected_account_data::CBI_VERISON);
        self.trie
            .get_key(&cbi_key)
            .map(|version| u32::from_le_bytes(version.try_into().unwrap()))
    }

    /// Get value from account storage with application key. None if the key was not set or account storage is not found.
    pub fn storage_value(&self, address: &PublicAddress, key: &AppKey) -> Option<Value> {
        let storage_hash = self.account_storage_hash(address);
        match storage_hash {
            Some(hash) => {
                let account_storage =
                    AccountStorageState::open_from_world_state(self, address, hash);
                let ws_key = WSKey::for_public_account_storage_state(key);
                account_storage.trie.get_key(&ws_key)
            }
            None => None,
        }
    }

    /// Initialize account storage trie from empty hash
    pub fn initialize_account_storage_state(
        &self,
        address: PublicAddress,
    ) -> AccountStorageState<S> {
        AccountStorageState::initialize(self, &address)
    }

    /// Open account storage trie from world state with an address.
    pub fn account_storage_state(&self, address: PublicAddress) -> Option<AccountStorageState<S>> {
        let storage_hash = self.account_storage_hash(&address)?;
        Some(AccountStorageState::open_from_world_state(
            self,
            &address,
            storage_hash,
        ))
    }

    /// Get values from account storage with vector of application key Return a hashmap of AppKey-value pairs.
    /// If a key is not set or account storage is not found, return None for that key.
    pub fn storage_values(
        &self,
        address: &PublicAddress,
        key_set: &Vec<AppKey>,
    ) -> HashMap<AppKey, Option<Value>> {
        let mut result = HashMap::with_capacity(key_set.len());
        let storage_hash = self.account_storage_hash(address);
        match storage_hash {
            Some(hash) => {
                let account_storage =
                    AccountStorageState::open_from_world_state(self, address, hash);
                for key in key_set {
                    let ws_key = WSKey::for_public_account_storage_state(key);
                    result.insert(key.clone(), account_storage.trie.get_key(&ws_key));
                }
            }
            None => {
                for key in key_set {
                    result.insert(key.clone(), None);
                }
            }
        }
        result
    }

    /// Get the whole list of key-value pair associated with this address
    /// Return None if
    /// 1. No storage is associated with this address
    /// 2. Storage was set up before, but no key-value pair is associated with this address in the current state.
    pub fn account_storage(&self, address: &PublicAddress) -> Option<HashMap<AppKey, Value>> {
        let storage_hash = self.account_storage_hash(address);
        match storage_hash {
            Some(hash) => {
                let acc_storage = AccountStorageState::open_from_world_state(self, address, hash);
                acc_storage.get_all_data()
            }
            None => None,
        }
    }

    /// Commit cached changes to world state, state hash would be updated.
    pub fn commit(&mut self) {
        // let (mut cached_protected, cached_storage) = self.cached_changes.consume();
        // loop and apply cached storage changes
        let addresses: Vec<PublicAddress> = self.cached_changes.storage().keys().copied().collect();
        for address in addresses {
            let storage_hash = match self.account_storage_hash(&address) {
                Some(hash) => hash,
                None => self.initialize_storage(&address),
            };
            let values = self.cached_changes.storage().get(&address).unwrap();

            // Update the key-value in storage trie to get the new storage hash and old value.
            let new_storage_hash = {
                let mut acc_storage =
                    AccountStorageState::open_from_world_state(self, &address, storage_hash);
                acc_storage.inserts(values);
                let (storage_mutations, new_storage_hash) = acc_storage.get_cached_changes();
                self.trie.merge_mutations(storage_mutations);

                new_storage_hash
            };

            let storage_hash_key =
                WSKey::for_protected_account_data(&address, protected_account_data::STORAGE_HASH);
            self.cached_changes
                .insert_world_state(storage_hash_key, new_storage_hash.to_vec());
        }

        // apply protected data changes
        self.trie.set_values(self.cached_changes.world_state());

        // clear cache changes after updating trie
        self.cached_changes.clear();
    }

    /// Consume WorldState and returns the cached changes since state creation.
    /// WorldStateChanges.mutations’s keys are ‘actual‘ trie node keys, minus the WorldState keyspace defined in application.
    pub fn commit_and_close(mut self) -> WorldStateChanges {
        self.commit();

        let (mutations, next_state_hash) = self.trie.close();
        let (inserts, deletes) = mutations.consume();
        WorldStateChanges {
            inserts,
            deletes,
            next_state_hash,
        }
    }

    /// Clear all uncommitted changes in caches
    pub fn clear_cached_changes(&mut self) {
        self.cached_changes.clear();
    }

    /// Discard all changes, including both committed and uncommitted changes, and restore to the state hash while open world state.
    pub fn discard_pending_writes(&mut self) {
        self.trie.flush();
        self.cached_changes.clear();
    }

    /// Return root hash of storage trie of given account address
    /// return None in two conditions:
    /// 1. the address is an External owned account, or
    /// 2. the address is a contract account, but no storage data is saved before
    pub fn account_storage_hash(&self, address: &PublicAddress) -> Option<Sha256Hash> {
        let acc_storage_key =
            WSKey::for_protected_account_data(address, protected_account_data::STORAGE_HASH);

        let storage_hash = self.trie.get_key(&acc_storage_key);
        storage_hash.map(|hash| hash.try_into().unwrap())
    }

    /// Initialize a new storage trie with empty root prefixed by account address
    fn initialize_storage(&mut self, address: &PublicAddress) -> Sha256Hash {
        let acc_storage = AccountStorageState::initialize(self, address);
        let (storage_write_set, storage_hash) = acc_storage.get_cached_changes();

        self.trie.merge_mutations(storage_write_set);
        storage_hash
    }
}

/// AccountStorageState is used by operations in [WorldState]. Each account address is allocated specific location for its account storage. AccountStorageState is initialized with an address.
pub struct AccountStorageState<S>
where
    S: WorldStateStorage + Send + Sync + Clone,
{
    address: PublicAddress,
    trie: Mpt<S>,
}

impl<S: WorldStateStorage + Send + Sync + Clone> AccountStorageState<S> {
    /// Initialize a trie for an address at TrieLevel [TrieLevel::Storage].
    fn initialize(world_state: &WorldState<S>, address: &PublicAddress) -> AccountStorageState<S> {
        let mut storage = world_state.trie.db().to_owned();
        storage.set_keyspace(TrieLevel::Storage(*address));
        let trie = Mpt::new(storage);
        AccountStorageState {
            address: *address,
            trie,
        }
    }

    /// Open a trie for an address at TrieLevel [TrieLevel::Storage] from world state.
    fn open_from_world_state(
        world_state: &WorldState<S>,
        address: &PublicAddress,
        storage_hash: Sha256Hash,
    ) -> AccountStorageState<S> {
        let mut storage = world_state.trie.db().to_owned();
        storage.set_keyspace(TrieLevel::Storage(*address));
        let trie = Mpt::open(storage, storage_hash).expect("Unable to open account storage");
        AccountStorageState {
            address: *address,
            trie,
        }
    }

    /// Address that was used for opening this account state.
    pub fn address(&self) -> PublicAddress {
        self.address
    }

    /// Get the value for an App Key.
    pub fn get(&self, key: &AppKey) -> Option<Value> {
        let ws_key = WSKey::for_public_account_storage_state(key);
        self.trie.get_key(&ws_key)
    }

    /// Set a hashmap of key-values in account storage and call `commit()` to apply changes to world state.
    fn inserts(&mut self, values: &HashMap<WSKey, Value>) {
        self.trie.set_values(values)
    }

    /// Consume account state to commit the changes and return mutations.
    fn get_cached_changes(self) -> (StorageMutations, Sha256Hash) {
        let (mutations, storage_hash) = self.trie.close();
        (mutations, storage_hash.to_owned())
    }

    /// Get all the key-values in this Account State.
    fn get_all_data(&self) -> Option<HashMap<AppKey, Value>> {
        let data = self.trie.get_all_elements();
        if !data.is_empty() {
            Some(data)
        } else {
            None
        }
    }
}
