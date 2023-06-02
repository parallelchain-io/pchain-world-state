/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Functions to initialize/open a Merkle Patricia Trie with given storage source, and implementation of updating and
//! geting key or proof from the trie.

use hash_db::{AsHashDB, HashDB, HashDBRef, Hasher as KeyHasher, Prefix};
use pchain_types::cryptography::Sha256Hash;
use reference_trie::NoExtensionLayout;
use std::{collections::HashMap, convert::TryInto};
use trie_db::{proof::generate_proof, Trie, TrieDB, TrieDBMut, TrieMut};

use crate::{
    error::WorldStateError,
    keys::{AppKey, PrefixedTrieNodeKey, WSKey},
    storage::{KeyspacedInstrumentedDB, StorageMutations, WorldStateStorage},
};

pub type Value = Vec<u8>;
pub type Proof = Vec<Vec<u8>>;
type RefHasher = keccak_hasher::KeccakHasher;
type Hash256 = [u8; 32];

/// MPT is to read and update data stored in trie structure. It builds up `write_set` in KeyspacedInstrumentedDB that contains all of the storage mutations
/// required to atomically persist a new, correct world state into persistent storage. MPT provides `get`s methods to read trie data from a persistent storage
/// that implemented the trait storage::WorldStateStorage.
#[derive(Clone)]
pub(crate) struct Mpt<S>
where
    S: WorldStateStorage + Send + Sync + Clone,
{
    storage: KeyspacedInstrumentedDB<S>,
    primary_state_hash: Sha256Hash,
    state_hash: Sha256Hash,
}

impl<S: WorldStateStorage + Send + Sync + Clone> Mpt<S> {
    /// initialize is to create a new Trie with empty state hash. This is a tool to prepares the `write_set` that contains all
    /// storage mutations by genesis transactions. The `write_set` will be used to initialize persistent storage managed by Hotstuff.
    /// This function should be called only once, during the first startup of fullnode.
    pub(crate) fn new(mut storage: KeyspacedInstrumentedDB<S>) -> Mpt<S> {
        let (mutation, state_hash) = {
            // build and commit empty MPT for genesis world state.
            let mut root_hash = Default::default();

            let key = RefHasher::hash(&[0u8][..]);
            storage.put(key.to_vec(), [0u8].to_vec());

            let mut genesis: Mpt<S> = Mpt {
                storage: storage.clone(),
                primary_state_hash: root_hash,
                state_hash: root_hash,
            };
            let root = {
                let mut trie = TrieDBMut::<NoExtensionLayout>::new(&mut genesis, &mut root_hash);
                trie.commit();
                *trie.root()
            };

            genesis.state_hash = root;
            genesis.close()
        };
        // merge uncommitted changes to world state storage
        storage.merge(mutation);

        Mpt {
            storage,
            primary_state_hash: state_hash,
            state_hash,
        }
    }

    /// `open` is to open the world state from given storage source and state_hash.
    /// Returns an error if root_hash does not exist or unable to open the trie.
    pub(crate) fn open(
        storage: KeyspacedInstrumentedDB<S>,
        mut root_hash: Sha256Hash,
    ) -> Result<Self, WorldStateError> {
        let mut mpt = Mpt {
            storage,
            primary_state_hash: root_hash,
            state_hash: root_hash,
        };

        let _ = match TrieDBMut::<NoExtensionLayout>::from_existing(&mut mpt, &mut root_hash) {
            Ok(t) => t,
            Err(err) => return Err(WorldStateError::from(*err)),
        };

        let _ = match TrieDB::<NoExtensionLayout>::new(&mpt, &root_hash) {
            Ok(t) => t,
            Err(err) => return Err(WorldStateError::from(*err)),
        };

        Ok(mpt)
    }

    /// Get backing storage including uncommitted changes of MPT
    pub(crate) fn db(&self) -> &KeyspacedInstrumentedDB<S> {
        &self.storage
    }

    /// loop over the trie with depth-first iterator and return all AppKey-value pairs
    pub(crate) fn get_all_elements(&self) -> HashMap<AppKey, Value> {
        let trie = TrieDB::<NoExtensionLayout>::new(self, &self.state_hash)
            .expect("Fail to open worldstate trie");
        let mut kvs = HashMap::new();
        for item in trie.iter().unwrap() {
            let (key, value) = item.unwrap();
            kvs.insert(WSKey::Public(key).try_into().unwrap(), value);
        }
        kvs
    }

    /// Reads and returns the value in a trie
    /// Returns none if the key is not set before.
    /// This method would panic if the storage were corrupted and missed some trie nodes.
    pub(crate) fn get_key(&self, key: &WSKey) -> Option<Value> {
        let trie = TrieDB::<NoExtensionLayout>::new(self, &self.state_hash)
            .expect("Fail to open worldstate trie");
        trie.get(key.as_ref())
            .unwrap_or_else(|e| panic!("Data corruption. {}", e))
    }

    /// Return proof and value of the key, if it existed. This function generates a compact proof for key-value pair in a trie given key
    /// The proof contains information so that the verifier can reconstruct the subset of nodes in the trie required to lookup the key.
    /// The trie nodes are listed in pre-order traversal order with some values and internal hashes omitted.
    /// This method would panic if the storage were corrupted and missed some trie nodes.
    pub(crate) fn get_key_with_proof(&self, key: &WSKey) -> (Proof, Option<Value>) {
        let trie = TrieDB::<NoExtensionLayout>::new(self, &self.state_hash)
            .expect("Fail to open worldstate trie");

        let proof = generate_proof::<_, NoExtensionLayout, _, _>(&trie, vec![key].iter())
            .expect("Fail to open worldstate trie");
        let value = trie
            .get(key.as_ref())
            .unwrap_or_else(|e| panic!("Data corruption. {}", e));

        (proof, value)
    }

    /// Check if the key exists in a trie
    pub(crate) fn contains_key(&self, key: &WSKey) -> bool {
        let trie = TrieDB::<NoExtensionLayout>::new(self, &self.state_hash)
            .expect("Fail to open worldstate trie");
        trie.contains(key.as_ref()) == Ok(true)
    }

    /// Set WSKey and value.
    /// Any value change will be reflected on `state_hash` change in WorldState.
    pub(crate) fn set_value(&mut self, key: WSKey, value: Value) {
        let mut root_hash = self.state_hash;
        let new_root = {
            // Apply changes to world state trie
            let mut trie = TrieDBMut::<NoExtensionLayout>::from_existing(self, &mut root_hash)
                .expect("Fail to open worldstate trie");
            trie.insert(key.as_ref(), &value)
                .expect("fail to insert to trie.");

            // Commit the in-memory changes to disk and update the state root.
            trie.commit();
            *trie.root()
        };
        self.state_hash = new_root;
    }

    /// Set a batch of WSKey and value pairs.
    /// Any value change will be reflected on `state_hash` change in WorldState.
    pub(crate) fn set_values(&mut self, values: &HashMap<WSKey, Value>) {
        let mut root_hash = self.state_hash;
        let new_root = {
            // Apply changes to world state trie
            let mut trie = TrieDBMut::<NoExtensionLayout>::from_existing(self, &mut root_hash)
                .expect("Fail to open worldstate trie");
            for (key, value) in values {
                trie.insert(key.as_ref(), value)
                    .expect("fail to insert to trie.");
            }

            // Commit the in-memory changes to disk and update the state root.
            trie.commit();
            *trie.root()
        };
        self.state_hash = new_root;
    }

    /// Insert changes to storage.mutations. Instead of inserting key-value pair to update trie node, this method provides direct way to
    /// attach extra data to storage. This is useful when you want to save data with some prepended prefixes.
    pub(crate) fn merge_mutations(&mut self, mutations: StorageMutations) {
        self.storage.merge(mutations);
    }

    /// Consume MPT and returns the cached changes since state creation.
    pub(crate) fn close(self) -> (StorageMutations, Sha256Hash) {
        (self.storage.commit(), self.state_hash)
    }

    /// Flush all cached changes
    pub(crate) fn flush(&mut self) {
        self.storage.clear_cache();
        self.state_hash = self.primary_state_hash;
    }
}

/// Implemented to meet a requirement of Parity's Base-16 Modified Merkle Tree ("Trie")
/// Database must implement this HashDB trait to create mutable trie.
/// Read more: https://docs.rs/trie-db/latest/trie_db/triedbmut/struct.TrieDBMut.html#method.new
impl<S: WorldStateStorage + Send + Sync + Clone> HashDB<RefHasher, Value> for Mpt<S> {
    // Look up a given hash into the bytes that hash to it, returning None if the hash is not known.
    fn get(&self, key: &Hash256, mpt_prefix: Prefix) -> Option<Value> {
        let key = PrefixedTrieNodeKey::<RefHasher>::key(key, mpt_prefix);
        self.storage.get(&key)
    }

    // Check for the existence of a hash-key.
    fn contains(&self, key: &Hash256, mpt_prefix: Prefix) -> bool {
        let key: [u8; 32] = PrefixedTrieNodeKey::<RefHasher>::key(key, mpt_prefix)
            .try_into()
            .unwrap();
        HashDB::get(self, &key, mpt_prefix).is_some()
    }

    // Insert item into the DB and return the hash for a later lookup.
    fn insert(&mut self, mpt_prefix: Prefix, value: &[u8]) -> Hash256 {
        let key = RefHasher::hash(value);
        self.emplace(key, mpt_prefix, value.to_vec());
        key
    }

    // Like insert(), except you provide the key and the data is all moved.
    fn emplace(&mut self, key: Hash256, mpt_prefix: Prefix, value: Value) {
        let key = PrefixedTrieNodeKey::<RefHasher>::key(&key, mpt_prefix);
        self.storage.put(key, value);
    }

    // Remove an item previously inserted.
    fn remove(&mut self, key: &Hash256, mpt_prefix: Prefix) {
        let key = PrefixedTrieNodeKey::<RefHasher>::key(key, mpt_prefix);
        self.storage.delete(key);
    }
}

/// Implemented to meet a requirement of Parity's Base-16 Modified Merkle Tree ("Trie").
/// Database must implement this HashDBRef trait to create immutable trie for querying and merkle proof.
/// Read more: https://docs.rs/hash-db/latest/hash_db/trait.AsHashDB.html
impl<S: WorldStateStorage + Send + Sync + Clone> AsHashDB<RefHasher, Value> for Mpt<S> {
    fn as_hash_db(&self) -> &dyn HashDB<RefHasher, Value> {
        self
    }

    fn as_hash_db_mut<'b>(&'b mut self) -> &'b mut (dyn HashDB<RefHasher, Value> + 'b) {
        &mut *self
    }
}

/// Implemented to meet a requirement of Parity's Base-16 Modified Merkle Tree ("Trie").
/// Database must implement this HashDBRef trait to create immutable trie for querying and merkle proof.
/// Read more: https://docs.rs/trie-db/latest/trie_db/triedb/struct.TrieDB.html#method.new
impl<S: WorldStateStorage + Send + Sync + Clone> HashDBRef<RefHasher, Value> for Mpt<S> {
    fn get(&self, key: &Hash256, mpt_prefix: Prefix) -> Option<Value> {
        HashDB::get(self, key, mpt_prefix)
    }

    fn contains(&self, key: &Hash256, mpt_prefix: Prefix) -> bool {
        HashDB::contains(self, key, mpt_prefix)
    }
}
