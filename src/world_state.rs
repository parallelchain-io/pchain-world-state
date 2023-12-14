/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! This mod provides WorldState's implementations.
//! [WorldStateChanges] store the WorldState changes since opening.For Fullnode and Runtime to create AppState updates
//! [WorldState] read and update data in trie structrue.

use core::prelude::v1;
use std::collections::{HashMap, HashSet};

use pchain_types::cryptography::{PublicAddress, Sha256Hash};

use crate::db::DB;

use crate::{
    accounts_trie::AccountsTrie,
    error::{MptError, WorldStateError},
    storage_trie::StorageTrie,
    version::*,
};

/// `WorldStateChanges` store the WorldState changes since opening.
/// For Fullnode and Runtime to create AppState updates
///
/// Keys in inserts and deletes are physical keys
#[derive(Debug, Clone)]
pub struct WorldStateChanges {
    pub inserts: HashMap<Vec<u8>, Vec<u8>>,
    pub deletes: HashSet<Vec<u8>>,
    pub new_root_hash: Sha256Hash,
}

/// WorldState is a struct to read and update data in trie structrue.
/// It caches account information and account storage change by [KeyInstrumentedDB](crate::db::KeyInstrumentedDB).
/// And `close` will return the cached changes as struct [WorldStateChanges] to caller, which can store the change to physical database.
///
/// `accounts_trie` store accounts affected in current block change
///
/// `storage_trie_map` store account storages affected in current block change
#[derive(Debug, Clone)]
pub struct WorldState<'a, S, V>
where
    S: DB + Send + Sync + Clone,
    V: VersionProvider + Send + Sync + Clone,
{
    accounts_trie: AccountsTrie<'a, S, V>,
    storage_trie_map: HashMap<PublicAddress, StorageTrie<'a, S, V>>,
    db: &'a S,
}

impl<'a, S: DB + Send + Sync + Clone, V: VersionProvider + Send + Sync + Clone>
    WorldState<'a, S, V>
{
    /// `new` initialize a genesis WorldState
    ///
    /// Only can be called once in gensis stage
    pub fn new(db: &'a S) -> Self {
        let accounts_trie = AccountsTrie::<S, V>::new(db);
        WorldState {
            accounts_trie,
            storage_trie_map: HashMap::new(),
            db,
        }
    }

    /// `open` create WorldState by state_hash
    ///
    /// Return AccountTrie with input state_hash and an empty StorageTrie map
    pub fn open(db: &'a S, state_hash: Sha256Hash) -> Self {
        let accounts_trie = AccountsTrie::<S, V>::open(db, state_hash);
        WorldState {
            accounts_trie,
            storage_trie_map: HashMap::new(),
            db,
        }
    }

    /// `account_trie_mut` return the created AccountTrie mut ref from created/opened WorldState for mutable operation
    pub fn account_trie_mut(&mut self) -> &mut AccountsTrie<'a, S, V> {
        &mut self.accounts_trie
    }

    /// `account_trie` return the created AccountTrie ref from created/opened WorldState for mutable operation
    pub fn account_trie(&self) -> &AccountsTrie<S, V> {
        &self.accounts_trie
    }

    /// `storage_trie` return StorageTrie mut ref
    ///
    /// If the queried StorageTrie is not in storage_trie_map, will do following process:
    ///
    /// 1. check if the given account(by input PublicAddress) contains a storage_hash not not.
    ///
    /// 2. if storage_hash is not empty, create the StorageTire will storage_hash
    ///
    /// 3. if storage_hash is empty, init a StorageTire with empty storage_hash
    ///
    /// 4. put the created StorageTrie into storage_trie_map
    pub fn storage_trie_mut(
        &mut self,
        address: &PublicAddress,
    ) -> Result<&mut StorageTrie<'a, S, V>, MptError> {
        // if StorageTrie has been created, just return the created StorageTrie
        if self.storage_trie_map.contains_key(address) {
            return Ok(self.storage_trie_map.get_mut(address).unwrap());
        }
        // let mut account_trie = self.accounts_trie1;
        let storage_trie = match self.accounts_trie.storage_hash(address)? {
            Some(storage_hash) => {
                // StorageTrie of input account address has been init
                StorageTrie::<S, V>::open(self.db, storage_hash, address)
            }
            None => {
                // StorageTrie of input account address has not been init
                let storage_trie = StorageTrie::new(self.db, address);
                // get the new storage_hash and insert into AccountTrie
                let storage_trie_root = storage_trie.root_hash();
                self.accounts_trie
                    .set_storage_hash(address, storage_trie_root)?;
                storage_trie
            }
        };
        // insert created StorageTrie into storage_trie_map
        self.storage_trie_map.insert(*address, storage_trie.clone());
        return Ok(self.storage_trie_map.get_mut(address).unwrap());
    }

    /// `storage_trie` return StorageTrie unmut ref
    ///  
    /// If the queried StorageTrie is not in storage_trie_map, will do following process:
    ///
    /// 1. check if the given account(by input PublicAddress) contains a storage_hash not not.
    ///
    /// 2. if storage_hash is not empty, create the StorageTire will storage_hash
    ///
    /// 3. if storage_hash is empty, init a StorageTire with empty storage_hash
    ///
    /// 4. put the created StorageTrie into storage_trie_map
    pub fn storage_trie(
        &mut self,
        address: &PublicAddress,
    ) -> Result<&StorageTrie<'a, S, V>, MptError> {
        // if StorageTrie has been created, just return the created StorageTrie
        if self.storage_trie_map.contains_key(address) {
            return Ok(self.storage_trie_map.get(address).unwrap());
        }
        let storage_trie = match self.accounts_trie.storage_hash(address)? {
            Some(storage_hash) => {
                // StorageTrie of input account address has been init
                StorageTrie::open(self.db, storage_hash, address)
            }
            None => {
                // StorageTrie of input account address has not been init
                let storage_trie = StorageTrie::new(self.db, address);
                // get the new storage_hash and insert into AccountTrie
                let storage_trie_root = storage_trie.root_hash();
                self.accounts_trie
                    .set_storage_hash(address, storage_trie_root)?;
                storage_trie
            }
        };
        // insert created StorageTrie into storage_trie_map
        self.storage_trie_map.insert(*address, storage_trie.clone());
        return Ok(self.storage_trie_map.get(address).unwrap());
    }

    /// `close` return all cached changes from the WorldState for caller to create App updates
    pub fn close(&mut self) -> Result<WorldStateChanges, WorldStateError> {
        let mut inserts = HashMap::new();
        let mut deletes = HashSet::new();
        // collect all changes from StorageTire by close all cached StorageTrie
        for (address, mut storage_trie) in self.storage_trie_map.clone().into_iter() {
            let storage_change = storage_trie.close();
            // update storage_hash for matched AccountTrie by closed storage_change's stroage_hash
            self.accounts_trie
                .set_storage_hash(&address, storage_change.new_root_hash)?;
            // merge the inserts and deletes from StroageTrie
            inserts.extend(storage_change.inserts);
            deletes.extend(storage_change.deletes);
        }
        // collect all changes from AccountTrie by close AccountTrie
        let accounts_change = self.accounts_trie.close();
        // merge the inserts and deletes from AccountTrie
        inserts.extend(accounts_change.inserts);
        deletes.extend(accounts_change.deletes);
        Ok(WorldStateChanges {
            inserts,
            deletes,
            new_root_hash: accounts_change.new_root_hash,
        })
    }
}

/// implementations only for WorldState V1
impl<'a, S: DB + Send + Sync + Clone> WorldState<'a, S, V1> {
    /// `upgrade` consume a WorldState::<V1> instance and return a WorldState::<V2>
    pub fn upgrade(self) -> Result<WorldState<'a, S, V2>, WorldStateError> {
        let (account_v2, storage_info_map) = self.accounts_trie.upgrade()?;
        let mut storage_map: HashMap<PublicAddress, StorageTrie<'a, S, V2>> = HashMap::new();
        for (address, storage_hash) in storage_info_map {
            let storage_trie_v1: StorageTrie<'a, S, V1> = {
                // suppose the worldstate v1 still have some unclosed changes
                if self.storage_trie_map.contains_key(&address) {
                    self.storage_trie_map.get(&address).unwrap().to_owned()
                } else {
                    StorageTrie::open(self.db, storage_hash, &address)
                }
            };
            let storage_trie_v2 = storage_trie_v1.upgrade()?;
            storage_map.insert(address, storage_trie_v2);
        }
        Ok(WorldState {
            accounts_trie: account_v2,
            storage_trie_map: storage_map,
            db: self.db,
        })
    }
}
