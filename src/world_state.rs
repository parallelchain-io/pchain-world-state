/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! This mod provides WorldState's implementations.
//! [WorldStateChanges] store the WorldState changes since opening.For Fullnode and Runtime to create AppState updates
//! [WorldState] read and update data in trie structrue.

use std::collections::{HashMap, HashSet};

use pchain_types::cryptography::{PublicAddress, Sha256Hash};

use crate::db::DB;
use crate::{
    accounts_trie::{Account, AccountsTrie},
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

/// `DestoryReturn` is to store the necessary info when user call destory from WorldState
///
/// inserts: <key, value> pairs need to be insert into physical db
///
/// deletes: keys need to be delete from physical db
///
/// data_map: data which need to rebuild WorldState
#[derive(Debug, Clone)]
pub struct DestroyWorldStateChanges {
    pub inserts: HashMap<Vec<u8>, Vec<u8>>,
    pub deletes: HashSet<Vec<u8>>,
    pub accounts: HashMap<PublicAddress, Account>,
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

    /// `destory` is destroy the WorldState return [DestroyReturn](crate::AccountTrie::DestroyReturn)
    pub fn destroy(&mut self) -> Result<DestroyWorldStateChanges, WorldStateError> {
        // get and destory the AccountsTrie
        let mut accounts_destroy_return = self.account_trie_mut().destroy()?;
        for (address, account) in accounts_destroy_return.accounts.iter_mut() {
            // get and destory the StorageTrie
            if let Some(storage_hash) = account.storage_hash() {
                let mut storage_trie = StorageTrie::<S, V>::open(self.db, storage_hash, address);
                let storage_destory_return = storage_trie.destroy()?;
                // merge the info from StorageTrie destroy return to the account trie destroy return
                accounts_destroy_return
                    .inserts
                    .extend(storage_destory_return.inserts);
                accounts_destroy_return
                    .deletes
                    .extend(storage_destory_return.deletes);
                account.set_storages(storage_destory_return.data_map);
            }
        }
        Ok(accounts_destroy_return)
    }

    /// `build` is rebuild the WorldState and return [WorldStateChanges] for physical db change
    pub fn build(
        &mut self,
        accounts: HashMap<PublicAddress, Account>,
    ) -> Result<WorldStateChanges, WorldStateError> {
        for (address, account) in accounts.into_iter() {
            let account_trie_mut = self.account_trie_mut();
            account_trie_mut
                .set_nonce(&address, account.nonce)
                .map_err(WorldStateError::MptError)?;
            account_trie_mut
                .set_balance(&address, account.balance)
                .map_err(WorldStateError::MptError)?;
            account_trie_mut
                .set_cbi_version(&address, account.cbi_version)
                .map_err(WorldStateError::MptError)?;
            account_trie_mut
                .set_code(&address, account.code.clone())
                .map_err(WorldStateError::MptError)?;
            if !account.storages().is_empty() {
                let storage_trie_mut = self
                    .storage_trie_mut(&address)
                    .map_err(WorldStateError::MptError)?;
                storage_trie_mut
                    .batch_set(&account.storages())
                    .map_err(WorldStateError::MptError)?;
            }
        }
        // storage_hash of each accounts will be set inside close
        self.close()
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
                .set_storage_hash(&address, storage_change.new_root_hash)
                .map_err(WorldStateError::MptError)?;
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
