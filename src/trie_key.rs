/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! This mod only public inside crate. Provide struct and implementation of Key operations for Trie structure

use hash_db::{Hasher as KeyHasher, Prefix};
use pchain_types::cryptography::PublicAddress;
use std::{marker::PhantomData, mem::size_of};

use crate::accounts_trie::AccountField;
use crate::{
    error::TrieKeyBuildError,
    version::{Version, VersionProvider},
};

/// `Visibility` is the prefix to identify the external account key and contract account key
#[repr(u8)]
pub(crate) enum KeyVisibility {
    Public = 0,
    Protected = 1,
}

/// `TrieKey` is logic key for TrieDB
pub(crate) struct TrieKey<V: VersionProvider> {
    _type: PhantomData<V>,
}

impl<V: VersionProvider> TrieKey<V> {
    /// `account_key` is to create the key for [AccountsTrie](crate::accounts_trie::AccountsTrie)
    ///
    /// V1 AccountTrie Key is in form PublicAddress + KeyVisibility + AccountField
    ///
    /// V2 AccountTrie Key is in form PublicAddress + AccountField
    pub(crate) fn account_key(address: &PublicAddress, account_field: AccountField) -> Vec<u8> {
        match <V>::version() {
            Version::V1 => {
                let mut account_key: Vec<u8> = Vec::with_capacity(
                    size_of::<PublicAddress>() + size_of::<u8>() + size_of::<u8>(),
                );
                account_key.extend_from_slice(address);
                account_key.push(KeyVisibility::Protected as u8);
                account_key.push(account_field as u8);
                account_key
            }
            Version::V2 => {
                let mut account_key: Vec<u8> =
                    Vec::with_capacity(size_of::<PublicAddress>() + size_of::<u8>());
                account_key.extend_from_slice(address);
                account_key.push(account_field as u8);
                account_key
            }
        }
    }

    /// `storage_key` is to crate the key for [StorageTrie](crate::storage::StorageTrie)
    ///
    /// V1 StorageTrie Key is in form KeyVisibility + Vec<u8>
    ///
    /// V2 StorageTrie Key is in form Vec<u8>
    pub(crate) fn storage_key(key: &Vec<u8>) -> Vec<u8> {
        match <V>::version() {
            Version::V1 => {
                let mut storage_key: Vec<u8> = Vec::with_capacity(size_of::<u8>() + key.len());
                storage_key.push(KeyVisibility::Public as u8);
                storage_key.extend_from_slice(key);
                storage_key
            }
            Version::V2 => {
                let mut storage_key: Vec<u8> = Vec::with_capacity(key.len());
                storage_key.extend_from_slice(key);
                storage_key
            }
        }
    }

    /// `account_field` is to seperate the AccountField from [AccountsTrie](crate::accounts_trie::AccountsTrie) Key
    pub(crate) fn account_field(key: &[u8]) -> Result<AccountField, TrieKeyBuildError> {
        let account_field_byte_index = match <V>::version() {
            Version::V1 => size_of::<PublicAddress>() + size_of::<u8>(),
            Version::V2 => size_of::<PublicAddress>(),
        };

        if key.len() <= account_field_byte_index {
            return Err(TrieKeyBuildError::InvalidAccountField);
        }

        AccountField::try_from(key[account_field_byte_index])
    }

    /// `account_address` is to seperate the account address from [AccountsTrie](crate::accounts_trie::AccountsTrie) Key
    pub(crate) fn account_address(key: &[u8]) -> Result<PublicAddress, TrieKeyBuildError> {
        if key.len() < size_of::<PublicAddress>() {    
            return Err(TrieKeyBuildError::InvalidPublicAddress);
        }
        
        key[..size_of::<PublicAddress>()]
            .try_into()
            .map_err(|_| TrieKeyBuildError::InvalidPublicAddress)
    }

    /// `drop_visibility_type` is to drop the visibility byte from [AccountsTrie](crate::accounts_trie::AccountsTrie) Key
    pub(crate) fn drop_visibility_type(key: &[u8]) -> Result<Vec<u8>, TrieKeyBuildError> {
        if key.len() < size_of::<u8>() {
            return Err(TrieKeyBuildError::Other);
        }

        Ok(key[size_of::<u8>()..].to_vec())
    }
}

/// Struct store node data inside Trie structure.
pub(crate) struct PrefixedTrieNodeKey<H>(PhantomData<H>);
impl<H: KeyHasher> PrefixedTrieNodeKey<H> {
    /// Key function that concatenates prefix and hash.
    // Derive a database key from hash value of the node (key) and the node prefix.
    pub(crate) fn key(hash: &H::Out, mpt_prefix: Prefix) -> Vec<u8> {
        let mut prefixed_key = Vec::with_capacity(hash.as_ref().len() + mpt_prefix.0.len() + 1);
        prefixed_key.extend_from_slice(mpt_prefix.0);
        if let Some(last) = mpt_prefix.1 {
            prefixed_key.push(last);
        }
        prefixed_key.extend_from_slice(hash.as_ref());
        prefixed_key
    }
}
