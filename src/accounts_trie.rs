/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! This mod provide structs and implementation related to Account and AccountTrie.
//! [AccountsTrie] store external account information in blockchain.
//! [Account] store external account information in blockchain.
//! [AccountField] prefix to identify the data type.

use std::collections::{HashMap, HashSet};

use pchain_types::cryptography::{PublicAddress, Sha256Hash};

use crate::db::{KeyInstrumentedDB, DB};
use crate::error::{DecodeOrEncodeError, MptError, TrieKeyBuildError, WorldStateError};
use crate::mpt::{Mpt, Proof};
use crate::proof_node::{proof_level, WSProofNode};
use crate::trie_key::TrieKey;
use crate::version::*;
use crate::world_state::{DestroyWorldStateChanges, WorldStateChanges};
use reference_trie::{ExtensionLayout, NoExtensionLayout};
use trie_db::{Trie, TrieDBBuilder};
/// Struct store external account information in blockchain
#[derive(Debug, Clone)]
pub struct AccountsTrie<'a, S, V>
where
    S: DB + Send + Sync + Clone,
    V: VersionProvider + Send + Sync + Clone,
{
    trie: Mpt<'a, S, V>,
}
/// `Account` store information about account and return to caller when caller iter the [AccountsTrie](crate::accounts::AccountsTrie)
#[derive(Debug, Clone)]
pub struct Account {
    pub nonce: u64,
    pub balance: u64,
    pub code: Vec<u8>,
    pub cbi_version: u32,
    pub storage_hash: Vec<u8>,
    pub storages: HashMap<Vec<u8>, Vec<u8>>,
}

impl Default for Account {
    fn default() -> Self {
        Self {
            nonce: 0_u64,
            balance: 0_u64,
            code: vec![],
            cbi_version: 0_u32,
            storage_hash: vec![],
            storages: HashMap::new(),
        }
    }
}

impl Account {
    pub fn storage_hash(&self) -> Option<Sha256Hash> {
        if !self.storage_hash.is_empty() {
            Some(self.storage_hash.clone().try_into().unwrap())
        } else {
            None
        }
    }

    pub fn storages(&self) -> HashMap<Vec<u8>, Vec<u8>> {
        self.storages.clone()
    }

    pub fn set_storage_hash(&mut self, storage_hash: Vec<u8>) {
        self.storage_hash = storage_hash;
    }

    pub fn set_storages(&mut self, storages: HashMap<Vec<u8>, Vec<u8>>) {
        if storages.is_empty() {
            return;
        }
        self.storages = storages;
    }
}

/// `AccountField` prefix to identify the data type belong to [AccountsTrie](crate::accounts::AccountsTrie)
#[repr(u8)]
pub enum AccountField {
    Nonce = 0,
    Balance = 1,
    ContractCode = 2,
    CbiVersion = 3,
    StorageHash = 4,
}

impl TryInto<AccountField> for u8 {
    type Error = TrieKeyBuildError;

    fn try_into(self) -> Result<AccountField, TrieKeyBuildError> {
        match self {
            0_u8 => Ok(AccountField::Nonce),
            1_u8 => Ok(AccountField::Balance),
            2_u8 => Ok(AccountField::ContractCode),
            3_u8 => Ok(AccountField::CbiVersion),
            4_u8 => Ok(AccountField::StorageHash),
            _ => Err(TrieKeyBuildError::InvalidAccountField),
        }
    }
}

/// interfaces can be called by outside user
impl<'a, S: DB + Send + Sync + Clone, V: VersionProvider + Send + Sync + Clone>
    AccountsTrie<'a, S, V>
{
    /// `nonce` is return the nonce of given account address
    ///
    /// 0 if the account address is not found in world state
    ///
    /// Error if state_hash does not exist or missed some trie nodes
    pub fn nonce(&self, address: &PublicAddress) -> Result<u64, MptError> {
        let nonce_key: Vec<u8> = TrieKey::<V>::account_key(address, AccountField::Nonce);
        if let Some(value) = self.trie.get(&nonce_key)? {
            Ok(u64::from_le_bytes(value.try_into().unwrap()))
        } else {
            Ok(0_u64)
        }
    }

    /// `nonce_with_proof` is return the nonce with proof of given account address
    ///
    /// (empty vector, 0) if the account address is not found in world state
    ///
    /// Error if state_hash does not exist or missed some trie nodes
    pub fn nonce_with_proof(&self, address: &PublicAddress) -> Result<(Proof, u64), MptError> {
        let nonce_key: Vec<u8> = TrieKey::<V>::account_key(address, AccountField::Nonce);
        let (proof, value) = self.trie.get_with_proof(&nonce_key)?;
        let proof: Proof = proof
            .into_iter()
            .map(|node| WSProofNode::new(proof_level::ACCOUNTS, node).into())
            .collect();
        if value.is_some() {
            Ok((
                proof,
                u64::from_le_bytes(value.unwrap().try_into().unwrap()),
            ))
        } else {
            Ok((proof, 0_u64))
        }
    }

    /// `balance` is return the balance of given account address
    ///
    /// 0 if the account address is not found in world state
    ///
    /// Error if state_hash does not exist or missed some trie nodes
    pub fn balance(&self, address: &PublicAddress) -> Result<u64, MptError> {
        let balance_key: Vec<u8> = TrieKey::<V>::account_key(address, AccountField::Balance);
        let value = self.trie.get(&balance_key)?;
        if value.is_some() {
            Ok(u64::from_le_bytes(value.unwrap().try_into().unwrap()))
        } else {
            Ok(0_u64)
        }
    }

    /// `balance_with_proof` is return the balance with proof of given account address
    ///
    /// (empty vector, 0) if the account address is not found in world state
    ///
    /// Error if state_hash does not exist or missed some trie nodes
    pub fn balance_with_proof(&self, address: &PublicAddress) -> Result<(Proof, u64), MptError> {
        let balance_key: Vec<u8> = TrieKey::<V>::account_key(address, AccountField::Balance);
        let (proof, value) = self.trie.get_with_proof(&balance_key)?;
        let proof: Proof = proof
            .into_iter()
            .map(|node| WSProofNode::new(proof_level::ACCOUNTS, node).into())
            .collect();
        if value.is_some() {
            Ok((
                proof,
                u64::from_le_bytes(value.unwrap().try_into().unwrap()),
            ))
        } else {
            Ok((proof, 0_u64))
        }
    }

    /// `code` is return the code of given account address
    ///
    /// empty vector if the account address is not found in world state
    ///
    /// Error if state_hash does not exist or missed some trie nodes
    pub fn code(&self, address: &PublicAddress) -> Result<Option<Vec<u8>>, MptError> {
        let code_key: Vec<u8> = TrieKey::<V>::account_key(address, AccountField::ContractCode);
        let value = self.trie.get(&code_key)?;
        Ok(value)
    }

    /// `code_with_proof` is return the code with proof of given account address
    ///
    /// (empty vector, empty vector) if the account address is not found in world state
    ///
    /// Error if state_hash does not exist or missed some trie nodes
    pub fn code_with_proof(
        &self,
        address: &PublicAddress,
    ) -> Result<(Proof, Option<Vec<u8>>), MptError> {
        let code_key: Vec<u8> = TrieKey::<V>::account_key(address, AccountField::ContractCode);
        let (proof, value) = self.trie.get_with_proof(&code_key)?;
        let proof: Proof = proof
            .into_iter()
            .map(|node| WSProofNode::new(proof_level::ACCOUNTS, node).into())
            .collect();
        Ok((proof, value))
    }

    /// `cbi_version` is return the cbi_version of given account address
    ///
    /// 0 if the account address is not found in world state
    ///
    /// Error if state_hash does not exist or missed some trie nodes
    pub fn cbi_version(&self, address: &PublicAddress) -> Result<u32, MptError> {
        let cbi_version_key: Vec<u8> = TrieKey::<V>::account_key(address, AccountField::CbiVersion);
        let value = self.trie.get(&cbi_version_key)?;
        if value.is_some() {
            Ok(u32::from_le_bytes(value.unwrap().try_into().unwrap()))
        } else {
            Ok(0_u32)
        }
    }

    /// `cbi_version_with_proof` is return the cbi_version with proof of given account address
    ///
    /// (empty vector, 0) if the account address is not found in world state
    ///
    /// Error if state_hash does not exist or missed some trie nodes
    pub fn cbi_version_with_proof(
        &self,
        address: &PublicAddress,
    ) -> Result<(Proof, u32), MptError> {
        let cbi_version_key: Vec<u8> = TrieKey::<V>::account_key(address, AccountField::CbiVersion);
        let (proof, value) = self.trie.get_with_proof(&cbi_version_key)?;
        let proof = proof
            .into_iter()
            .map(|node| WSProofNode::new(proof_level::ACCOUNTS, node).into())
            .collect();
        if value.is_some() {
            Ok((
                proof,
                u32::from_le_bytes(value.unwrap().try_into().unwrap()),
            ))
        } else {
            Ok((proof, 0_u32))
        }
    }

    /// `storage_hash` is return the storage_hash of given account address
    ///
    /// empty vector if the account address is not found in world state
    ///
    /// Error if state_hash does not exist or missed some trie nodes
    pub fn storage_hash(&self, address: &PublicAddress) -> Result<Option<Sha256Hash>, MptError> {
        let storage_hash_key: Vec<u8> =
            TrieKey::<V>::account_key(address, AccountField::StorageHash);
        let value = self.trie.get(&storage_hash_key)?;
        Ok(value.map(|hash_value| hash_value.try_into().unwrap()))
    }

    /// `storage_hash` is return the storage_hash with proof of given account address
    ///
    /// (empty vector, empty vector) if the account address is not found in world state
    ///
    /// Error if state_hash does not exist or missed some trie nodes
    pub fn storage_hash_with_proof(
        &self,
        address: &PublicAddress,
    ) -> Result<(Proof, Option<Sha256Hash>), MptError> {
        let storage_hash_key: Vec<u8> =
            TrieKey::<V>::account_key(address, AccountField::StorageHash);
        let (proof, value) = self.trie.get_with_proof(&storage_hash_key)?;
        let proof = proof
            .into_iter()
            .map(|node| WSProofNode::new(proof_level::ACCOUNTS, node).into())
            .collect();
        Ok((
            proof,
            value.map(|hash_value| hash_value.try_into().unwrap()),
        ))
    }

    /// `all` is to iterator all Account information in AccountTrie
    ///
    /// Return a iterator of (PublicAddress, Account)
    ///
    /// Error if state_hash does not exist or missed some trie nodes
    pub fn all(&self) -> Result<HashMap<PublicAddress, Account>, WorldStateError> {
        let account_map = self.trie.all().map_err(WorldStateError::MptError)?;
        let mut ret_map: HashMap<PublicAddress, Account> = HashMap::new();
        for (key, value) in account_map.into_iter() {
            let account_address =
                TrieKey::<V>::account_address(&key).map_err(WorldStateError::TrieKeyBuildError)?;
            let mut account_value: Account = match ret_map.get(&account_address) {
                Some(account) => account.clone(),
                None => Account::default(),
            };
            let account_filed: AccountField =
                TrieKey::<V>::account_field(&key).map_err(WorldStateError::TrieKeyBuildError)?;
            match account_filed {
                AccountField::Nonce => {
                    account_value.nonce = u64::from_le_bytes(value.try_into().map_err(|_| {
                        WorldStateError::DecodeOrEncodeError(DecodeOrEncodeError::DecodeError)
                    })?);
                }
                AccountField::Balance => {
                    account_value.balance = u64::from_le_bytes(value.try_into().map_err(|_| {
                        WorldStateError::DecodeOrEncodeError(DecodeOrEncodeError::DecodeError)
                    })?);
                }
                AccountField::ContractCode => account_value.code = value,
                AccountField::CbiVersion => {
                    account_value.cbi_version =
                        u32::from_le_bytes(value.try_into().map_err(|_| {
                            WorldStateError::DecodeOrEncodeError(DecodeOrEncodeError::DecodeError)
                        })?);
                }
                AccountField::StorageHash => account_value.set_storage_hash(value),
            }
            ret_map.insert(account_address, account_value);
        }
        Ok(ret_map)
    }

    /// `contains` is to check if account field data exsists or not
    ///
    /// Error when state_hash does not exist or missed some trie nodes
    pub fn contains(
        &self,
        address: &PublicAddress,
        account_field: AccountField,
    ) -> Result<bool, MptError> {
        let key: Vec<u8> = TrieKey::<V>::account_key(address, account_field);
        let exists = self.trie.contains(&key)?;
        Ok(exists)
    }

    /// `set_nonce` is to set/update account nonce
    pub fn set_nonce(&mut self, address: &PublicAddress, nonce: u64) -> Result<(), MptError> {
        let nonce_key: Vec<u8> = TrieKey::<V>::account_key(address, AccountField::Nonce);
        let value = nonce.to_le_bytes().to_vec();
        self.trie.set(&nonce_key, value)?;
        Ok(())
    }

    /// `set_balance` is to set/update account balance
    pub fn set_balance(&mut self, address: &PublicAddress, balance: u64) -> Result<(), MptError> {
        let balance_key: Vec<u8> = TrieKey::<V>::account_key(address, AccountField::Balance);
        let value = balance.to_le_bytes().to_vec();
        self.trie.set(&balance_key, value)?;
        Ok(())
    }

    /// `set_code` is to set contract code of contract account
    pub fn set_code(&mut self, address: &PublicAddress, code: Vec<u8>) -> Result<(), MptError> {
        let code_key: Vec<u8> = TrieKey::<V>::account_key(address, AccountField::ContractCode);
        self.trie.set(&code_key, code)?;
        Ok(())
    }

    /// `set_cbi_version` is to set/update account cbi_version
    pub fn set_cbi_version(
        &mut self,
        address: &PublicAddress,
        cbi_version: u32,
    ) -> Result<(), MptError> {
        let cbi_version_key: Vec<u8> = TrieKey::<V>::account_key(address, AccountField::CbiVersion);
        let value = cbi_version.to_le_bytes().to_vec();
        self.trie.set(&cbi_version_key, value)?;
        Ok(())
    }
}

/// intefaces called by [WorldState](crate::world_state::WorldState)
impl<'a, S: DB + Send + Sync + Clone, V: VersionProvider + Send + Sync + Clone>
    AccountsTrie<'a, S, V>
{
    /// `new` called by [WorldState](crate::world_state::WorldState) to create a new AccountsTrie at the genesis
    ///
    /// Only can be called once
    pub(crate) fn new(storage: &'a S) -> Self {
        let db = KeyInstrumentedDB::new(storage, vec![]);
        let trie = Mpt::new(db);
        AccountsTrie { trie }
    }

    /// `open` called by [WorldState](crate::world_state::WorldState) to open the created AccountTrie by specific state_hash
    pub(crate) fn open(storage: &'a S, state_hash: Sha256Hash) -> AccountsTrie<S, V> {
        let db = KeyInstrumentedDB::new(storage, vec![]);
        let trie = Mpt::open(db, state_hash);
        AccountsTrie { trie }
    }

    /// `root_hash` called by [WorldState](crate::world_state::WorldState) to get the root hash of the current trie
    pub(crate) fn root_hash(&self) -> Sha256Hash {
        self.trie.root_hash()
    }

    /// `set_storage_hash` called by [WorldState](crate::world_state::WorldState) to set account storage_hash
    pub(crate) fn set_storage_hash(
        &mut self,
        address: &PublicAddress,
        storage_hash: Sha256Hash,
    ) -> Result<(), MptError> {
        let storage_hash_key: Vec<u8> =
            TrieKey::<V>::account_key(address, AccountField::StorageHash);
        let value = storage_hash.to_vec();
        self.trie.set(&storage_hash_key, value)?;
        Ok(())
    }

    /// `close` called by [WorldState](crate::world_state::WorldState) return all cached updates in AccountTrie and updated root_hash of AccountTrie
    pub(crate) fn close(&mut self) -> WorldStateChanges {
        let mpt_changes = self.trie.close();
        WorldStateChanges {
            inserts: mpt_changes.0,
            deletes: mpt_changes.1,
            new_root_hash: mpt_changes.2,
        }
    }

    /// `destory` is called by [WorldState](crate::world_state::WorldState) to destroy the existing AccountTrie
    ///
    /// Return the [DestoryWorldStateReturn] for physical db to do the physical deletion and future WorldState rebuild
    pub(crate) fn destroy(&mut self) -> Result<DestroyWorldStateChanges, WorldStateError> {
        let mut data_map: HashMap<PublicAddress, Account> = HashMap::new();
        match <V>::version() {
            Version::V1 => {
                let current_root_hash = self.trie.root_hash();
                let account_trie =
                    TrieDBBuilder::<NoExtensionLayout>::new(&self.trie, &current_root_hash).build();
                let account_iter = account_trie
                    .iter()
                    .map_err(|err| WorldStateError::MptError(MptError::from(*err)))?;
                // key_set to store keys that need to be delete
                let mut key_set: HashSet<Vec<u8>> = HashSet::new();
                for item in account_iter {
                    let (key, value) =
                        item.map_err(|err| WorldStateError::MptError(MptError::from(*err)))?;
                    let account_address = TrieKey::<V>::account_address(&key)
                        .map_err(WorldStateError::TrieKeyBuildError)?;
                    let mut account: Account = match data_map.get(&account_address) {
                        Some(account) => account.clone(),
                        None => Account::default(),
                    };
                    let account_filed: AccountField = TrieKey::<V>::account_field(&key)
                        .map_err(WorldStateError::TrieKeyBuildError)?;
                    key_set.insert(key);
                    match account_filed {
                        AccountField::Nonce => {
                            account.nonce = u64::from_le_bytes(value.try_into().map_err(|_| {
                                WorldStateError::DecodeOrEncodeError(
                                    DecodeOrEncodeError::DecodeError,
                                )
                            })?);
                        }
                        AccountField::Balance => {
                            account.balance =
                                u64::from_le_bytes(value.try_into().map_err(|_| {
                                    WorldStateError::DecodeOrEncodeError(
                                        DecodeOrEncodeError::DecodeError,
                                    )
                                })?);
                        }
                        AccountField::ContractCode => {
                            account.code = value;
                        }
                        AccountField::CbiVersion => {
                            account.cbi_version =
                                u32::from_le_bytes(value.try_into().map_err(|_| {
                                    WorldStateError::DecodeOrEncodeError(
                                        DecodeOrEncodeError::DecodeError,
                                    )
                                })?);
                        }
                        AccountField::StorageHash => {
                            account.set_storage_hash(value);
                        }
                    }

                    data_map.insert(account_address, account);
                }
                // destroy all account field info
                self.trie
                    .batch_remove(&key_set)
                    .map_err(WorldStateError::MptError)?;
                // destroy the account trie
                self.trie.deinit().map_err(WorldStateError::MptError)?;
                let mpt_changes = self.trie.close();
                Ok(DestroyWorldStateChanges {
                    inserts: mpt_changes.0,
                    deletes: mpt_changes.1,
                    accounts: data_map,
                })
            }
            Version::V2 => {
                let current_root_hash = self.trie.root_hash();
                let account_trie =
                    TrieDBBuilder::<ExtensionLayout>::new(&self.trie, &current_root_hash).build();
                let account_iter = account_trie
                    .iter()
                    .map_err(|err| WorldStateError::MptError(MptError::from(*err)))?;
                // key_set to store keys that need to be delete
                let mut key_set: HashSet<Vec<u8>> = HashSet::new();
                for item in account_iter {
                    let (key, value) =
                        item.map_err(|err| WorldStateError::MptError(MptError::from(*err)))?;
                    let account_address = TrieKey::<V>::account_address(&key)
                        .map_err(WorldStateError::TrieKeyBuildError)?;
                    let mut account: Account = match data_map.get(&account_address) {
                        Some(account) => account.clone(),
                        None => Account::default(),
                    };
                    let account_filed: AccountField = TrieKey::<V>::account_field(&key)
                        .map_err(WorldStateError::TrieKeyBuildError)?;
                    key_set.insert(key);
                    match account_filed {
                        AccountField::Nonce => {
                            account.nonce = u64::from_le_bytes(value.try_into().map_err(|_| {
                                WorldStateError::DecodeOrEncodeError(
                                    DecodeOrEncodeError::DecodeError,
                                )
                            })?);
                        }
                        AccountField::Balance => {
                            account.balance =
                                u64::from_le_bytes(value.try_into().map_err(|_| {
                                    WorldStateError::DecodeOrEncodeError(
                                        DecodeOrEncodeError::DecodeError,
                                    )
                                })?);
                        }
                        AccountField::ContractCode => {
                            account.code = value;
                        }
                        AccountField::CbiVersion => {
                            account.cbi_version =
                                u32::from_le_bytes(value.try_into().map_err(|_| {
                                    WorldStateError::DecodeOrEncodeError(
                                        DecodeOrEncodeError::DecodeError,
                                    )
                                })?);
                        }
                        AccountField::StorageHash => {
                            account.set_storage_hash(value);
                        }
                    }
                    data_map.insert(account_address, account);
                }
                // destroy all account field info
                self.trie
                    .batch_remove(&key_set)
                    .map_err(WorldStateError::MptError)?;
                // destroy the account trie
                self.trie.deinit().map_err(WorldStateError::MptError)?;
                let mpt_changes = self.trie.close();
                Ok(DestroyWorldStateChanges {
                    inserts: mpt_changes.0,
                    deletes: mpt_changes.1,
                    accounts: data_map,
                })
            }
        }
    }
}
