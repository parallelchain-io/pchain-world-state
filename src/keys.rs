/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Definition of Keys that Parallelchain-F are used to writes into persistent storage.
//!

use hash_db::{Hasher as KeyHasher, Prefix};
use std::{
    convert::TryInto,
    marker::PhantomData,
    mem::size_of,
    ops::{Deref, DerefMut},
};

use crate::error::WorldStateError;

/// AppKeys are keys from the point of view of smart contracts.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct AppKey(Vec<u8>);

impl AppKey {
    pub fn new(unboxed_key: Vec<u8>) -> AppKey {
        AppKey(unboxed_key)
    }
}

impl From<AppKey> for Vec<u8> {
    fn from(app_key: AppKey) -> Self {
        app_key.0
    }
}

impl Deref for AppKey {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for AppKey {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// WSKeys are AppKeys prefixed by the account that they belong to, and their app_visibility:
/// For Protected :
/// `${account}/${ws_key_visibility}/${account_key}`
/// For Public:
/// `${ws_key_visibility}/${account_key}`
/// As all public data is stored in independent account storage for each account,
/// the account prefix of WSKey is not required.
///
/// WSKeys are typically constructed from AppKeys using the `to_ws_key` method of the latter
/// type.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub(crate) enum WSKey {
    Public(Vec<u8>),
    Protected(Vec<u8>),
}

impl WSKey {
    pub(crate) fn for_public_account_storage_state(app_key: &AppKey) -> WSKey {
        let mut key: Vec<u8> = Vec::with_capacity(size_of::<u8>() + app_key.len());
        key.push(ws_key_visibility::PUBLIC);
        key.extend_from_slice(app_key);

        WSKey::Public(key)
    }

    pub(crate) fn for_protected_account_data(
        address: &pchain_types::cryptography::PublicAddress,
        item: ProtectedAccountData,
    ) -> WSKey {
        let mut key: Vec<u8> = Vec::with_capacity(
            size_of::<pchain_types::cryptography::PublicAddress>()
                + size_of::<u8>()
                + size_of::<u8>(),
        );
        key.extend_from_slice(address);
        key.push(ws_key_visibility::PROTECTED);
        key.push(item);

        WSKey::Protected(key)
    }
}
impl From<WSKey> for Vec<u8> {
    fn from(ws_key: WSKey) -> Self {
        match ws_key {
            WSKey::Public(key) => key,
            WSKey::Protected(key) => key,
        }
    }
}

impl<'a> From<&'a WSKey> for &'a Vec<u8> {
    fn from(ws_key: &'a WSKey) -> Self {
        match ws_key {
            WSKey::Public(key) => key,
            WSKey::Protected(key) => key,
        }
    }
}

impl AsRef<[u8]> for WSKey {
    fn as_ref(&self) -> &[u8] {
        match self {
            WSKey::Public(key) => key,
            WSKey::Protected(key) => key,
        }
    }
}

impl TryInto<AppKey> for WSKey {
    type Error = WorldStateError;

    fn try_into(self) -> Result<AppKey, WorldStateError> {
        match self {
            WSKey::Public(key) => {
                let mut full_wskey = key;
                let unboxed_key = full_wskey.split_off(size_of::<u8>());
                Ok(AppKey::new(unboxed_key))
            }
            WSKey::Protected(_) => Err(WorldStateError::ProtectedKey),
        }
    }
}

pub(crate) mod ws_key_visibility {
    /// WSKeyVisibility forms part of a WSKey's prefix. It splits the World State keys of an account into two: one which
    /// is freely read and writable from inside smart contracts, and another with special write protections.
    pub(crate) type WSKeyVisibility = u8;

    /// PUBLIC world state keys of an account is freely read and writable from inside smart contracts.
    pub(crate) const PUBLIC: WSKeyVisibility = 0x00;

    /// PROTECTED world state keys of an account cannot be written from inside smart contracts.
    pub(crate) const PROTECTED: WSKeyVisibility = 0x01;
}

pub(crate) mod protected_account_data {
    /// ProtectedAccountData enumerates the keys in the PROTECTED world state keys of an account that are used to store known
    /// values.  
    pub(crate) type ProtectedAccountData = u8;

    /// NONCE is the subkey used to store the number of transactions that an account has successfully included
    /// in the blockchain.
    pub(crate) const NONCE: ProtectedAccountData = 0x00;

    /// BALANCE is the subkey used to store the account's balance.
    pub(crate) const BALANCE: ProtectedAccountData = 0x01;

    /// CONTRACT_CODE is the subkey used to store the contract account's WASM bytecode. Obviously, it is not associated with
    /// any value if the account is an External account.
    pub(crate) const CONTRACT_CODE: ProtectedAccountData = 0x02;

    /// CBI_VERISON is the subkey used to store the version of the Contract ABI that the Contract’s Code expects. It is not associated with
    /// any value if the account is an External account.
    pub(crate) const CBI_VERISON: ProtectedAccountData = 0x03;

    /// STORAGE_HASH is the root hash of Account's storage subtrie. This is not associated with any value if the account is
    /// an External account.
    pub(crate) const STORAGE_HASH: ProtectedAccountData = 0x04;
}
pub(crate) use protected_account_data::ProtectedAccountData;

// WSProofNode is node in the trie traversed while performing lookups on the WSKey, prefixed by the trie level that they belong to:
/// `${proof_level}/${trie_node_key}`
pub(crate) struct WSProofNode(Vec<u8>);

impl WSProofNode {
    pub(crate) fn new(level: ProofLevel, node_key: Vec<u8>) -> WSProofNode {
        let mut key: Vec<u8> = Vec::with_capacity(node_key.len() + size_of::<u8>());
        key.push(level);
        key.extend_from_slice(&node_key);

        WSProofNode(key)
    }
}

impl From<WSProofNode> for Vec<u8> {
    fn from(proof_node: WSProofNode) -> Self {
        proof_node.0
    }
}

pub(crate) mod proof_level {
    /// ProofLevel forms part of a proof node prefix. It splits the Proof of key into two: one which
    /// is world state level, and another is storage level.
    pub(crate) type ProofLevel = u8;

    /// WORLDSTATE is the proof of the storage hash in world state trie.
    pub(crate) const WORLDSTATE: ProofLevel = 0x00;

    /// STORAGE is the proof of key inside smart contracts (AppKey) in storage tire.
    pub(crate) const STORAGE: ProofLevel = 0x01;
}
pub(crate) use proof_level::ProofLevel;

/// Trie node prefix is the nibble path from the trie root to the trie node. It is the leftmost portion of the node key.
/// This is used internally to build the Trie.
pub(crate) struct PrefixedTrieNodeKey<H>(PhantomData<H>);
impl<H: KeyHasher> PrefixedTrieNodeKey<H> {
    // Key function that concatenates prefix and hash.
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
