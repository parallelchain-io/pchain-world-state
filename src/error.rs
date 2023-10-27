/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! This mod define Errors of crate

use std::fmt::{self};

/// `WorldStateError is wraper of errors triggled inside crate`
#[derive(Debug)]
pub enum WorldStateError {
    MptError(MptError),
    TrieKeyBuildError(TrieKeyBuildError),
    DecodeOrEncodeError(DecodeOrEncodeError),
}

/// `MptError` is error from lib trie_db
#[derive(Debug, PartialEq, Eq)]
pub enum MptError {
    /// Attempted to create a trie with a state root not in the database.
    InvalidStateRoot,
    /// Trie item not found in the database,
    IncompleteDatabase,
    /// A value was found in the trie with a nibble key that was not byte-aligned.
    ValueAtIncompleteKey,
    /// Corrupt Trie item.
    DecoderError,
    /// Encoded node contains invalid hash reference.
    InvalidHash,
    /// Empty Trie
    EmptyTrie,
}

/// `TrieKeyBuildError` is error triggled when create trie logic key
#[derive(Debug, PartialEq, Eq)]
pub enum TrieKeyBuildError {
    /// Unrecognized [AccountField](crate::accounts::AccountField)
    InvalidAccountField,
    // Invalid PublicAddress
    InvalidPublicAddress,
    // other errors
    Other,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DecodeOrEncodeError {
    DecodeError,
    EncodeError,
}

impl fmt::Display for TrieKeyBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            TrieKeyBuildError::InvalidAccountField => write!(f, "Invalid AccountField"),
            TrieKeyBuildError::InvalidPublicAddress => write!(f, "Invalid PublicAddress"),
            TrieKeyBuildError::Other => write!(f, "Other errors"),
        }
    }
}

impl fmt::Display for DecodeOrEncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            DecodeOrEncodeError::DecodeError => write!(f, "Decode Error"),
            DecodeOrEncodeError::EncodeError => write!(f, "Encode Error"),
        }
    }
}

impl<T, E> From<trie_db::TrieError<T, E>> for MptError {
    fn from(err: trie_db::TrieError<T, E>) -> Self {
        match err {
            trie_db::TrieError::InvalidStateRoot(_) => MptError::InvalidStateRoot,
            trie_db::TrieError::IncompleteDatabase(_) => MptError::IncompleteDatabase,
            trie_db::TrieError::ValueAtIncompleteKey(_, _) => MptError::ValueAtIncompleteKey,
            trie_db::TrieError::DecoderError(_, _) => MptError::DecoderError,
            trie_db::TrieError::InvalidHash(_, _) => MptError::InvalidHash,
        }
    }
}
