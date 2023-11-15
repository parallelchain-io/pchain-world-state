/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! This mod only public to crate inside. Provides structs and implementations

use crate::db::{KeyInstrumentedDB, DB};
use crate::error::MptError;
use crate::trie_key::PrefixedTrieNodeKey;
use crate::version::{Version, VersionProvider};
use hash_db::{AsHashDB, HashDB, HashDBRef, Hasher as KeyHasher, Prefix};
use pchain_types::cryptography::Sha256Hash;
use reference_trie::{ExtensionLayout, NoExtensionLayout, RefHasher};
use std::collections::{HashMap, HashSet};
use trie_db::proof::generate_proof;
use trie_db::{Trie, TrieDBBuilder, TrieDBMutBuilder, TrieMut};

type Hash256 = [u8; 32];

pub type Proof = Vec<Vec<u8>>;
const NULL_NODE_KEY: &[u8] = &[0_u8];
const NULL_NODE_VALUE: &[u8] = &[0_u8];

/// `MptChanges` is a wrapper of changes in [Mpt] when call function close()
#[derive(Debug, Clone)]
pub(crate) struct MptChanges(
    pub(crate) HashMap<Vec<u8>, Vec<u8>>,
    pub(crate) HashSet<Vec<u8>>,
    pub(crate) Sha256Hash,
);

/// `Mpt` is struct to maintain the Merkle Patricia Trie tree as storage struct
///
/// Merkle Tree: A hash tree in which each node’s hash is computed from its child nodes hashes.
///
/// Patricia Trie: A efficient Radix Trie (r=16), a data structure in which “keys” represent the path one has to take to reach a node
#[derive(Debug, Clone)]
pub(crate) struct Mpt<'a, S, V>
where
    S: DB + Send + Sync + Clone,
    V: VersionProvider + Send + Sync + Clone,
{
    storage: KeyInstrumentedDB<'a, S, V>,
    root_hash: Sha256Hash,
}

impl<'a, S: DB + Send + Sync + Clone, V: VersionProvider + Send + Sync + Clone> Mpt<'a, S, V> {
    /// `new` is to create a new Trie with empty state_hash
    /// This function should be called only once, during the first startup of fullnode
    pub(crate) fn new(mut storage: KeyInstrumentedDB<'a, S, V>) -> Self {
        // build an empty MPT for genesis world state
        let mut default_root_hash = Default::default();
        let key: Vec<u8> = RefHasher::hash(NULL_NODE_KEY).to_vec();
        let value: Vec<u8> = NULL_NODE_VALUE.to_vec();
        storage.put(key, value);
        let mut genesis_mpt: Mpt<S, V> = Mpt {
            storage: storage.clone(),
            root_hash: default_root_hash,
        };
        let root_hash = match <V>::version() {
            Version::V1 => {
                let mut trie = TrieDBMutBuilder::<NoExtensionLayout>::new(
                    &mut genesis_mpt,
                    &mut default_root_hash,
                )
                .build();
                trie.commit();
                *trie.root()
            }

            Version::V2 => {
                let mut trie = TrieDBMutBuilder::<ExtensionLayout>::new(
                    &mut genesis_mpt,
                    &mut default_root_hash,
                )
                .build();
                trie.commit();
                *trie.root()
            }
        };
        Mpt { storage, root_hash }
    }

    /// `deinit` is to remove the root hash by add the root hash into KeyInstrumentedDB.deletes when the trie is empty
    pub(crate) fn deinit(&mut self) -> Result<(), MptError> {
        let empty_root_hash = RefHasher::hash(NULL_NODE_KEY).to_vec();
        // check if root_hash is equal to empty root hash
        if self.root_hash() != RefHasher::hash(NULL_NODE_KEY) {
            return Err(MptError::InvalidStateRoot);
        }
        match <V>::version() {
            Version::V1 => {
                // try to open the trie with the current root hash
                let trie = TrieDBBuilder::<NoExtensionLayout>::new(self, &self.root_hash).build();
                if trie.iter().is_ok() {
                    // need to hard delete the root node
                    self.storage.delete(empty_root_hash);
                }
                // else the root hash has already been deleted
            }
            Version::V2 => {
                // try to open the trie with the current root hash
                let trie = TrieDBBuilder::<ExtensionLayout>::new(self, &self.root_hash).build();
                if trie.iter().is_ok() {
                    // need to hard delete the root node
                    self.storage.delete(empty_root_hash);
                }
                // else the root hash has already been deleted
            }
        }
        Ok(())
    }

    /// `open` is to open the trie from give storage source and state_hash
    pub(crate) fn open(storage: KeyInstrumentedDB<'a, S, V>, root_hash: Sha256Hash) -> Self {
        let mpt: Mpt<S, V> = Mpt { storage, root_hash };
        mpt
    }

    /// `get` is read and returns the value by key in a trie
    ///
    /// Return None if the key is not set before
    ///
    /// Error when state_hash does not exist or missed some trie nodes
    pub(crate) fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, MptError> {
        match <V>::version() {
            Version::V1 => {
                let trie = TrieDBBuilder::<NoExtensionLayout>::new(self, &self.root_hash).build();
                let value = trie.get(key).map_err(|err| MptError::from(*err))?;
                Ok(value)
            }
            Version::V2 => {
                let trie = TrieDBBuilder::<ExtensionLayout>::new(self, &self.root_hash).build();
                let value = trie.get(key).map_err(|err| MptError::from(*err))?;
                Ok(value)
            }
        }
    }

    /// `root_hash` return the current root_hash of trie
    pub(crate) fn root_hash(&self) -> Sha256Hash {
        self.root_hash
    }

    /// `get_with_proof` is read and returns the proof and value by key in a trie.
    ///
    /// If key exists, generates a compat proof for key-value pair in a trie.
    /// The proof contains information so that the verifier can reconstruct the subset of nodes in the trie required to lookup the key.
    /// The trie nodes are listed in pre-order traversal order with some values and internal hashes omitted.
    ///
    /// Error when state_hash does not exist or missed some trie nodes
    pub(crate) fn get_with_proof(
        &self,
        key: &Vec<u8>,
    ) -> Result<(Proof, Option<Vec<u8>>), MptError> {
        match <V>::version() {
            Version::V1 => {
                let trie = TrieDBBuilder::<NoExtensionLayout>::new(self, &self.root_hash).build();
                let value = trie.get(key).map_err(|err| MptError::from(*err))?;
                let proof_ret = generate_proof::<_, NoExtensionLayout, _, _>(
                    self,
                    &self.root_hash,
                    vec![key].iter(),
                );
                let proof = proof_ret.map_err(|err| MptError::from(*err))?;
                Ok((proof, value))
            }
            Version::V2 => {
                let trie = TrieDBBuilder::<ExtensionLayout>::new(self, &self.root_hash).build();
                let value = trie.get(key).map_err(|err| MptError::from(*err))?;
                let proof_ret = generate_proof::<_, ExtensionLayout, _, _>(
                    self,
                    &self.root_hash,
                    vec![key].iter(),
                );
                let proof = proof_ret.map_err(|err| MptError::from(*err))?;
                Ok((proof, value))
            }
        }
    }

    /// `contains` check is the key exists in a trie
    ///
    /// Error when state_hash does not exist or missed some trie nodes
    pub(crate) fn contains(&self, key: &[u8]) -> Result<bool, MptError> {
        match <V>::version() {
            Version::V1 => {
                let trie = TrieDBBuilder::<NoExtensionLayout>::new(self, &self.root_hash).build();
                let exsits = trie.contains(key).map_err(|err| MptError::from(*err))?;
                Ok(exsits)
            }
            Version::V2 => {
                let trie = TrieDBBuilder::<ExtensionLayout>::new(self, &self.root_hash).build();
                let exsits = trie.contains(key).map_err(|err| MptError::from(*err))?;
                Ok(exsits)
            }
        }
    }

    /// `iterate_all` in DFS approach to iterate all key-value pairs in MPT by a function. The iteration may
    /// end earlier if it fails to obtain key-value from the trie (e.g. state_hash does
    /// not exist or missed some trie nodes), or the function returns error.
    pub(crate) fn iterate_all<F, E>(&self, mut f: F) -> Result<(), E>
    where
        F: FnMut(Vec<u8>, Vec<u8>) -> Result<(), E>,
        E: From<MptError>,
    {
        match <V>::version() {
            Version::V1 => {
                let trie = TrieDBBuilder::<NoExtensionLayout>::new(self, &self.root_hash).build();
                let trie_iter = trie.iter().map_err(|err| MptError::from(*err))?;
                for item in trie_iter {
                    let (key, value) = item.map_err(|err| MptError::from(*err))?;
                    f(key, value)?;
                }
            }
            Version::V2 => {
                let trie = TrieDBBuilder::<ExtensionLayout>::new(self, &self.root_hash).build();
                let trie_iter = trie.iter().map_err(|err| MptError::from(*err))?;
                for item in trie_iter {
                    let (key, value) = item.map_err(|err| MptError::from(*err))?;
                    f(key, value)?;
                }
            }
        }
        Ok(())
    }

    /// `set` is set <key, value> pair to Trie
    /// Any value change will be reflected on `state_hash` change in Worldstate
    ///
    /// Error when state_hash does not exist or missed some trie nodes
    pub(crate) fn set(&mut self, key: &[u8], value: Vec<u8>) -> Result<(), MptError> {
        let mut cur_root_hash = self.root_hash;
        let new_root_hash = {
            match <V>::version() {
                Version::V1 => {
                    let mut trie = TrieDBMutBuilder::<NoExtensionLayout>::from_existing(
                        self,
                        &mut cur_root_hash,
                    )
                    .build();
                    let _ = trie
                        .insert(key, &value)
                        .map_err(|err| MptError::from(*err))?;
                    trie.commit();
                    *trie.root()
                }
                Version::V2 => {
                    let mut trie = TrieDBMutBuilder::<ExtensionLayout>::from_existing(
                        self,
                        &mut cur_root_hash,
                    )
                    .build();
                    let _ = trie
                        .insert(key, &value)
                        .map_err(|err| MptError::from(*err))?;
                    trie.commit();
                    *trie.root()
                }
            }
        };

        self.root_hash = new_root_hash;
        Ok(())
    }

    /// `batch_set` is batch set <key, value> pairs into Trie
    /// Any value change will be reflected on `state_hash` change in WorldState
    ///
    /// Error when state_hash does not exist or missed some trie nodes
    pub(crate) fn batch_set(&mut self, data: &HashMap<Vec<u8>, Vec<u8>>) -> Result<(), MptError> {
        let mut cur_root_hash = self.root_hash;
        let new_root_hash = {
            match <V>::version() {
                Version::V1 => {
                    let mut trie = TrieDBMutBuilder::<NoExtensionLayout>::from_existing(
                        self,
                        &mut cur_root_hash,
                    )
                    .build();
                    for (key, value) in data.iter() {
                        let _ = trie
                            .insert(key, value)
                            .map_err(|err| MptError::from(*err))?;
                    }
                    trie.commit();
                    *trie.root()
                }
                Version::V2 => {
                    let mut trie = TrieDBMutBuilder::<ExtensionLayout>::from_existing(
                        self,
                        &mut cur_root_hash,
                    )
                    .build();
                    for (key, value) in data.iter() {
                        let _ = trie
                            .insert(key, value)
                            .map_err(|err| MptError::from(*err))?;
                    }
                    trie.commit();
                    *trie.root()
                }
            }
        };
        self.root_hash = new_root_hash;
        Ok(())
    }

    /// `remove` remove <key, value> pair in Trie by input key
    /// Any drop will be reflected on `state_hash` change in Worldstate
    ///
    /// Error when state_hash does not exist or missed some trie nodes
    pub(crate) fn remove(&mut self, key: &[u8]) -> Result<(), MptError> {
        if !self.contains(key)? {
            return Ok(());
        }
        let mut cur_root_hash = self.root_hash;
        let new_root_hash = {
            match <V>::version() {
                Version::V1 => {
                    let mut trie = TrieDBMutBuilder::<NoExtensionLayout>::from_existing(
                        self,
                        &mut cur_root_hash,
                    )
                    .build();
                    let _ = trie.remove(key).map_err(|err| MptError::from(*err))?;
                    trie.commit();
                    *trie.root()
                }
                Version::V2 => {
                    let mut trie = TrieDBMutBuilder::<ExtensionLayout>::from_existing(
                        self,
                        &mut cur_root_hash,
                    )
                    .build();
                    let _ = trie.remove(key).map_err(|err| MptError::from(*err))?;
                    trie.commit();
                    *trie.root()
                }
            }
        };
        self.root_hash = new_root_hash;
        Ok(())
    }

    /// `batch_remove` batch remove <key, value> pairs in Trie by input key_set
    /// Any drop will be reflected on `state_hash` change in Worldstate
    ///
    /// Error when state_hash does not exist or missed some trie nodes
    pub(crate) fn batch_remove(&mut self, key_set: &HashSet<Vec<u8>>) -> Result<(), MptError> {
        let mut cur_root_hash = self.root_hash;
        let new_root_hash = {
            match <V>::version() {
                Version::V1 => {
                    let mut trie = TrieDBMutBuilder::<NoExtensionLayout>::from_existing(
                        self,
                        &mut cur_root_hash,
                    )
                    .build();
                    for key in key_set.iter() {
                        let _ = trie.remove(key).map_err(|err| MptError::from(*err));
                    }
                    trie.commit();
                    *trie.root()
                }
                Version::V2 => {
                    let mut trie = TrieDBMutBuilder::<ExtensionLayout>::from_existing(
                        self,
                        &mut cur_root_hash,
                    )
                    .build();
                    for key in key_set.iter() {
                        let _ = trie.remove(key).map_err(|err| MptError::from(*err));
                    }
                    trie.commit();
                    *trie.root()
                }
            }
        };
        self.root_hash = new_root_hash;
        Ok(())
    }

    /// `close` is return and flush cache changes in [DB](crate::db::DB). Also return the updated state_hash
    pub(crate) fn close(&mut self) -> MptChanges {
        let db_changes = self.storage.close();
        MptChanges(db_changes.0, db_changes.1, self.root_hash)
    }
}

impl<'a, S: DB + Send + Sync + Clone> Mpt<'a, S, crate::V1> {
    pub(crate) fn upgrade(self) -> Mpt<'a, S, crate::V2> {
        // upgrade
        let mut new_storage: KeyInstrumentedDB<'a, S, crate::V2> = self.storage.upgrade();
        // compute root_hash for a empty trie in V2
        let mut default_root_hash = Default::default();
        let key: Vec<u8> = RefHasher::hash(NULL_NODE_KEY).to_vec();
        let value: Vec<u8> = NULL_NODE_VALUE.to_vec();
        new_storage.put(key, value);
        let mut genesis_mpt: Mpt<S, crate::V2> = Mpt {
            storage: new_storage.clone(),
            root_hash: default_root_hash,
        };
        let new_root_hash = {
            let mut trie =
                TrieDBMutBuilder::<ExtensionLayout>::new(&mut genesis_mpt, &mut default_root_hash)
                    .build();
            trie.commit();
            *trie.root()
        };
        Mpt {
            storage: new_storage,
            root_hash: new_root_hash,
        }
    }
}

/// Implemented to meet a requirement of Parity's Base-16 Modified Merkle Tree ("Trie")
///
/// Database must implement this HashDB trait to create mutable trie.
///
/// For example https://github.com/paritytech/trie/blob/master/memory-db/src/lib.rs
impl<'a, S: DB + Send + Sync + Clone, V: VersionProvider + Send + Sync + Clone>
    HashDB<RefHasher, Vec<u8>> for Mpt<'a, S, V>
{
    /// Look up a given hash into the bytes that hash to it, returning None if the hash is not known.
    fn get(&self, key: &Hash256, mpt_prefix: Prefix) -> Option<Vec<u8>> {
        let key = PrefixedTrieNodeKey::<RefHasher>::key(key, mpt_prefix);
        self.storage.get(&key)
    }

    /// Check for the existence of a hash-key.
    fn contains(&self, key: &Hash256, mpt_prefix: Prefix) -> bool {
        let key = PrefixedTrieNodeKey::<RefHasher>::key(key, mpt_prefix);
        self.storage.get(key.as_ref()).is_some()
    }

    /// Insert item into the DB and return the hash for a later lookup.
    fn insert(&mut self, mpt_prefix: Prefix, value: &[u8]) -> Hash256 {
        let key = RefHasher::hash(value);
        self.emplace(key, mpt_prefix, value.to_vec());
        key
    }

    /// Like insert(), except you provide the key and the data is all moved.
    fn emplace(&mut self, key: Hash256, mpt_prefix: Prefix, value: Vec<u8>) {
        let key = PrefixedTrieNodeKey::<RefHasher>::key(&key, mpt_prefix);
        self.storage.put(key, value);
    }

    /// Remove an item previously inserted.
    fn remove(&mut self, key: &Hash256, mpt_prefix: Prefix) {
        if key[..] == RefHasher::hash(NULL_NODE_KEY) {
            return;
        }
        let key = PrefixedTrieNodeKey::<RefHasher>::key(key, mpt_prefix);
        self.storage.delete(key);
    }
}

/// Implemented to meet a requirement of Parity's Base-16 Modified Merkle Tree ("Trie").
///
/// Database must implement this HashDBRef trait to create immutable trie for querying and merkle proof.
///
/// For example https://github.com/paritytech/trie/blob/master/memory-db/src/lib.rs
impl<'a, S: DB + Send + Sync + Clone, V: VersionProvider + Send + Sync + Clone>
    AsHashDB<RefHasher, Vec<u8>> for Mpt<'a, S, V>
{
    fn as_hash_db(&self) -> &dyn HashDB<RefHasher, Vec<u8>> {
        self
    }

    fn as_hash_db_mut<'b>(&'b mut self) -> &'b mut (dyn HashDB<RefHasher, Vec<u8>> + 'b) {
        &mut *self
    }
}

/// Implemented to meet a requirement of Parity's Base-16 Modified Merkle Tree ("Trie").
///
/// Database must implement this HashDBRef trait to create immutable trie for querying and merkle proof.
///
/// For example https://github.com/paritytech/trie/blob/master/memory-db/src/lib.rs
impl<'a, S: DB + Send + Sync + Clone, V: VersionProvider + Send + Sync + Clone>
    HashDBRef<RefHasher, Vec<u8>> for Mpt<'a, S, V>
{
    fn get(&self, key: &Hash256, mpt_prefix: Prefix) -> Option<Vec<u8>> {
        HashDB::get(self, key, mpt_prefix)
    }

    fn contains(&self, key: &Hash256, mpt_prefix: Prefix) -> bool {
        HashDB::contains(self, key, mpt_prefix)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::accounts_trie::AccountField;
    use crate::trie_key::TrieKey;
    use crate::version::{V1, V2};
    use pchain_types::cryptography::PublicAddress;

    #[derive(Debug, Clone)]
    struct DummyStorage(HashMap<Vec<u8>, Vec<u8>>);
    impl DB for DummyStorage {
        fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
            match self.0.get(key) {
                Some(value) => Some(value.to_owned()),
                None => None,
            }
        }
    }
    impl DummyStorage {
        fn apply_changes(&mut self, inserts: HashMap<Vec<u8>, Vec<u8>>, deletes: HashSet<Vec<u8>>) {
            for (key, value) in inserts.into_iter() {
                self.0.insert(key, value);
            }
            for key in deletes.into_iter() {
                self.0.remove(&key);
            }
        }
    }

    #[derive(Debug, Clone)]
    struct TestEnv {
        db: DummyStorage,
        address: PublicAddress,
    }
    impl Default for TestEnv {
        fn default() -> Self {
            let db = DummyStorage(HashMap::new());
            const PUBLIC_KEY: &str = "ipy_VXNiwHNP9mx6-nKxht_ZJNfYoMAcCnLykpq4x_k";
            let address = base64url::decode(PUBLIC_KEY).unwrap().try_into().unwrap();
            Self { db, address }
        }
    }
    #[test]
    pub fn init_and_open() {
        let env = TestEnv::default();
        let db = KeyInstrumentedDB::<DummyStorage, V1>::new(&env.db, env.address.to_vec());
        let ret = Mpt::<DummyStorage, V1>::new(db.clone());
        let key = TrieKey::<V1>::account_key(&env.address, AccountField::Nonce);
        assert_eq!(ret.get(&key).unwrap(), None);
    }

    #[test]
    pub fn init_and_add_null_key() {
        let mut env = TestEnv::default();
        let db = KeyInstrumentedDB::<DummyStorage, V1>::new(&env.db, env.address.to_vec());
        let mut ret = Mpt::<DummyStorage, V1>::new(db);
        let mpt_change = ret.close();
        println!("inserts{:?}", &mpt_change.0);
        println!("deletes{:?}", &mpt_change.1);
        println!("root_hash{:?}", &mpt_change.2);
        env.db.apply_changes(mpt_change.0, mpt_change.1);
        let db = KeyInstrumentedDB::<DummyStorage, V1>::new(&env.db, env.address.to_vec());
        let mut ret = Mpt::<DummyStorage, V1>::open(db, mpt_change.2);
        ret.set(
            &RefHasher::hash(NULL_NODE_KEY),
            RefHasher::hash(NULL_NODE_VALUE).to_vec(),
        )
        .unwrap();
        let mpt_change = ret.close();
        println!("inserts{:?}", &mpt_change.0);
        println!("deletes{:?}", &mpt_change.1);
        println!("root_hash{:?}", &mpt_change.2);
        env.db.apply_changes(mpt_change.0, mpt_change.1);
        let db = KeyInstrumentedDB::<DummyStorage, V1>::new(&env.db, env.address.to_vec());
        let ret = Mpt::<DummyStorage, V1>::open(db, mpt_change.2);
        ret.get(&RefHasher::hash(NULL_NODE_KEY)).unwrap();
    }

    #[test]
    pub fn test() {
        println!("{:?}", RefHasher::hash(NULL_NODE_KEY));
        println!("{:?}", RefHasher::hash(NULL_NODE_VALUE));
    }

    #[test]
    pub fn init_add_delete_and_open() {
        // init the trie
        let mut env = TestEnv::default();
        let db = KeyInstrumentedDB::<DummyStorage, V2>::new(&env.db, env.address.to_vec());
        let mut ret = Mpt::<DummyStorage, V2>::new(db.clone());
        let changes = ret.close();
        env.db.apply_changes(changes.0, changes.1);
        println!("{:?}", env.address.to_vec());
        // insert 2 pair of key, values into the trie
        let db = KeyInstrumentedDB::<DummyStorage, V2>::new(&env.db, env.address.to_vec());
        println!("root_hash after init, {:?}", changes.2);
        let mut ret = Mpt::<DummyStorage, V2>::open(db, changes.2);
        let data_key = b"apple".to_vec();
        let data_value = b"apple_12345".to_vec();
        let data_key_b = b"banana".to_vec();
        let data_value_b = b"banana_12345".to_vec();
        println!("{:?}", data_key);
        println!("{:?}", data_value);
        println!("{:?}", data_key_b);
        println!("{:?}", data_value_b);
        assert_eq!(ret.get(&data_key).unwrap(), None);

        ret.set(&data_key, data_value.clone()).unwrap();
        ret.set(&data_key_b, data_value_b.clone()).unwrap();
        let changes = ret.close();
        println!("+++++++++=+++++++++++++++++ MPT after inserts +++++++++++++++");
        println!(
            "MPT inserts: {:?}, MPT deletes: {:?}",
            changes.clone().0,
            changes.clone().1
        );
        env.db.apply_changes(changes.0, changes.1);

        // open the trie after insertion and test if can find the first <key, value> pair in the existing trie
        let db = KeyInstrumentedDB::<DummyStorage, V2>::new(&env.db, env.address.to_vec());
        println!("root_hash after insert, {:?}", changes.2);
        let mut ret = Mpt::<DummyStorage, V2>::open(db, changes.2);
        assert!(ret.contains(&data_key).unwrap());
        assert!(ret.contains(&data_key_b).unwrap());
        assert_eq!(ret.get(&data_key).unwrap().unwrap(), data_value);

        // remove the 2 pair of <key value>
        ret.remove(&data_key).unwrap();
        ret.remove(&data_key_b).unwrap();
        let changes = ret.close();
        println!("+++++++++=+++++++++++++++++ MPT after delete +++++++++++++++");
        println!(
            "MPT inserts: {:?}, MPT deletes: {:?}",
            changes.clone().0,
            changes.clone().1
        );
        env.db.apply_changes(changes.0, changes.1);

        // open the trie after deletion, and print out the root_hash, and try to test if can find the second <key, value> pair in the trie (which should return None)
        let db = KeyInstrumentedDB::<DummyStorage, V2>::new(&env.db, env.address.to_vec());
        println!("root_hash after delete, {:?}", changes.2);
        let ret = Mpt::<DummyStorage, V2>::open(db, changes.2);
        // assert_eq!(ret.get(&data_key_b).unwrap().unwrap(), data_value_b);
        assert_eq!(ret.get(&data_key_b).unwrap(), None);
    }

    #[test]
    fn delete_root() {
        let mut env = TestEnv::default();
        // do insert
        let db = KeyInstrumentedDB::<DummyStorage, V1>::new(&env.db, env.address.to_vec());
        let mut ret = Mpt::<DummyStorage, V1>::new(db);
        let apple_key = b"apple".to_vec();
        let apple_value = b"apple_12345".to_vec();
        let banana_key = b"banana".to_vec();
        let banana_value = b"banana_12345".to_vec();
        ret.set(&apple_key, apple_value.clone()).unwrap();
        ret.set(&banana_key, banana_value.clone()).unwrap();
        let ws_changes = ret.close();
        env.db.apply_changes(ws_changes.0, ws_changes.1);
        println!("==================== db after insert ==================");
        println!("{:?}", &env.db);
        // do delete
        let db = KeyInstrumentedDB::<DummyStorage, V1>::new(&env.db, env.address.to_vec());
        let mut ret = Mpt::<DummyStorage, V1>::open(db, ws_changes.2);
        ret.remove(&apple_key).unwrap();
        ret.remove(&banana_key).unwrap();
        let ws_changes = ret.close();
        env.db.apply_changes(ws_changes.0, ws_changes.1);
        println!("==================== db after delete ==================");
        println!("{:?}", &env.db);
        // do de_init
        let db = KeyInstrumentedDB::<DummyStorage, V1>::new(&env.db, env.address.to_vec());
        let mut ret = Mpt::<DummyStorage, V1>::open(db, ws_changes.2);
        ret.deinit().unwrap();
        let ws_changes = ret.close();
        env.db.apply_changes(ws_changes.0, ws_changes.1);
        println!("==================== db after deinit ==================");
        println!("{:?}", &env.db);
    }
}
