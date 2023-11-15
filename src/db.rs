/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! This mod is only public inside crate except [DB]. Provides struct and implementation of database operations

use std::{
    collections::{HashMap, HashSet},
    marker::PhantomData,
};

use crate::{Version, VersionProvider, V1, V2};

/// Define the methods that a type must implemented to be used as a persistent storage inside WorldState.
/// The method `get` must be implemented in order to open the Trie.
pub trait DB {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
}

/// `KeyInstrumentedDB` is a wrapper around implementations of 'DB' that enforces
/// that all KVs read from/written into persistent storage are properly formed KeyspacedKeys.
/// All changes store into an in-memory write-collector instead of writing directly into persistent store
#[derive(Debug, Clone)]
pub(crate) struct KeyInstrumentedDB<'a, S, V>
where
    S: DB + Send + Sync + Clone,
    V: VersionProvider + Send + Sync + Clone,
{
    storage: &'a S,
    inserts: HashMap<Vec<u8>, Vec<u8>>,
    deletes: HashSet<Vec<u8>>,
    // for AccoutTrie is None, for StorageTrie is PublicAddress
    prefix: Vec<u8>,
    _type: PhantomData<V>,
}

/// `DbChanges` is a wrapper of changes in [KeyInstrumentedDB] when call function close()
#[derive(Debug, Clone)]
pub(crate) struct DbChanges(
    pub(crate) HashMap<Vec<u8>, Vec<u8>>,
    pub(crate) HashSet<Vec<u8>>,
);

impl<'a, S: DB + Send + Sync + Clone, V: VersionProvider + Send + Sync + Clone>
    KeyInstrumentedDB<'a, S, V>
{
    /// `new` is to create a new [KeyInstrumentedDB] with empty memory cache `inserts` and `deletes`
    pub(crate) fn new(storage: &S, prefix: Vec<u8>) -> KeyInstrumentedDB<S, V> {
        KeyInstrumentedDB {
            storage,
            inserts: HashMap::new(),
            deletes: HashSet::new(),
            prefix,
            _type: PhantomData,
        }
    }

    /// `get` is return value from memory cache `inserts` by input key
    pub(crate) fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let search_key = self.build_key(key);
        match self.inserts.get(&search_key) {
            Some(value) => Some(value.to_owned()),
            None => {
                if self.deletes.contains(&search_key) {
                    return None;
                }
                if let Some(value) = self.storage.get(&search_key) {
                    return Some(value);
                }
                None
            }
        }
    }

    /// `put` add input `<key, value>` into memory cache `inserts` and remove input key from memory cache `deletes`
    pub(crate) fn put(&mut self, key: Vec<u8>, value: Vec<u8>) -> Option<Vec<u8>> {
        let insert_key = self.build_key(&key);
        self.deletes.remove(&insert_key);
        self.inserts.insert(insert_key, value)
    }

    /// `delete` remove `<key, value`> from memory cache `inserts` by input key, and add the input key into memory cache `deletes`
    pub(crate) fn delete(&mut self, key: Vec<u8>) {
        let delete_key = self.build_key(&key);
        self.inserts.remove(&delete_key);
        self.deletes.insert(delete_key);
    }

    /// `close` return memory cache `inserts` and `deletes`
    pub(crate) fn close(&mut self) -> DbChanges {
        let inserts = self.inserts.clone();
        let deletes = self.deletes.clone();
        self.inserts.clear();
        self.deletes.clear();
        DbChanges(inserts, deletes)
    }

    /// `build_key` is a private function to build physical key for physical storage
    fn build_key(&self, key: &[u8]) -> Vec<u8> {
        let mut ret_key: Vec<u8> = Vec::new();
        match <V>::version() {
            Version::V1 => {
                if self.prefix.is_empty() {
                    ret_key.extend_from_slice(key);
                } else {
                    ret_key.extend_from_slice(&self.prefix);
                    ret_key.extend_from_slice(key);
                }
            }
            Version::V2 => {
                if self.prefix.is_empty() {
                    ret_key.push(0_u8);
                    ret_key.extend_from_slice(key);
                } else {
                    ret_key.push(1_u8);
                    ret_key.extend_from_slice(&self.prefix);
                    ret_key.extend_from_slice(key);
                }
            }
        }
        ret_key
    }
}

impl<'a, S: DB + Send + Sync + Clone> KeyInstrumentedDB<'a, S, V1> {
    pub(crate) fn upgrade(self) -> KeyInstrumentedDB<'a, S, V2> {
        KeyInstrumentedDB {
            storage: self.storage,
            inserts: self.inserts,
            deletes: self.deletes,
            prefix: self.prefix,
            _type: PhantomData,
        }
    }
}
