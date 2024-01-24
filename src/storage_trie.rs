/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! This mod provide struct and implementations for account storage

use std::collections::{HashMap, HashSet};
use std::mem::size_of;

use crate::error::{MptError, WorldStateError};
use crate::mpt::{proof_level, KeyVisibility, Mpt, Proof, WSProofNode};
use crate::world_state::WorldStateChanges;
use crate::TrieKeyBuildError;
use crate::{
    db::{KeyInstrumentedDB, DB},
    version::*,
};
use hash_db::Hasher;
use pchain_types::cryptography::{PublicAddress, Sha256Hash};
use reference_trie::RefHasher;

const NULL_NODE_KEY: &[u8] = &[0_u8];
/// Struct store account storage information for contract account
#[derive(Debug, Clone)]
pub struct StorageTrie<'a, S, V>
where
    S: DB + Send + Sync + Clone,
    V: VersionProvider + Send + Sync + Clone,
{
    trie: Mpt<'a, S, V>,
}

/// interfaces can be called by outside user
impl<'a, S: DB + Send + Sync + Clone, V: VersionProvider + Send + Sync + Clone>
    StorageTrie<'a, S, V>
{
    /// `get` return storage value by specific storage key
    ///
    /// empty vector if key is not found in storage trie
    ///
    /// Error if storage_hash does not exists or missed some trie nodes
    pub fn get(&self, key: &Vec<u8>) -> Result<Option<Vec<u8>>, MptError> {
        let trie_key: Vec<u8> = storage_key::<V>(key);
        self.trie.get(&trie_key)
    }

    /// `get_with_proof` return storage value with proof by specific storage key
    ///
    /// (empty vector, empty vector) if key is not found in storage trie
    ///
    /// Error if storage_hash does not exists or missed some trie nodes
    pub fn get_with_proof(&self, key: &Vec<u8>) -> Result<(Proof, Option<Vec<u8>>), MptError> {
        let trie_key: Vec<u8> = storage_key::<V>(key);
        self.trie.get_with_proof(&trie_key).map(|(proof, value)| {
            let proof = proof
                .into_iter()
                .map(|node| WSProofNode::new(proof_level::STORAGE, node).into())
                .collect();
            (proof, value)
        })
    }

    /// `contains` is to check if the key exists in current StorageTrie or not
    ///
    /// Error if storage_hash does not exists or missed some trie nodes
    pub fn contains(&self, key: &Vec<u8>) -> Result<bool, MptError> {
        let storage_key: Vec<u8> = storage_key::<V>(key);
        self.trie.contains(&storage_key)
    }

    /// `set` is to set/update <Key, Value> pair in StorageTrie
    pub fn set(&mut self, key: &Vec<u8>, value: Vec<u8>) -> Result<(), MptError> {
        let storage_key: Vec<u8> = storage_key::<V>(key);
        self.trie.set(&storage_key, value)
    }

    /// `remove` is to remove key in StorageTrie
    pub fn remove(&mut self, key: &Vec<u8>) -> Result<(), MptError> {
        let storage_key: Vec<u8> = storage_key::<V>(key);
        self.trie.remove(&storage_key)
    }

    /// `remove trie` is to clear the target StorageTrie and inside the target account
    pub fn remove_trie(&mut self) -> Result<(), MptError> {
        let mut key_set = HashSet::new();
        self.trie.iterate_all(|key, _| {
            key_set.insert(key);
            Ok::<(), MptError>(())
        })?;
        // batch delete keys
        self.trie.batch_remove(&key_set)
    }

    /// `batch_set` is to batch set/update <Key, Value> pairs in StorageTrie
    pub fn batch_set(&mut self, data: &HashMap<Vec<u8>, Vec<u8>>) -> Result<(), MptError> {
        let mut storage_data_set = HashMap::new();
        for (key, value) in data.iter() {
            let storage_key: Vec<u8> = storage_key::<V>(key);
            storage_data_set.insert(storage_key, value.clone());
        }
        self.trie.batch_set(&storage_data_set)
    }
}

/// intefaces called by [WorldState](crate::world_state::WorldState)
impl<'a, S: DB + Send + Sync + Clone, V: VersionProvider + Send + Sync + Clone>
    StorageTrie<'a, S, V>
{
    /// `new` called by [WorldState](crate::world_state::WorldState) to create a new StorageTrie with empty storage_hash
    pub(crate) fn new(storage: &'a S, address: &PublicAddress) -> Self {
        let db = KeyInstrumentedDB::new(storage, address.to_vec());
        let trie = Mpt::<S, V>::new(db);
        StorageTrie { trie }
    }

    /// `open` called by [WorldState](crate::world_state::WorldState) to open a StorageTrie with an existing storage_hash
    pub(crate) fn open(storage: &'a S, storage_hash: Sha256Hash, address: &PublicAddress) -> Self {
        let db = KeyInstrumentedDB::new(storage, address.to_vec());
        let trie = Mpt::open(db, storage_hash);
        StorageTrie { trie }
    }

    /// `root_hash` called by [WorldState](crate::world_state::WorldState) to get the root hash of the current trie
    pub(crate) fn root_hash(&self) -> Sha256Hash {
        self.trie.root_hash()
    }

    /// `close` called by [WorldState](crate::world_state::WorldState) return all cached updates in current StorageTrie and updated storage_hash
    pub(crate) fn close(&mut self) -> WorldStateChanges {
        let mpt_changes = self.trie.close();
        WorldStateChanges {
            inserts: mpt_changes.0,
            deletes: mpt_changes.1,
            new_root_hash: mpt_changes.2,
        }
    }
}

impl<'a, S: DB + Send + Sync + Clone> StorageTrie<'a, S, V1> {
    pub(crate) fn upgrade(mut self) -> Result<StorageTrie<'a, S, V2>, WorldStateError> {
        let mut data_map: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
        // check current root hash is equal to empty trie root hash
        if self.trie.root_hash() == RefHasher::hash(NULL_NODE_KEY) {
            // get the V2 mpt with the empty root hash
            let mpt_v2: Mpt<'a, S, V2> = self.trie.deinit_and_upgrade()?;
            return Ok(StorageTrie { trie: mpt_v2 });
        }
        let mut key_set: HashSet<Vec<u8>> = HashSet::new();
        self.trie.iterate_all(|key, value| {
            key_set.insert(key.clone());
            let storage_key_v1 = drop_visibility_type::<V1>(&key)?;
            let storage_key_v2 = storage_key::<V2>(&storage_key_v1);
            data_map.insert(storage_key_v2, value);
            Ok::<(), WorldStateError>(())
        })?;
        // batch delete
        self.trie.batch_remove(&key_set)?;
        // after delete all <key, value> pair, destroy the empty trie and get the V2 mpt for stroage
        let mut trie_v2 = self.trie.deinit_and_upgrade()?;
        // batch insert all data into the new mpt
        trie_v2.batch_set(&data_map)?;
        Ok(StorageTrie { trie: trie_v2 })
    }
}

/// `storage_key` is to crate the key for [StorageTrie](crate::storage::StorageTrie)
///
/// V1 StorageTrie Key is in form KeyVisibility + Vec<u8>
///
/// V2 StorageTrie Key is in form Vec<u8>
pub(crate) fn storage_key<V: VersionProvider>(key: &Vec<u8>) -> Vec<u8> {
    match <V>::version() {
        Version::V1 => {
            let mut storage_key: Vec<u8> = Vec::with_capacity(size_of::<u8>() + key.len());
            storage_key.push(KeyVisibility::Public as u8);
            storage_key.extend_from_slice(key);
            storage_key
        }
        Version::V2 => {
            let mut storage_key: Vec<u8> = Vec::new();
            storage_key.extend_from_slice(key);
            storage_key
        }
    }
}

/// `drop_visibility_type` is to drop the visibility byte from [AccountsTrie](crate::accounts_trie::AccountsTrie) Key or [StorageTrie](crate::storage_trie::StorageTrie) Key
pub(crate) fn drop_visibility_type<V: VersionProvider>(
    key: &[u8],
) -> Result<Vec<u8>, TrieKeyBuildError> {
    if key.len() < size_of::<u8>() {
        return Err(TrieKeyBuildError::Other);
    }
    match <V>::version() {
        Version::V1 => Ok(key[size_of::<u8>()..].to_vec()),
        Version::V2 => Ok(key.to_vec()),
    }
}
