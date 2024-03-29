/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! This mod only public to crate inside. Provides structs and implementations

use crate::db::{KeyInstrumentedDB, DB};
use crate::error::MptError;
use crate::version::VersionProvider;
use hash_db::{AsHashDB, HashDB, HashDBRef, Hasher as KeyHasher, Prefix};
use pchain_types::cryptography::Sha256Hash;
use reference_trie::{NoExtensionLayout, RefHasher};
use std::collections::{HashMap, HashSet};
use trie_db::proof::generate_proof;
use trie_db::{Trie, TrieDBBuilder, TrieDBMutBuilder, TrieMut};

pub type Proof = Vec<Vec<u8>>;

/// `Mpt` is struct to maintain the Merkle Patricia Trie tree as storage struct
///
/// Merkle Tree: A hash tree in which each node’s hash is computed from its child nodes hashes.
///
/// Patricia Trie: A efficient Radix Trie (r=16), a data structure in which “keys” represent the path one has to take to reach a node
///
/// The reason that Mpt struct exposed to public is we need it in benchmark test
#[derive(Debug, Clone)]
pub struct Mpt<'a, S, V>
where
    S: DB + Send + Sync + Clone,
    V: VersionProvider + Send + Sync + Clone,
{
    db: KeyInstrumentedDB<'a, S, V>,
    root_hash: Sha256Hash,
}

const PREIMAGE_OF_EMPTY_TRIE_ROOT_HASH: &[u8] = &[0_u8];
const EMPTY_TRIE_DUMMY_ROOT_NODE: &[u8] = &[0_u8];

impl<'a, S: DB + Send + Sync + Clone, V: VersionProvider + Send + Sync + Clone> Mpt<'a, S, V> {
    /// `new` is to create a new Trie with empty state_hash
    /// This function should be called only once, during the first startup of fullnode
    pub(crate) fn new(mut db: KeyInstrumentedDB<'a, S, V>) -> Self {
        // Even when opening an empty trie, `trie_db` expects a key to store a root node. We need to add this root node manually.
        // This root node's hash (`empty_trie_root_hash`) must be the RefHasher::hash of 0u8. This is equal to the value of
        // `L:Codec::hashed_null_node()`, which is defined in `trie_db`.
        let empty_trie_root_hash: Vec<u8> =
            RefHasher::hash(PREIMAGE_OF_EMPTY_TRIE_ROOT_HASH).to_vec();
        let empty_trie_dummy_root_node: Vec<u8> = EMPTY_TRIE_DUMMY_ROOT_NODE.to_vec();
        db.put(empty_trie_root_hash, empty_trie_dummy_root_node);

        // This `dummy_root_hash` variable is passed to TrieDBMutBuilder::new as the second parameter, but its value is not used,
        // because `new` immediately overwrites its value to be `L::Codec::hashed_null_node();`. It is just here to satisfy the
        // TrieDBMutBuilder::new's interface. Its value does not matter.
        let mut dummy_root_hash = [0u8; 32];
        let mut genesis_mpt: Mpt<S, V> = Mpt {
            db: db.clone(),
            root_hash: dummy_root_hash,
        };
        let root_hash = {
            let mut trie =
                TrieDBMutBuilder::<NoExtensionLayout>::new(&mut genesis_mpt, &mut dummy_root_hash)
                    .build();
            trie.commit();
            *trie.root()
        };
        Mpt { db, root_hash }
    }

    /// `unsafe_new` is contructor of MPT for benchmark test
    pub fn unsafe_new(db: KeyInstrumentedDB<'a, S, V>) -> Self {
        Self::new(db)
    }

    /// `open` is to open the trie from give storage source and state_hash
    pub fn open(db: KeyInstrumentedDB<'a, S, V>, root_hash: Sha256Hash) -> Self {
        let mpt: Mpt<S, V> = Mpt { db, root_hash };
        mpt
    }

    /// `get` is read and returns the value by key in a trie
    ///
    /// Return None if the key is not set before
    ///
    /// Error when state_hash does not exist or missed some trie nodes
    pub(crate) fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, MptError> {
        let trie = TrieDBBuilder::<NoExtensionLayout>::new(self, &self.root_hash).build();
        let value = trie.get(key).map_err(|err| MptError::from(*err))?;
        Ok(value)
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
        let trie = TrieDBBuilder::<NoExtensionLayout>::new(self, &self.root_hash).build();
        let value = trie.get(key).map_err(|err| MptError::from(*err))?;
        let proof_ret =
            generate_proof::<_, NoExtensionLayout, _, _>(self, &self.root_hash, [key].iter());
        let proof = proof_ret.map_err(|err| MptError::from(*err))?;
        Ok((proof, value))
    }

    /// `contains` check is the key exists in a trie
    ///
    /// Error when state_hash does not exist or missed some trie nodes
    pub(crate) fn contains(&self, key: &[u8]) -> Result<bool, MptError> {
        let trie = TrieDBBuilder::<NoExtensionLayout>::new(self, &self.root_hash).build();
        let exsits = trie.contains(key).map_err(|err| MptError::from(*err))?;
        Ok(exsits)
    }

    /// `iterate_all` in DFS approach to iterate all key-value pairs in MPT by a function. The iteration may
    /// end earlier if it fails to obtain key-value from the trie (e.g. state_hash does
    /// not exist or missed some trie nodes), or the function returns error.
    pub fn iterate_all<F, E>(&self, mut f: F) -> Result<(), E>
    where
        F: FnMut(Vec<u8>, Vec<u8>) -> Result<(), E>,
        E: From<MptError>,
    {
        let trie = TrieDBBuilder::<NoExtensionLayout>::new(self, &self.root_hash).build();
        let trie_iter = trie.iter().map_err(|err| MptError::from(*err))?;
        for item in trie_iter {
            let (key, value) = item.map_err(|err| MptError::from(*err))?;
            f(key, value)?;
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
            let mut trie =
                TrieDBMutBuilder::<NoExtensionLayout>::from_existing(self, &mut cur_root_hash)
                    .build();
            let _ = trie
                .insert(key, &value)
                .map_err(|err| MptError::from(*err))?;
            trie.commit();
            *trie.root()
        };

        self.root_hash = new_root_hash;
        Ok(())
    }

    /// `batch_set` is batch set <key, value> pairs into Trie
    /// Any value change will be reflected on `state_hash` change in WorldState
    ///
    /// Error when state_hash does not exist or missed some trie nodes
    pub fn batch_set(&mut self, data: &HashMap<Vec<u8>, Vec<u8>>) -> Result<(), MptError> {
        let mut cur_root_hash = self.root_hash;
        let new_root_hash = {
            let mut trie =
                TrieDBMutBuilder::<NoExtensionLayout>::from_existing(self, &mut cur_root_hash)
                    .build();
            for (key, value) in data.iter() {
                let _ = trie
                    .insert(key, value)
                    .map_err(|err| MptError::from(*err))?;
            }
            trie.commit();
            *trie.root()
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
            let mut trie =
                TrieDBMutBuilder::<NoExtensionLayout>::from_existing(self, &mut cur_root_hash)
                    .build();
            let _ = trie.remove(key).map_err(|err| MptError::from(*err))?;
            trie.commit();
            *trie.root()
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
            let mut trie =
                TrieDBMutBuilder::<NoExtensionLayout>::from_existing(self, &mut cur_root_hash)
                    .build();
            for key in key_set.iter() {
                let _ = trie.remove(key).map_err(|err| MptError::from(*err));
            }
            trie.commit();
            *trie.root()
        };
        self.root_hash = new_root_hash;
        Ok(())
    }

    /// `close` is return and flush cache changes in [DB](crate::db::DB). Also return the updated state_hash
    pub fn close(&mut self) -> MptChanges {
        let db_changes = self.db.close();
        MptChanges(db_changes.0, db_changes.1, self.root_hash)
    }
}

/// `MptChanges` is a wrapper of changes in [Mpt] when call function close()
///
/// The reason that MptChanges struct exposed to public is we need it in benchmark test
#[derive(Debug, Clone)]
pub struct MptChanges(
    pub HashMap<Vec<u8>, Vec<u8>>,
    pub HashSet<Vec<u8>>,
    pub Sha256Hash,
);

impl<'a, S: DB + Send + Sync + Clone> Mpt<'a, S, crate::V1> {
    pub(crate) fn deinit_and_upgrade(mut self) -> Result<Mpt<'a, S, crate::V2>, MptError> {
        // deinit the the V1 mpt
        let empty_root_hash = RefHasher::hash(PREIMAGE_OF_EMPTY_TRIE_ROOT_HASH).to_vec();
        // check if root_hash is equal to empty root hash
        if self.root_hash() != RefHasher::hash(PREIMAGE_OF_EMPTY_TRIE_ROOT_HASH) {
            return Err(MptError::InvalidStateRoot);
        }
        let trie = TrieDBBuilder::<NoExtensionLayout>::new(&self, &self.root_hash).build();
        if trie.iter().is_ok() {
            // need to hard delete the root node
            self.db.delete(empty_root_hash);
        }
        // else the root hash has already been deleted

        // upgrade
        let mut new_storage: KeyInstrumentedDB<'a, S, crate::V2> = self.db.upgrade();
        // compute root_hash for a empty trie in V2
        let mut default_root_hash = Default::default();
        let key: Vec<u8> = RefHasher::hash(PREIMAGE_OF_EMPTY_TRIE_ROOT_HASH).to_vec();
        let value: Vec<u8> = EMPTY_TRIE_DUMMY_ROOT_NODE.to_vec();
        new_storage.put(key, value);
        let mut genesis_mpt: Mpt<S, crate::V2> = Mpt {
            db: new_storage.clone(),
            root_hash: default_root_hash,
        };
        let new_root_hash = {
            let mut trie = TrieDBMutBuilder::<NoExtensionLayout>::new(
                &mut genesis_mpt,
                &mut default_root_hash,
            )
            .build();
            trie.commit();
            *trie.root()
        };
        Ok(Mpt {
            db: new_storage,
            root_hash: new_root_hash,
        })
    }
}

type Hash256 = [u8; 32];

/// Implemented to meet a requirement of Parity's Base-16 Modified Merkle Tree ("Trie")
///
/// Database must implement this HashDB trait to create mutable trie.
///
/// For example https://github.com/paritytech/trie/blob/master/memory-db/src/lib.rs
impl<'a, S: DB + Send + Sync + Clone, V: VersionProvider + Send + Sync + Clone>
    HashDB<RefHasher, Vec<u8>> for Mpt<'a, S, V>
{
    /// Look up a given hash into the bytes that hash to it, returning None if the hash is not known.
    fn get(&self, key: &Hash256, nibble_prefix: Prefix) -> Option<Vec<u8>> {
        let key = prefixed_trie_node_key::<RefHasher>(key, nibble_prefix);
        self.db.get(&key)
    }

    /// Check for the existence of a hash-key.
    fn contains(&self, key: &Hash256, nibble_prefix: Prefix) -> bool {
        let key = prefixed_trie_node_key::<RefHasher>(key, nibble_prefix);
        self.db.get(key.as_ref()).is_some()
    }

    /// Insert item into the DB and return the hash for a later lookup.
    fn insert(&mut self, nibble_prefix: Prefix, value: &[u8]) -> Hash256 {
        let key = RefHasher::hash(value);
        self.emplace(key, nibble_prefix, value.to_vec());
        key
    }

    /// Like insert(), except you provide the key and the data is all moved.
    fn emplace(&mut self, key: Hash256, nibble_prefix: Prefix, value: Vec<u8>) {
        let key = prefixed_trie_node_key::<RefHasher>(&key, nibble_prefix);
        self.db.put(key, value);
    }

    /// Remove an item previously inserted.
    fn remove(&mut self, key: &Hash256, nibble_prefix: Prefix) {
        if key[..] == RefHasher::hash(PREIMAGE_OF_EMPTY_TRIE_ROOT_HASH) {
            return;
        }
        let key = prefixed_trie_node_key::<RefHasher>(key, nibble_prefix);
        self.db.delete(key);
    }
}

pub(crate) fn prefixed_trie_node_key<H: KeyHasher>(
    hash: &H::Out,
    nibble_prefix: Prefix,
) -> Vec<u8> {
    let mut prefixed_key = Vec::with_capacity(hash.as_ref().len() + nibble_prefix.0.len() + 1);
    prefixed_key.extend_from_slice(nibble_prefix.0);
    if let Some(last) = nibble_prefix.1 {
        prefixed_key.push(last);
    }
    prefixed_key.extend_from_slice(hash.as_ref());
    prefixed_key
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
    fn get(&self, key: &Hash256, nibble_prefix: Prefix) -> Option<Vec<u8>> {
        HashDB::get(self, key, nibble_prefix)
    }

    fn contains(&self, key: &Hash256, nibble_prefix: Prefix) -> bool {
        HashDB::contains(self, key, nibble_prefix)
    }
}

pub(crate) use proof_level::ProofLevel;
use std::mem::size_of;

/// WSProofNode is node in the trie traversed while performing lookups on the Key, prefixed by the trie level that they belong to:
/// `${proof_level}/${trie_node_key}`
pub(crate) struct WSProofNode(Vec<u8>);

impl WSProofNode {
    pub(crate) fn new(proof_level: ProofLevel, node_key: Vec<u8>) -> WSProofNode {
        let mut key: Vec<u8> = Vec::with_capacity(node_key.len() + size_of::<u8>());
        key.push(proof_level);
        key.extend_from_slice(&node_key);
        WSProofNode(key)
    }
}

impl From<WSProofNode> for Vec<u8> {
    fn from(proof_node: WSProofNode) -> Self {
        proof_node.0
    }
}

/// This sub mod provides prefix for proof node.
pub(crate) mod proof_level {
    /// ProofLevel forms part of a proof node prefix. It splits the Proof of key into two:
    ///
    /// Accounts level
    ///
    /// Storage level
    pub(crate) type ProofLevel = u8;

    /// `ACCOUNTS` is the proof of the storage hash in AccountsTrie
    pub(crate) const ACCOUNTS: ProofLevel = 0x00;

    /// `STORAGE` is the proof of key inside smart contracts (AppKey) in storage tire.
    pub(crate) const STORAGE: ProofLevel = 0x01;
}

/// `Visibility` is the prefix to identify the external account key and contract account key
#[repr(u8)]
pub(crate) enum KeyVisibility {
    Public = 0,
    Protected = 1,
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::accounts_trie::account_key;
    use crate::accounts_trie::AccountField;
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
    pub fn simple_insert_v1() {
        let mut env = TestEnv::default();
        let db = KeyInstrumentedDB::<DummyStorage, V1>::new(&env.db, env.address.to_vec());
        let apple_key = b"apple".to_vec();
        let apple_value = b"apple_12345".to_vec();
        let banana_key = b"banana".to_vec();
        let banana_value = b"banana_12345".to_vec();
        let mut mpt = Mpt::<DummyStorage, V1>::new(db);
        mpt.set(&apple_key, apple_value.clone()).unwrap();
        mpt.set(&banana_key, banana_value.clone()).unwrap();
        let changes = mpt.close();
        println!("================================mpt changes============================");
        println!("inserts: {:?}, deletes {:?}", changes.0, changes.1);
        env.db.apply_changes(changes.0, changes.1);
        println!("{:?}", env.db);
    }

    #[test]
    pub fn init_and_open() {
        let env = TestEnv::default();
        let db = KeyInstrumentedDB::<DummyStorage, V1>::new(&env.db, env.address.to_vec());
        let ret = Mpt::<DummyStorage, V1>::new(db.clone());
        let key = account_key::<V1>(&env.address, AccountField::Nonce);
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
            &RefHasher::hash(PREIMAGE_OF_EMPTY_TRIE_ROOT_HASH),
            RefHasher::hash(EMPTY_TRIE_DUMMY_ROOT_NODE).to_vec(),
        )
        .unwrap();
        let mpt_change = ret.close();
        println!("inserts{:?}", &mpt_change.0);
        println!("deletes{:?}", &mpt_change.1);
        println!("root_hash{:?}", &mpt_change.2);
        env.db.apply_changes(mpt_change.0, mpt_change.1);
        let db = KeyInstrumentedDB::<DummyStorage, V1>::new(&env.db, env.address.to_vec());
        let ret = Mpt::<DummyStorage, V1>::open(db, mpt_change.2);
        ret.get(&RefHasher::hash(PREIMAGE_OF_EMPTY_TRIE_ROOT_HASH))
            .unwrap();
    }

    #[test]
    pub fn test() {
        println!("{:?}", RefHasher::hash(PREIMAGE_OF_EMPTY_TRIE_ROOT_HASH));
        println!("{:?}", RefHasher::hash(EMPTY_TRIE_DUMMY_ROOT_NODE));
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
        let ret = Mpt::<DummyStorage, V1>::open(db, ws_changes.2);
        let mut mpt_v2 = ret.deinit_and_upgrade().unwrap();
        let ws_changes = mpt_v2.close();
        env.db.apply_changes(ws_changes.0, ws_changes.1);
        println!("==================== db after deinit ==================");
        println!("{:?}", &env.db);
    }
}
