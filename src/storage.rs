/*
    Copyright © 2023, ParallelChain Lab 
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Definition of trait for accessing persistent storage from world state, and data structure of the world state changes.

use std::collections::{HashMap, HashSet};
use pchain_types::cryptography::{PublicAddress, Sha256Hash};

use crate::keys::WSKey;

/// Key is the key interfacing between WorldState and the persistent storage.
pub type Key = Vec<u8>;
/// Value is the value interfacing between WorldState and the persistent storage.
pub type Value = Vec<u8>;

/// Trie Level indicates the data layer that World State partitions on a Trie.
#[derive(Clone)]
pub(crate) enum TrieLevel {
    /// The base Level of the world state data.
    WorldState,
    /// The level descented from [TrieLevel::WorldState] that represents a Storage Trie (MPT) associated with Contract Account. 
    Storage(Sha256Hash)
}

/// WorldStateStorage defines the methods that a type must implemented to be used as a persistent storage inside WorldState.
/// The method `get` must be implemented in order to open the Trie.
pub trait WorldStateStorage {
    /// Reads and returns the value in storage. 
    /// Returns none if the key is not set before.
    fn get(&self, key: &Key) -> Option<Value> ;
}

/// WorldStateChanges defines the trie node changes since opening.
/// Keys in inserts and deletes are ‘actual‘ trie node keys, minus the WorldState keyspace defined in application. 
pub struct WorldStateChanges {
    pub inserts: HashMap<Vec<u8>, Vec<u8>>,
    pub deletes: HashSet<Vec<u8>>,
    pub next_state_hash: Sha256Hash,
}

/// Caches store lists of changes before insert to the trie in batch.
/// Keys in here are WSKey.
#[derive(Clone)]
pub(crate) struct Caches {
    world_state: HashMap<WSKey, Value>,
    storage: HashMap<PublicAddress, HashMap<WSKey, Value>>
}

impl Caches{
    pub(crate) fn new() -> Self {
        Caches{world_state: HashMap::new(), storage: HashMap::new()}
    }

    pub(crate) fn insert_world_state(&mut self, key: WSKey, value: Value) {
        self.world_state.insert(key, value);
    }

    pub(crate) fn insert_storage(&mut self, address: PublicAddress, key: WSKey, value: Value) {
        match self.storage.get_mut(&address){
            Some(list) => {
                list.insert(key, value);
            },
            None => {
                let mut list = HashMap::new();
                list.insert(key, value);
                self.storage.insert(address, list);
            }
        };
    }

    pub(crate) fn clear(&mut self){
        self.world_state.clear();
        self.storage.clear();
    }

    pub(crate) fn world_state(&self) -> &HashMap<WSKey, Value>{
        &self.world_state
    }

    pub(crate) fn storage(&self) -> &HashMap<PublicAddress, HashMap<WSKey, Value>>{
        &self.storage
    }
}


/// KeyspacedInstrumentedDB is a wrapper around implementations of [WorldStateStorage] that enforces that 
/// all KVs read from / written into persistent storage are properly formed KeyspacedKeys. All changes 
/// store into an in-memory write-collector instead of writing directly into persistent store.
#[derive(Clone)]
pub(crate) struct KeyspacedInstrumentedDB<S>
where S: WorldStateStorage + Send + Sync + Clone{
    storage: S,
    write_set: StorageMutations,
    keyspace: TrieLevel,
}

impl<S: WorldStateStorage + Send + Sync + Clone> KeyspacedInstrumentedDB<S>{
    pub(crate) fn open(storage: S, keyspace: TrieLevel) -> KeyspacedInstrumentedDB<S>{
        KeyspacedInstrumentedDB{ storage, write_set: StorageMutations::new(), keyspace}
    }

    pub(crate) fn set_keyspace(&mut self, keyspace: TrieLevel){
        self.keyspace = keyspace;
    }

    pub(crate) fn get(&self, key: &Key) -> Option<Value>{
        let key = match self.keyspace{
            TrieLevel::WorldState => key.to_owned(),
            TrieLevel::Storage(address) => {
                let mut keyspaced_key: Vec<u8> = address.into();
                keyspaced_key.append(&mut key.clone());
                keyspaced_key
            },
        };
        
        match self.write_set.get_insert(&key){
            Some(value) => Some(value.to_owned()),
            None => {
                if let Some(value) = self.storage.get(&key){
                    return Some(value)
                }

                None
            }
        }
    }

    pub(crate) fn put(&mut self, mut key: Key, value: Value) -> Option<Value>{
        let key = match self.keyspace{
            TrieLevel::WorldState => key,
            TrieLevel::Storage(address) => {
                let mut keyspaced_key: Vec<u8> = address.into();
                keyspaced_key.append(&mut key);
                keyspaced_key
            },
        };
        self.write_set.insert(key, value)
    }

    pub(crate) fn delete(&mut self, key: Key){
        self.write_set.delete(key);
    }

    pub(crate) fn merge(&mut self, mutation: StorageMutations){
        self.write_set.merge(mutation);
    }

    pub(crate) fn commit(self) -> StorageMutations{
        self.write_set
    }

    pub(crate) fn clear_cache(&mut self) {
        self.write_set.clear()
    }
}

#[derive(Clone)]
pub(crate) struct StorageMutations {
    inserts: HashMap<Key, Value>,
    deletes: HashSet<Key>, 
}
impl StorageMutations {
    fn new() -> StorageMutations {
        StorageMutations {
            inserts: HashMap::new(),
            deletes: HashSet::new(),
        }
    }

    fn insert(&mut self, key: Key, value: Value) -> Option<Value>{
        self.deletes.remove(&key);
        self.inserts.insert(key, value)
    }

    fn delete(&mut self, key: Key) {
        self.inserts.remove(&key);
        self.deletes.insert(key);
    }

    fn merge(&mut self, mutation: StorageMutations){
        self.inserts.extend(mutation.inserts);
        self.deletes.extend(mutation.deletes);
    }

    fn clear(&mut self){
        self.inserts.clear();
        self.deletes.clear();
    }

    fn get_insert(&self, key: &Key) -> Option<&Value> {
        self.inserts.get(key)
    } 

    pub(crate) fn consume(self) -> (HashMap<Key, Value>, HashSet<Key>) {
        (self.inserts, self.deletes)
    } 
}