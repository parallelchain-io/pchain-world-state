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

use crate::{
    db::{KeyInstrumentedDB, DB},
    error::{DecodeOrEncodeError, MptError, TrieKeyBuildError, WorldStateError},
    mpt::{Mpt, Proof},
    proof_node::{proof_level, WSProofNode},
    trie_key::TrieKey,
    world_state::WorldStateChanges,
    VersionProvider, V1, V2,
};

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
#[derive(Debug, Clone, Default)]
pub struct Account {
    pub nonce: u64,
    pub balance: u64,
    pub code: Vec<u8>,
    pub cbi_version: Option<u32>,
    pub storage_hash: Vec<u8>,
}

impl Account {
    pub fn storage_hash(&self) -> Option<Sha256Hash> {
        if !self.storage_hash.is_empty() {
            Some(self.storage_hash.clone().try_into().unwrap())
        } else {
            None
        }
    }
    pub fn set_storage_hash(&mut self, storage_hash: Vec<u8>) {
        self.storage_hash = storage_hash;
    }
}

/// `AccountField` prefix to identify the data type belong to [AccountsTrie](crate::accounts::AccountsTrie)
#[repr(u8)]
pub(crate) enum AccountField {
    Nonce = 0,
    Balance = 1,
    ContractCode = 2,
    CbiVersion = 3,
    StorageHash = 4,
}

impl TryFrom<u8> for AccountField {
    type Error = TrieKeyBuildError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
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
        let nonce_key = TrieKey::<V>::account_key(address, AccountField::Nonce);
        self.trie
            .get(&nonce_key)
            .map(|value| value.map_or(0, |value| u64::from_le_bytes(value.try_into().unwrap())))
    }

    /// `nonce_with_proof` is return the nonce with proof of given account address
    ///
    /// (empty vector, 0) if the account address is not found in world state
    ///
    /// Error if state_hash does not exist or missed some trie nodes
    pub fn nonce_with_proof(&self, address: &PublicAddress) -> Result<(Proof, u64), MptError> {
        let nonce_key = TrieKey::<V>::account_key(address, AccountField::Nonce);
        self.get_with_proof_from_trie_key(&nonce_key)
            .map(|(proof, value)| {
                let value = value.map_or(0, |value| u64::from_le_bytes(value.try_into().unwrap()));
                (proof, value)
            })
    }

    /// `balance` is return the balance of given account address
    ///
    /// 0 if the account address is not found in world state
    ///
    /// Error if state_hash does not exist or missed some trie nodes
    pub fn balance(&self, address: &PublicAddress) -> Result<u64, MptError> {
        let balance_key = TrieKey::<V>::account_key(address, AccountField::Balance);
        self.trie
            .get(&balance_key)
            .map(|value| value.map_or(0, |value| u64::from_le_bytes(value.try_into().unwrap())))
    }

    /// `balance_with_proof` is return the balance with proof of given account address
    ///
    /// (empty vector, 0) if the account address is not found in world state
    ///
    /// Error if state_hash does not exist or missed some trie nodes
    pub fn balance_with_proof(&self, address: &PublicAddress) -> Result<(Proof, u64), MptError> {
        let balance_key: Vec<u8> = TrieKey::<V>::account_key(address, AccountField::Balance);
        self.get_with_proof_from_trie_key(&balance_key)
            .map(|(proof, value)| {
                let value = value.map_or(0, |value| u64::from_le_bytes(value.try_into().unwrap()));
                (proof, value)
            })
    }

    /// `code` is return the code of given account address
    ///
    /// empty vector if the account address is not found in world state
    ///
    /// Error if state_hash does not exist or missed some trie nodes
    pub fn code(&self, address: &PublicAddress) -> Result<Option<Vec<u8>>, MptError> {
        let code_key: Vec<u8> = TrieKey::<V>::account_key(address, AccountField::ContractCode);
        self.trie.get(&code_key)
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
        self.get_with_proof_from_trie_key(&code_key)
    }

    /// `cbi_version` is return the cbi_version of given account address
    ///
    /// None if the account address is not found in world state
    ///
    /// Error if state_hash does not exist or missed some trie nodes
    pub fn cbi_version(&self, address: &PublicAddress) -> Result<Option<u32>, MptError> {
        let cbi_version_key: Vec<u8> = TrieKey::<V>::account_key(address, AccountField::CbiVersion);
        self.trie
            .get(&cbi_version_key)
            .map(|value| value.map(|value| u32::from_le_bytes(value.try_into().unwrap())))
    }

    /// `cbi_version_with_proof` is return the cbi_version with proof of given account address
    ///
    /// (empty vector, None) if the account address is not found in world state
    ///
    /// Error if state_hash does not exist or missed some trie nodes
    pub fn cbi_version_with_proof(
        &self,
        address: &PublicAddress,
    ) -> Result<(Proof, Option<u32>), MptError> {
        let cbi_version_key: Vec<u8> = TrieKey::<V>::account_key(address, AccountField::CbiVersion);
        self.get_with_proof_from_trie_key(&cbi_version_key)
            .map(|(proof, value)| {
                let value = value.map(|value| u32::from_le_bytes(value.try_into().unwrap()));
                (proof, value)
            })
    }

    /// `storage_hash` is return the storage_hash of given account address
    ///
    /// empty vector if the account address is not found in world state
    ///
    /// Error if state_hash does not exist or missed some trie nodes
    pub fn storage_hash(&self, address: &PublicAddress) -> Result<Option<Sha256Hash>, MptError> {
        let storage_hash_key = TrieKey::<V>::account_key(address, AccountField::StorageHash);
        self.trie
            .get(&storage_hash_key)
            .map(|value| value.map(|hash_value| hash_value.try_into().unwrap()))
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
        let storage_hash_key = TrieKey::<V>::account_key(address, AccountField::StorageHash);
        self.get_with_proof_from_trie_key(&storage_hash_key)
            .map(|(proof, value)| {
                let value = value.map(|hash_value| hash_value.try_into().unwrap());
                (proof, value)
            })
    }

    /// `all` is to iterator all Account information in AccountTrie
    ///
    /// Return a iterator of (PublicAddress, Account)
    ///
    /// Error if state_hash does not exist or missed some trie nodes
    pub fn all(&self) -> Result<HashMap<PublicAddress, Account>, WorldStateError> {
        let mut ret_map: HashMap<PublicAddress, Account> = HashMap::new();

        self.trie.iterate_all(|key, value| {
            // Get the account address and the field from the key
            let account_address = TrieKey::<V>::account_address(&key)?;

            let account_field = TrieKey::<V>::account_field(&key)?;

            // Get mutable reference to the account from the account map.
            let account_value = match ret_map.get_mut(&account_address) {
                Some(account) => account,
                None => {
                    ret_map.insert(account_address, Account::default());
                    ret_map.get_mut(&account_address).unwrap()
                }
            };

            // Set the account according to account field
            match account_field {
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
                        Some(u32::from_le_bytes(value.try_into().map_err(|_| {
                            WorldStateError::DecodeOrEncodeError(DecodeOrEncodeError::DecodeError)
                        })?));
                }
                AccountField::StorageHash => account_value.set_storage_hash(value),
            }

            Ok::<(), WorldStateError>(())
        })?;

        Ok(ret_map)
    }

    /// `contains_nonce` is to check if account field `Nonce` exists in the world state
    ///
    /// Error when state_hash does not exist or missed some trie nodes
    pub fn contains_nonce(&self, address: &PublicAddress) -> Result<bool, MptError> {
        let key = TrieKey::<V>::account_key(address, AccountField::Nonce);
        self.trie.contains(&key)
    }

    /// `contains_balance` is to check if account field `Balance` exists in the world state
    ///
    /// Error when state_hash does not exist or missed some trie nodes
    pub fn contains_balance(&self, address: &PublicAddress) -> Result<bool, MptError> {
        let key = TrieKey::<V>::account_key(address, AccountField::Balance);
        self.trie.contains(&key)
    }

    /// `contains_code` is to check if account field `Code` exists in the world state
    ///
    /// Error when state_hash does not exist or missed some trie nodes
    pub fn contains_code(&self, address: &PublicAddress) -> Result<bool, MptError> {
        let key = TrieKey::<V>::account_key(address, AccountField::ContractCode);
        self.trie.contains(&key)
    }

    /// `contains_cbi_version` is to check if account field `CBI Version` exists in the world state
    ///
    /// Error when state_hash does not exist or missed some trie nodes
    pub fn contains_cbi_version(&self, address: &PublicAddress) -> Result<bool, MptError> {
        let key = TrieKey::<V>::account_key(address, AccountField::CbiVersion);
        self.trie.contains(&key)
    }

    /// `contains_storage_hash` is to check if account field `Storage Hash` exists in the world state
    ///
    /// Error when state_hash does not exist or missed some trie nodes
    pub fn contains_storage_hash(&self, address: &PublicAddress) -> Result<bool, MptError> {
        let key = TrieKey::<V>::account_key(address, AccountField::StorageHash);
        self.trie.contains(&key)
    }

    /// `set_nonce` is to set/update account nonce
    pub fn set_nonce(&mut self, address: &PublicAddress, nonce: u64) -> Result<(), MptError> {
        let nonce_key = TrieKey::<V>::account_key(address, AccountField::Nonce);
        let value = nonce.to_le_bytes().to_vec();
        self.trie.set(&nonce_key, value)
    }

    /// `set_balance` is to set/update account balance
    pub fn set_balance(&mut self, address: &PublicAddress, balance: u64) -> Result<(), MptError> {
        let balance_key = TrieKey::<V>::account_key(address, AccountField::Balance);
        let value = balance.to_le_bytes().to_vec();
        self.trie.set(&balance_key, value)
    }

    /// `set_code` is to set contract code of contract account
    pub fn set_code(&mut self, address: &PublicAddress, code: Vec<u8>) -> Result<(), MptError> {
        let code_key = TrieKey::<V>::account_key(address, AccountField::ContractCode);
        self.trie.set(&code_key, code)
    }

    /// `set_cbi_version` is to set/update account cbi_version
    pub fn set_cbi_version(
        &mut self,
        address: &PublicAddress,
        cbi_version: u32,
    ) -> Result<(), MptError> {
        let cbi_version_key = TrieKey::<V>::account_key(address, AccountField::CbiVersion);
        let value = cbi_version.to_le_bytes().to_vec();
        self.trie.set(&cbi_version_key, value)
    }

    /// Get the value with the proof from the account trie given a key. Each node in the proof is
    /// prepended with a prefix for proof level ACCOUNTS.
    ///
    /// Error if root hash does not exists or missed some trie nodes
    fn get_with_proof_from_trie_key(
        &self,
        trie_key: &Vec<u8>,
    ) -> Result<(Proof, Option<Vec<u8>>), MptError> {
        self.trie.get_with_proof(trie_key).map(|(proof, value)| {
            let proof = proof
                .into_iter()
                .map(|node| WSProofNode::new(proof_level::ACCOUNTS, node).into())
                .collect();
            (proof, value)
        })
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

    /// `set_storage_hash` called by [WorldState](crate::world_state::WorldState) to set account storage_hash
    pub(crate) fn set_storage_hash(
        &mut self,
        address: &PublicAddress,
        storage_hash: Sha256Hash,
    ) -> Result<(), MptError> {
        let storage_hash_key: Vec<u8> =
            TrieKey::<V>::account_key(address, AccountField::StorageHash);
        let value = storage_hash.to_vec();
        self.trie.set(&storage_hash_key, value)
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
}

pub(crate) type AccountUpgradeReturn<'a, S, V2> =
    (AccountsTrie<'a, S, V2>, HashMap<PublicAddress, [u8; 32]>);

impl<'a, S: DB + Send + Sync + Clone> AccountsTrie<'a, S, crate::V1> {
    pub(crate) fn upgrade(mut self) -> Result<AccountUpgradeReturn<'a, S, V2>, WorldStateError> {
        let mut data_map: HashMap<PublicAddress, Account> = HashMap::new();
        let mut key_set: HashSet<Vec<u8>> = HashSet::new();
        self.trie.iterate_all(|key, value| {
            key_set.insert(key.clone());
            let account_address = TrieKey::<V1>::account_address(&key)?;
            let account_field: AccountField = TrieKey::<V1>::account_field(&key)?;
            let account = match data_map.get_mut(&account_address) {
                Some(account) => account,
                None => {
                    data_map.insert(account_address, Account::default());
                    data_map.get_mut(&account_address).unwrap()
                }
            };
            match account_field {
                AccountField::Nonce => {
                    account.nonce = u64::from_le_bytes(value.try_into().map_err(|_| {
                        WorldStateError::DecodeOrEncodeError(DecodeOrEncodeError::DecodeError)
                    })?);
                }
                AccountField::Balance => {
                    account.balance = u64::from_le_bytes(value.try_into().map_err(|_| {
                        WorldStateError::DecodeOrEncodeError(DecodeOrEncodeError::DecodeError)
                    })?);
                }
                AccountField::ContractCode => {
                    account.code = value;
                }
                AccountField::CbiVersion => {
                    account.cbi_version =
                        Some(u32::from_le_bytes(value.try_into().map_err(|_| {
                            WorldStateError::DecodeOrEncodeError(DecodeOrEncodeError::DecodeError)
                        })?));
                }
                AccountField::StorageHash => {
                    account.set_storage_hash(value);
                }
            }
            Ok::<(), WorldStateError>(())
        })?;
        // destroy all account field info
        self.trie.batch_remove(&key_set)?;
        // destroy the account trie
        self.trie.deinit()?;
        // get v2 mpt for accounts
        let mut trie_v2 = self.trie.upgrade();
        // rebuild all accounts(except storage_hash) and storages
        let mut account_info_map: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
        let mut storage_info_map: HashMap<PublicAddress, [u8; 32]> = HashMap::new();
        for (address, account) in data_map {
            if account.nonce != 0_u64 {
                let nonce_key = TrieKey::<V2>::account_key(&address, AccountField::Nonce);
                let value = account.nonce.to_le_bytes().to_vec();
                account_info_map.insert(nonce_key, value);
            }
            if account.balance != 0_u64 {
                let balance_key = TrieKey::<V2>::account_key(&address, AccountField::Balance);
                let value = account.balance.to_le_bytes().to_vec();
                account_info_map.insert(balance_key, value);
            }
            if !account.code.is_empty() {
                let code_key = TrieKey::<V2>::account_key(&address, AccountField::ContractCode);
                account_info_map.insert(code_key, account.code.clone());
            }
            if account.cbi_version.is_some() {
                let cbi_version_key =
                    TrieKey::<V2>::account_key(&address, AccountField::CbiVersion);
                let value = account.cbi_version.unwrap().to_le_bytes().to_vec();
                account_info_map.insert(cbi_version_key, value);
            }
            if let Some(storage_hash) = account.storage_hash() {
                storage_info_map.insert(address, storage_hash);
            }
        }
        // batch insert account info
        trie_v2.batch_set(&account_info_map)?;
        Ok((AccountsTrie { trie: trie_v2 }, storage_info_map))
    }
}
