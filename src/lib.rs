//! `pchain-world-state` is a library to provide operations on WorldState(Account information on blockchain) and AccountStorage(Contract account storage on blockchain)
//!
//! # Example
//! ```ignore
//! // init a genesis WorldState with old version
//! let ws = WorldState::<DummyStorage, V1>::new(&storage);
//! // set nonce for account
//! ws.account_trie_mut().set_nonce(&address, 1_u64).unwrap();
//! // set <key, value> to account storage
//! ws.storage_trie_mut(&address).unwrap().set(&key, value).unwrap();
//! // close and get changes as structure WorldStateChanges
//! let ws_change = ws.close().unwrap();
//! // caller need to apply the changes provided by WorldStateChanges to physical db
//! // open WorldState after change
//! let ws_after_change = WorldState::<DummyStorage, V1>::open(&storage, ws_change.new_root_hash);
//! // get updated nonce
//! let nonce = ws_after_change.account_trie().nonce(&address);
//! // get updated account storage change
//! let value = ws_after_change.storage_trie(&address).unwrap().get(&key);
//! ```
//!
//! # Example
//! ```ignore
//! // upgrade worldstate v1 to worldstate v2
//! let ws_2 = WorldState::<DummyStorage, V1>::upgrade(ws_1);
//! // get changes during the upgrades
//! let ws_changes = ws_2.close();
//! // user need to apply the physical db change by ws_change.inserts, and ws_change.deletes
//! ```

pub mod accounts_trie;
pub use accounts_trie::*;

pub mod db;
pub use db::DB;

pub mod error;
pub use error::*;

pub mod storage_trie;
pub use storage_trie::*;

pub mod mpt;
pub use mpt::*;

pub mod world_state;
pub use world_state::*;

pub mod version;
pub use version::*;

pub mod network_account_storage;
pub use network_account_storage::*;
