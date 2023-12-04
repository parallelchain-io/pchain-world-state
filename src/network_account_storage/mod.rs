/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! The world state data formatting scheme, as well as structs and functions associated with Network Account.
//! Network Account maintains network-wide state in ParallelChain F. This state is maintained in the Storage Trie of
//! an identified Account called the Network Account, which resides at constant address [NETWORK_ADDRESS](constants::NETWORK_ADDRESS).
//! This Account is not associated with Ed25519 material. The network-significant data that the Network Account stores
//! is composed of various fields stored in its Storage Trie.

pub mod network_account;
pub use network_account::*;

pub mod pool;
pub use pool::*;

pub mod deposit;
pub use deposit::*;

pub mod stake;
pub use stake::*;

pub(crate) mod index_heap;

pub(crate) mod index_map;

pub mod constants;
pub use constants::*;
