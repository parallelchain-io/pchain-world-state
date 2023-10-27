/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! This mod provide struct and implementations for account storage

use std::collections::{HashMap, HashSet};

use crate::error::{MptError, WorldStateError};
use crate::mpt::{Mpt, Proof};
use crate::world_state::WorldStateChanges;
use crate::{
    db::{KeyInstrumentedDB, DB},
    version::*,
};
use hash_db::Hasher;
use pchain_types::cryptography::{PublicAddress, Sha256Hash};

use crate::proof_node::{proof_level, WSProofNode};
use crate::trie_key::TrieKey;
use reference_trie::{ExtensionLayout, NoExtensionLayout, RefHasher};
use trie_db::{Trie, TrieDBBuilder};

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

/// Struct store return information when call `destory()`
///
/// inserts: <key, value> pairs need to be insert into physical db
///
/// deletes: keys need to be delete from physical db
///
/// data_map: data which need to rebuild this StorageTrie
#[derive(Debug, Clone)]
pub(crate) struct DestroyStorageChanges {
    pub(crate) inserts: HashMap<Vec<u8>, Vec<u8>>,
    pub(crate) deletes: HashSet<Vec<u8>>,
    pub(crate) data_map: HashMap<Vec<u8>, Vec<u8>>,
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
        let trie_key: Vec<u8> = TrieKey::<V>::storage_key(key);
        let value = self.trie.get(&trie_key)?;
        Ok(value)
    }

    /// `get_with_proof` return storage value with proof by specific storage key
    ///
    /// (empty vector, empty vector) if key is not found in storage trie
    ///
    /// Error if storage_hash does not exists or missed some trie nodes
    pub fn get_with_proof(&self, key: &Vec<u8>) -> Result<(Proof, Option<Vec<u8>>), MptError> {
        let trie_key: Vec<u8> = TrieKey::<V>::storage_key(key);
        let (proof, value) = self.trie.get_with_proof(&trie_key)?;
        let proof: Proof = proof
            .into_iter()
            .map(|node| WSProofNode::new(proof_level::STORAGE, node).into())
            .collect();
        Ok((proof, value))
    }

    /// `all` is to iterator all <Key, Value> in current StorageTrie
    ///
    /// Return a HashMap of (`Vec<u8>`, `Vec<u8>`)
    ///
    /// Error if storage_hash does not exists or missed some trie nodes
    pub fn all(&self) -> Result<HashMap<Vec<u8>, Vec<u8>>, WorldStateError> {
        let storage_map = self.trie.all().map_err(WorldStateError::MptError)?;
        let mut ret_map: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
        for (key, value) in storage_map.into_iter() {
            let storage_key: Vec<u8> = TrieKey::<V>::drop_visibility_type(&key);
            ret_map.insert(storage_key, value);
        }
        Ok(ret_map)
    }

    /// `contains` is to check if the key exists in current StorageTrie or not
    ///
    /// Error if storage_hash does not exists or missed some trie nodes
    pub fn contains(&self, key: &Vec<u8>) -> Result<bool, MptError> {
        let storage_key: Vec<u8> = TrieKey::<V>::storage_key(key);
        let exsits = self.trie.contains(&storage_key)?;
        Ok(exsits)
    }

    /// `set` is to set/update <Key, Value> pair in StorageTrie
    pub fn set(&mut self, key: &Vec<u8>, value: Vec<u8>) -> Result<(), MptError> {
        let storage_key: Vec<u8> = TrieKey::<V>::storage_key(key);
        self.trie.set(&storage_key, value)?;
        Ok(())
    }

    /// `remove` is to remove key in StorageTrie
    pub fn remove(&mut self, key: &Vec<u8>) -> Result<(), MptError> {
        let storage_key: Vec<u8> = TrieKey::<V>::storage_key(key);
        self.trie.remove(&storage_key)?;
        Ok(())
    }

    /// `remove trie` is to clear the target StorageTrie and inside the target account
    pub fn remove_trie(&mut self) -> Result<(), MptError> {
        let mut key_set: HashSet<Vec<u8>> = HashSet::new();
        for (key, _) in self.trie.all()? {
            key_set.insert(key);
        }
        // batch delete keys
        self.trie.batch_remove(&key_set)
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

    /// `batch_set` called by [WorldState](crate::world_state::WorldState) to batch set/update <Key, Value> pairs in StorageTrie
    pub(crate) fn batch_set(&mut self, data: &HashMap<Vec<u8>, Vec<u8>>) -> Result<(), MptError> {
        let mut storage_data_set = HashMap::new();
        for (key, value) in data.iter() {
            let storage_key: Vec<u8> = TrieKey::<V>::storage_key(key);
            storage_data_set.insert(storage_key, value.clone());
        }
        self.trie.batch_set(&storage_data_set)?;
        Ok(())
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

    /// `destory` called by [WorldState](crate::world_state::WorldState) return all data in current StorageTrie as HashMap, and destory the empty StorageTrie
    pub(crate) fn destroy(&mut self) -> Result<DestroyStorageChanges, MptError> {
        let mut data_map: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
        // check current root hash is equal to empty trie root hash
        if self.trie.root_hash() == RefHasher::hash(NULL_NODE_KEY) {
            self.trie.deinit()?;
            let mpt_changes = self.trie.close();
            return Ok(DestroyStorageChanges {
                inserts: mpt_changes.0,
                deletes: mpt_changes.1,
                data_map: HashMap::new(),
            });
        }
        match <V>::version() {
            Version::V1 => {
                let current_root_hash = self.trie.root_hash();
                let storage_trie =
                    TrieDBBuilder::<NoExtensionLayout>::new(&self.trie, &current_root_hash).build();
                let storage_iter = storage_trie.iter().map_err(|err| MptError::from(*err))?;
                let mut key_set: HashSet<Vec<u8>> = HashSet::new();
                for item in storage_iter {
                    let (key, value) = item.map_err(|err| MptError::from(*err))?;
                    let storage_key: Vec<u8> = TrieKey::<V>::drop_visibility_type(&key);
                    data_map.insert(storage_key, value);
                    key_set.insert(key);
                }
                // batch delete
                self.trie.batch_remove(&key_set)?;
                // after delete all <key, value> pair, destroy the empty trie
                self.trie.deinit()?;
                let mpt_changes = self.trie.close();
                Ok(DestroyStorageChanges {
                    inserts: mpt_changes.0,
                    deletes: mpt_changes.1,
                    data_map,
                })
            }
            Version::V2 => {
                let current_root_hash = self.trie.root_hash();
                let storage_trie =
                    TrieDBBuilder::<ExtensionLayout>::new(&self.trie, &current_root_hash).build();
                let storage_iter = storage_trie.iter().map_err(|err| MptError::from(*err))?;
                let mut key_set: HashSet<Vec<u8>> = HashSet::new();
                for item in storage_iter {
                    let (key, value) = item.map_err(|err| MptError::from(*err))?;
                    let storage_key: Vec<u8> = TrieKey::<V>::drop_visibility_type(&key);
                    data_map.insert(storage_key, value);
                    key_set.insert(key);
                }
                // batch delete
                self.trie.batch_remove(&key_set)?;
                // after delete all <key, value> pair, destroy the empty trie
                self.trie.deinit()?;
                let mpt_changes = self.trie.close();
                Ok(DestroyStorageChanges {
                    inserts: mpt_changes.0,
                    deletes: mpt_changes.1,
                    data_map,
                })
            }
        }
    }
}
