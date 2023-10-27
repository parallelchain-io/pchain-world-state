/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/
//! This mod only public inside crate. Provide struct and implementation of ProofNode
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
