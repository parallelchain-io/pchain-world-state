/*
    Copyright © 2023, ParallelChain Lab 
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Error handling behaviour of this crate.

/// WorldStateError enumerates the possible error of pchain_world_state::WorldState. 
#[derive(Debug)]
pub enum WorldStateError {
    // Attempted to create a trie with a state root not in the database.
	InvalidStateRoot,
	// Trie item not found in the database,
	IncompleteDatabase,
    // A value was found in the trie with a nibble key that was not byte-aligned.
	ValueAtIncompleteKey,
	// Corrupt Trie item.
	DecoderError,
	// Encoded node contains invalid hash reference.
	InvalidHash,
    // Attempted to convert protected WSKey to AppKey
    ProtectedKey
}

impl<T, E> From<trie_db::TrieError<T, E>> for WorldStateError{
    fn from(err: trie_db::TrieError<T, E>) -> Self {
        match err {
            trie_db::TrieError::InvalidStateRoot(_) => WorldStateError::InvalidStateRoot,
            trie_db::TrieError::IncompleteDatabase(_) => WorldStateError::IncompleteDatabase,
            trie_db::TrieError::ValueAtIncompleteKey(_, _) => WorldStateError::ValueAtIncompleteKey,
            trie_db::TrieError::DecoderError(_, _) => WorldStateError::DecoderError,
            trie_db::TrieError::InvalidHash(_, _) => WorldStateError::InvalidHash,
        }
    }
}