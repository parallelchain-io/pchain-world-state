/*
    Copyright © 2023, ParallelChain Lab 
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! pchain-world-state defines structs, functions, and methods for reading and writing world state from and into a persistent storage. 
//! The world state is stored in Merkle Patricia Trie structure which is the combination of a:
//! - Patricia Trie: An efficient Radix Trie (r=16), a data structure in which “keys” represent the path one has to take to reach a node
//! - Merkle Tree: A hash tree in which each node’s hash is computed from its child nodes hashes.

pub(crate) mod trie;

pub mod states;

pub mod keys;

pub mod storage;

pub mod error;

pub mod network;

#[cfg(test)]
mod tests;

