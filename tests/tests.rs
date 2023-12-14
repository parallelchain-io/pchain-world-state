/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Unit Test on functionalities on this crate
//! The test use [DummyStorage] to simulate the pysical data base.
//! [TestEnv] is a struct contains a simulate db and one account address
//! [TestEnvWithSeveralAccounts] is a struct contains a simulate db and two accounts address
//!
//! There are 10 tests currently
//! 1.  [diff_version] test build WorldState by different version, which will return different keys in WorldStateChanges.inserts
//! 2.  [update_nonce] test AccountTrie nonce operation
//! 3.  [update_balance] test AccountTrie balance operation
//! 4.  [update_code] test AccountTrie code operation
//! 5.  [update_cbi_version] test AccountTrie cbi_version operation
//! 6.  [update_storage] test StorageTrie operation
//! 7.  [iter] test AccountTrie and StorageTrie iteration
//! 8.  [search_with_proof] test AccountTrie and StorageTrie search with proof
//! 9.  [upgrade] test upgrade from WorldState Version 1 to Version 2
//! 10. [remove_storage_info] test remove <key, value > pair from StorageTrie by key

use pchain_types::cryptography::PublicAddress;
use pchain_world_state::*;
use std::collections::{HashMap, HashSet};
pub type Key = Vec<u8>;
pub type Value = Vec<u8>;

#[derive(Debug, Clone)]
struct DummyStorage(HashMap<Key, Value>);
impl DB for DummyStorage {
    fn get(&self, key: &[u8]) -> Option<Value> {
        match self.0.get(key) {
            Some(value) => Some(value.to_owned()),
            None => None,
        }
    }
}

impl DummyStorage {
    fn apply_changes(&mut self, inserts: HashMap<Vec<u8>, Vec<u8>>, deletes: HashSet<Vec<u8>>) {
        for (key, value) in inserts.into_iter() {
            self.0.insert(key, value);
        }
        for key in deletes.into_iter() {
            self.0.remove(&key);
        }
    }
}

#[derive(Debug, Clone)]
struct TestEnv {
    db: DummyStorage,
    address: PublicAddress,
}
impl Default for TestEnv {
    fn default() -> Self {
        let db = DummyStorage(HashMap::new());
        const PUBLIC_KEY: &str = "ipy_VXNiwHNP9mx6-nKxht_ZJNfYoMAcCnLykpq4x_k";
        let address = base64url::decode(PUBLIC_KEY).unwrap().try_into().unwrap();
        Self { db, address }
    }
}

#[derive(Debug, Clone)]
struct TestEnvWithSeveralAccounts {
    db: DummyStorage,
    addresses: Vec<PublicAddress>,
}

impl Default for TestEnvWithSeveralAccounts {
    fn default() -> Self {
        let db = DummyStorage(HashMap::new());
        const PUBLIC_KEY_ONE: &str = "ipy_VXNiwHNP9mx6-nKxht_ZJNfYoMAcCnLykpq4x_k";
        const PUBLIC_KEY_TWO: &str = "l1RjOEHtM-RvUh7BxmCBgu33Pw1vqb8AgKJLMLqz3js";
        let address_one = base64url::decode(PUBLIC_KEY_ONE)
            .unwrap()
            .try_into()
            .unwrap();
        let address_two = base64url::decode(PUBLIC_KEY_TWO)
            .unwrap()
            .try_into()
            .unwrap();
        let mut addresses = Vec::new();
        addresses.push(address_one);
        addresses.push(address_two);
        TestEnvWithSeveralAccounts { db, addresses }
    }
}

#[test]
fn diff_version() {
    let env_1 = TestEnv::default();
    let env_2 = TestEnv::default();
    let mut ws_1 = WorldState::<DummyStorage, V1>::new(&env_1.db);
    let mut ws_2 = WorldState::<DummyStorage, V2>::new(&env_2.db);
    let ws_change_1 = ws_1.close().unwrap();
    let ws_change_2 = ws_2.close().unwrap();
    let key_1 = ws_change_1.inserts.into_iter().next().unwrap().0;
    let key_2 = ws_change_2.inserts.into_iter().next().unwrap().0;
    assert_ne!(key_1, key_2);
}

#[test]
fn update_nonce() {
    // ================ Version1 ================
    let mut env_1 = TestEnv::default();
    let mut genesis_ws_v1 = WorldState::<DummyStorage, V1>::new(&env_1.db);
    assert_eq!(
        genesis_ws_v1.account_trie().nonce(&env_1.address).unwrap(),
        0_u64
    );
    genesis_ws_v1
        .account_trie_mut()
        .set_nonce(&(env_1.address.clone()), 1_u64)
        .unwrap();
    let ws_changes_v1 = genesis_ws_v1.close().unwrap();
    env_1
        .db
        .apply_changes(ws_changes_v1.inserts, ws_changes_v1.deletes);
    let new_ws_v1 = WorldState::<DummyStorage, V1>::open(&env_1.db, ws_changes_v1.new_root_hash);
    assert!(new_ws_v1
        .account_trie()
        .contains_nonce(&env_1.address)
        .unwrap(),);
    assert_eq!(
        new_ws_v1.account_trie().nonce(&env_1.address).unwrap(),
        1_u64
    );
    // ================ Version2 ================
    let mut env_2 = TestEnv::default();
    let mut genesis_ws_v2 = WorldState::<DummyStorage, V2>::new(&env_2.db);
    assert_eq!(
        genesis_ws_v2.account_trie().nonce(&env_2.address).unwrap(),
        0_u64
    );
    genesis_ws_v2
        .account_trie_mut()
        .set_nonce(&env_2.address, 1_u64)
        .unwrap();
    let ws_changes_v2 = genesis_ws_v2.close().unwrap();
    env_2
        .db
        .apply_changes(ws_changes_v2.inserts, ws_changes_v2.deletes);
    let new_ws_v2 = WorldState::<DummyStorage, V2>::open(&env_2.db, ws_changes_v2.new_root_hash);
    assert!(new_ws_v2
        .account_trie()
        .contains_nonce(&env_2.address)
        .unwrap(),);
    assert_eq!(
        new_ws_v2.account_trie().nonce(&env_2.address).unwrap(),
        1_u64
    );
}

#[test]
fn update_balance() {
    //================ Version1 ================
    let mut env_1 = TestEnv::default();
    let mut genesis_ws_1 = WorldState::<DummyStorage, V1>::new(&env_1.db);
    assert_eq!(
        genesis_ws_1.account_trie().balance(&env_1.address).unwrap(),
        0_u64
    );
    genesis_ws_1
        .account_trie_mut()
        .set_balance(&env_1.address, 100_000_u64)
        .unwrap();
    let ws_change_v1 = genesis_ws_1.close().unwrap();
    env_1
        .db
        .apply_changes(ws_change_v1.inserts, ws_change_v1.deletes);
    let new_ws_v1 = WorldState::<DummyStorage, V1>::open(&env_1.db, ws_change_v1.new_root_hash);
    assert_eq!(
        new_ws_v1.account_trie().balance(&env_1.address).unwrap(),
        100_000_u64
    );
    //================ Version2 ================
    let mut env_2 = TestEnv::default();
    let mut genesis_ws_2 = WorldState::<DummyStorage, V2>::new(&env_2.db);
    assert_eq!(
        genesis_ws_2.account_trie().balance(&env_2.address).unwrap(),
        0_u64
    );
    genesis_ws_2
        .account_trie_mut()
        .set_balance(&env_2.address, 100_000_u64)
        .unwrap();
    let ws_change_v2 = genesis_ws_2.close().unwrap();
    env_2
        .db
        .apply_changes(ws_change_v2.inserts, ws_change_v2.deletes);
    let new_ws_v2 = WorldState::<DummyStorage, V2>::open(&env_2.db, ws_change_v2.new_root_hash);
    assert_eq!(
        new_ws_v2.account_trie().balance(&env_2.address).unwrap(),
        100_000_u64
    );
}

#[test]
fn update_code() {
    //================ Version1 ================
    let code_str = "Hello world";
    let code_vec = code_str.as_bytes().to_vec();
    let mut env_1 = TestEnv::default();
    let mut genesis_ws_v1 = WorldState::<DummyStorage, V1>::new(&env_1.db);
    assert!(genesis_ws_v1
        .account_trie()
        .code(&env_1.address)
        .unwrap()
        .is_none());

    genesis_ws_v1
        .account_trie_mut()
        .set_code(&env_1.address, code_vec.clone())
        .unwrap();
    let ws_change_v1 = genesis_ws_v1.close().unwrap();
    env_1
        .db
        .apply_changes(ws_change_v1.inserts, ws_change_v1.deletes);
    let new_ws_v1 = WorldState::<DummyStorage, V1>::open(&env_1.db, ws_change_v1.new_root_hash);
    assert_eq!(
        std::str::from_utf8(
            &new_ws_v1
                .account_trie()
                .code(&env_1.address)
                .unwrap()
                .unwrap()
        )
        .unwrap(),
        "Hello world"
    );
    //================ Version2 ================
    let mut env_2 = TestEnv::default();
    let mut genesis_ws_v2 = WorldState::<DummyStorage, V2>::new(&env_2.db);
    assert!(genesis_ws_v2
        .account_trie()
        .code(&env_2.address)
        .unwrap()
        .is_none());

    genesis_ws_v2
        .account_trie_mut()
        .set_code(&env_2.address, code_vec.clone())
        .unwrap();
    let ws_change_v2 = genesis_ws_v2.close().unwrap();
    env_2
        .db
        .apply_changes(ws_change_v2.inserts, ws_change_v2.deletes);
    let new_ws_v2 = WorldState::<DummyStorage, V2>::open(&env_2.db, ws_change_v2.new_root_hash);
    assert_eq!(
        std::str::from_utf8(
            &new_ws_v2
                .account_trie()
                .code(&env_2.address)
                .unwrap()
                .unwrap()
        )
        .unwrap(),
        "Hello world"
    );
}

#[test]
pub fn update_cbi_version() {
    //================ Version1 ================
    let mut env_1 = TestEnv::default();
    let mut genesis_ws_v1 = WorldState::<DummyStorage, V1>::new(&env_1.db);
    assert_eq!(
        genesis_ws_v1
            .account_trie()
            .cbi_version(&env_1.address)
            .unwrap(),
        None
    );
    genesis_ws_v1
        .account_trie_mut()
        .set_cbi_version(&env_1.address, 1_u32)
        .unwrap();
    let ws_changes_v1 = genesis_ws_v1.close().unwrap();
    env_1
        .db
        .apply_changes(ws_changes_v1.inserts, ws_changes_v1.deletes);
    let new_ws_v1 = WorldState::<DummyStorage, V1>::open(&env_1.db, ws_changes_v1.new_root_hash);
    assert_eq!(
        new_ws_v1
            .account_trie()
            .cbi_version(&env_1.address)
            .unwrap(),
        Some(1_u32)
    );
    //================ Version2 ================
    let mut env_2 = TestEnv::default();
    let mut genesis_ws_v2 = WorldState::<DummyStorage, V2>::new(&env_2.db);
    assert_eq!(
        genesis_ws_v2
            .account_trie()
            .cbi_version(&env_2.address)
            .unwrap(),
        None
    );
    genesis_ws_v2
        .account_trie_mut()
        .set_cbi_version(&env_2.address, 1_u32)
        .unwrap();
    let ws_changes_v2 = genesis_ws_v2.close().unwrap();
    env_2
        .db
        .apply_changes(ws_changes_v2.inserts, ws_changes_v2.deletes);
    let new_ws_v2 = WorldState::<DummyStorage, V2>::open(&env_2.db, ws_changes_v2.new_root_hash);
    assert_eq!(
        new_ws_v2
            .account_trie()
            .cbi_version(&env_2.address)
            .unwrap(),
        Some(1_u32)
    );
}

#[test]
pub fn update_storage() {
    //================ Version1 ================
    let key_apple: Key = [
        4, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
        1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
        2, 2, 2, 3, 4, 5, 6, 7,
    ]
    .to_vec();
    let value_apple: Value = b"1234".to_vec();
    let key_banana: Key = b"banana".to_vec();
    let value_banana: Value = b"12345".to_vec();
    let mut env_1 = TestEnv::default();
    let mut genesis_ws_v1 = WorldState::<DummyStorage, V1>::new(&env_1.db);
    let storage_trie_unmut_ref = genesis_ws_v1.storage_trie(&env_1.address).unwrap();
    assert!(storage_trie_unmut_ref.get(&key_apple).unwrap().is_none());
    assert!(storage_trie_unmut_ref.get(&key_banana).unwrap().is_none());
    let storage_trie_mut_ref = genesis_ws_v1.storage_trie_mut(&env_1.address).unwrap();
    storage_trie_mut_ref
        .set(&key_banana, value_banana.clone())
        .unwrap();
    storage_trie_mut_ref
        .set(&key_apple, value_apple.clone())
        .unwrap();
    let ws_change_v1 = genesis_ws_v1.close().unwrap();
    env_1
        .db
        .apply_changes(ws_change_v1.inserts, ws_change_v1.deletes);
    let mut new_ws_v1 = WorldState::<DummyStorage, V1>::open(&env_1.db, ws_change_v1.new_root_hash);
    let storage_trie_unmut_ref = new_ws_v1.storage_trie(&env_1.address).unwrap();
    assert!(storage_trie_unmut_ref.contains(&key_apple).unwrap());
    assert!(storage_trie_unmut_ref.contains(&key_banana).unwrap());
    assert_eq!(
        storage_trie_unmut_ref.get(&key_apple).unwrap().unwrap(),
        value_apple
    );
    assert_eq!(
        storage_trie_unmut_ref.get(&key_banana).unwrap().unwrap(),
        value_banana
    );
    //================ Version2 ================
    let mut env_2 = TestEnv::default();
    let mut genesis_ws_v2 = WorldState::<DummyStorage, V2>::new(&env_2.db);
    let storage_trie_unmut_ref = genesis_ws_v2.storage_trie(&env_2.address).unwrap();
    assert!(storage_trie_unmut_ref.get(&key_apple).unwrap().is_none());
    assert!(storage_trie_unmut_ref.get(&key_banana).unwrap().is_none());
    genesis_ws_v2
        .storage_trie_mut(&env_2.address)
        .unwrap()
        .set(&key_apple, value_apple.clone())
        .unwrap();
    genesis_ws_v2
        .storage_trie_mut(&env_2.address)
        .unwrap()
        .set(&key_banana, value_banana.clone())
        .unwrap();
    let ws_change_v2 = genesis_ws_v2.close().unwrap();
    env_2
        .db
        .apply_changes(ws_change_v2.inserts, ws_change_v2.deletes);
    let mut new_ws_v2 = WorldState::<DummyStorage, V2>::open(&env_2.db, ws_change_v2.new_root_hash);
    let storage_trie_unmut_ref = new_ws_v2.storage_trie(&env_2.address).unwrap();
    assert!(storage_trie_unmut_ref.contains(&key_apple).unwrap());
    assert!(storage_trie_unmut_ref.contains(&key_banana).unwrap());
    assert_eq!(
        storage_trie_unmut_ref.get(&key_apple).unwrap().unwrap(),
        value_apple
    );
    assert_eq!(
        storage_trie_unmut_ref.get(&key_banana).unwrap().unwrap(),
        value_banana
    );
}

#[test]
pub fn iter() {
    // variables
    let code_str = "Hello world";
    let code_vec = code_str.as_bytes().to_vec();
    let key_apple: Key = [
        4, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
        1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
        2, 2, 2, 3, 4, 5, 6, 7,
    ]
    .to_vec();
    let value_apple: Value = b"1234".to_vec();
    let key_banana: Key = b"banana".to_vec();
    let value_banana: Value = b"12345".to_vec();

    //================ Version1 ================
    // init trie
    let mut env_1 = TestEnvWithSeveralAccounts::default();
    let mut genesis_ws_v1 = WorldState::<DummyStorage, V1>::new(&env_1.db);
    // update account info
    let account_trie_mut_ref = genesis_ws_v1.account_trie_mut();
    account_trie_mut_ref
        .set_nonce(env_1.addresses.get(0).unwrap(), 1_u64)
        .unwrap();
    account_trie_mut_ref
        .set_balance(env_1.addresses.get(0).unwrap(), 100_000_u64)
        .unwrap();
    account_trie_mut_ref
        .set_code(env_1.addresses.get(0).unwrap(), code_vec.clone())
        .unwrap();
    account_trie_mut_ref
        .set_cbi_version(env_1.addresses.get(0).unwrap(), 1_u32)
        .unwrap();
    account_trie_mut_ref
        .set_nonce(env_1.addresses.get(1).unwrap(), 2_u64)
        .unwrap();
    account_trie_mut_ref
        .set_balance(env_1.addresses.get(1).unwrap(), 200_000_u64)
        .unwrap();
    account_trie_mut_ref
        .set_code(env_1.addresses.get(1).unwrap(), code_vec.clone())
        .unwrap();
    account_trie_mut_ref
        .set_cbi_version(env_1.addresses.get(1).unwrap(), 2_u32)
        .unwrap();
    // update storage for account 1
    let storage_trie_mut_ref = genesis_ws_v1
        .storage_trie_mut(env_1.addresses.get(0).unwrap())
        .unwrap();
    storage_trie_mut_ref
        .set(&key_apple, value_apple.clone())
        .unwrap();
    storage_trie_mut_ref
        .set(&key_banana, value_banana.clone())
        .unwrap();
    // close change
    let ws_changes_v1 = genesis_ws_v1.close().unwrap();
    env_1
        .db
        .apply_changes(ws_changes_v1.inserts, ws_changes_v1.deletes);
    // check
    let mut new_ws_v1 =
        WorldState::<DummyStorage, V1>::open(&env_1.db, ws_changes_v1.new_root_hash);
    // iter account trie
    let account_iter = new_ws_v1.account_trie().all().unwrap();
    account_iter.into_iter().for_each(|(key, value)| {
        println!(
            "account_address: {:?},
        account_nonce: {:?},
        account_balance: {:?},
        account_cbi_version: {:?},
        account_code: {:?},
        account_storage_hash: {:?}",
            key,
            value.nonce,
            value.balance,
            value.cbi_version,
            value.code,
            value.storage_hash(),
        )
    });
    // iter storage trie
    let storage_trie_ref = new_ws_v1
        .storage_trie(env_1.addresses.get(0).unwrap())
        .unwrap();
    assert!(storage_trie_ref.contains(&key_apple).unwrap());
    assert_eq!(
        storage_trie_ref.get(&key_apple).unwrap(),
        Some(value_apple.clone())
    );
    assert!(storage_trie_ref.contains(&key_banana).unwrap());
    assert_eq!(
        storage_trie_ref.get(&key_banana).unwrap(),
        Some(value_banana.clone())
    );

    //================ Version2 ================
    // init trie
    let mut env_2 = TestEnvWithSeveralAccounts::default();
    let mut genesis_ws_v2 = WorldState::<DummyStorage, V2>::new(&env_2.db);
    // update account info
    let account_trie_mut_ref = genesis_ws_v2.account_trie_mut();
    account_trie_mut_ref
        .set_nonce(env_2.addresses.get(0).unwrap(), 1_u64)
        .unwrap();
    account_trie_mut_ref
        .set_balance(env_2.addresses.get(0).unwrap(), 100_000_u64)
        .unwrap();
    account_trie_mut_ref
        .set_code(env_2.addresses.get(0).unwrap(), code_vec.clone())
        .unwrap();
    account_trie_mut_ref
        .set_cbi_version(env_2.addresses.get(0).unwrap(), 1_u32)
        .unwrap();
    account_trie_mut_ref
        .set_nonce(env_2.addresses.get(1).unwrap(), 2_u64)
        .unwrap();
    account_trie_mut_ref
        .set_balance(env_2.addresses.get(1).unwrap(), 200_000_u64)
        .unwrap();
    account_trie_mut_ref
        .set_code(env_2.addresses.get(1).unwrap(), code_vec.clone())
        .unwrap();
    account_trie_mut_ref
        .set_cbi_version(env_2.addresses.get(1).unwrap(), 2_u32)
        .unwrap();
    // update storage for account 1
    let storage_trie_mut_ref = genesis_ws_v2
        .storage_trie_mut(env_2.addresses.get(0).unwrap())
        .unwrap();
    storage_trie_mut_ref
        .set(&key_apple, value_apple.clone())
        .unwrap();
    storage_trie_mut_ref
        .set(&key_banana, value_banana.clone())
        .unwrap();
    // close change
    let ws_changes_v2 = genesis_ws_v2.close().unwrap();
    env_2
        .db
        .apply_changes(ws_changes_v2.inserts, ws_changes_v2.deletes);
    // check
    let mut new_ws_v2 =
        WorldState::<DummyStorage, V2>::open(&env_2.db, ws_changes_v2.new_root_hash);
    // iter account trie
    let account_iter = new_ws_v2.account_trie().all();
    account_iter.unwrap().into_iter().for_each(|(key, value)| {
        println!(
            "account_address: {:?},
        account_nonce: {:?},
        account_balance: {:?},
        account_cbi_version: {:?},
        account_code: {:?},
        account_storage_hash: {:?}",
            key,
            value.nonce,
            value.balance,
            value.cbi_version,
            value.code,
            value.storage_hash()
        )
    });
    // iter storage trie
    let storage_trie_ref = new_ws_v2
        .storage_trie(env_2.addresses.get(0).unwrap())
        .unwrap();
    assert!(storage_trie_ref.contains(&key_apple).unwrap());
    assert!(storage_trie_ref.contains(&key_banana).unwrap());
}

#[test]
pub fn search_with_proof() {
    let key_apple: Key = [
        4, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
        1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
        2, 2, 2,
    ]
    .to_vec();
    let value_apple: Value = b"1234".to_vec();
    let code_str = "Hello world";
    let code_vec = code_str.as_bytes().to_vec();
    //================ Version1 ================
    let mut env_1 = TestEnv::default();
    let mut genesis_ws_v1 = WorldState::<DummyStorage, V1>::new(&env_1.db);
    let account_trie_mut_ref = genesis_ws_v1.account_trie_mut();
    account_trie_mut_ref.set_nonce(&env_1.address, 1).unwrap();
    account_trie_mut_ref
        .set_balance(&env_1.address, 100_000_u64)
        .unwrap();
    account_trie_mut_ref
        .set_code(&env_1.address, code_vec.clone())
        .unwrap();
    account_trie_mut_ref
        .set_cbi_version(&env_1.address, 1_u32)
        .unwrap();
    let storage_trie_mut_ref = genesis_ws_v1.storage_trie_mut(&env_1.address).unwrap();
    storage_trie_mut_ref
        .set(&key_apple, value_apple.clone())
        .unwrap();
    let ws_change_v1 = genesis_ws_v1.close().unwrap();
    env_1
        .db
        .apply_changes(ws_change_v1.inserts, ws_change_v1.deletes);
    let mut new_ws_v1 = WorldState::<DummyStorage, V1>::open(&env_1.db, ws_change_v1.new_root_hash);
    let account_trie_unmut_ref = new_ws_v1.account_trie();
    println!(
        "proof: {:?}",
        account_trie_unmut_ref
            .nonce_with_proof(&env_1.address)
            .unwrap()
            .0
    );
    assert_eq!(
        account_trie_unmut_ref
            .nonce_with_proof(&env_1.address)
            .unwrap()
            .1,
        1_u64
    );
    assert_eq!(
        account_trie_unmut_ref
            .balance_with_proof(&env_1.address)
            .unwrap()
            .1,
        100_000_u64
    );
    assert_eq!(
        std::str::from_utf8(
            account_trie_unmut_ref
                .code_with_proof(&env_1.address)
                .unwrap()
                .1
                .unwrap()
                .as_ref()
        )
        .unwrap(),
        code_str
    );
    assert_eq!(
        account_trie_unmut_ref
            .cbi_version_with_proof(&env_1.address)
            .unwrap()
            .1,
        Some(1_u32)
    );
    println!(
        "proof of storage hash {:?}",
        account_trie_unmut_ref
            .storage_hash_with_proof(&env_1.address)
            .unwrap()
            .0
    );
    let storage_trie_unmut_ref = new_ws_v1.storage_trie(&env_1.address).unwrap();
    println!(
        "proof: {:?}",
        storage_trie_unmut_ref.get_with_proof(&key_apple).unwrap().0
    );
    assert_eq!(
        storage_trie_unmut_ref
            .get_with_proof(&key_apple)
            .unwrap()
            .1
            .unwrap(),
        value_apple
    );
    //================ Version2 ================.
    let mut env_2 = TestEnv::default();
    let mut genesis_ws_v2 = WorldState::<DummyStorage, V2>::new(&env_2.db);
    let account_trie_mut_ref = genesis_ws_v2.account_trie_mut();
    account_trie_mut_ref.set_nonce(&env_2.address, 1).unwrap();
    account_trie_mut_ref
        .set_balance(&env_2.address, 100_000_u64)
        .unwrap();
    account_trie_mut_ref
        .set_code(&env_2.address, code_vec)
        .unwrap();
    account_trie_mut_ref
        .set_cbi_version(&env_2.address, 1_u32)
        .unwrap();
    let storage_trie_mut_ref = genesis_ws_v2.storage_trie_mut(&env_2.address).unwrap();
    storage_trie_mut_ref
        .set(&key_apple, value_apple.clone())
        .unwrap();
    let ws_change_v2 = genesis_ws_v2.close().unwrap();
    env_2
        .db
        .apply_changes(ws_change_v2.inserts, ws_change_v2.deletes);
    let mut new_ws_v2 = WorldState::<DummyStorage, V2>::open(&env_2.db, ws_change_v2.new_root_hash);
    let account_trie_unmut_ref = new_ws_v2.account_trie();
    println!(
        "proof: {:?}",
        account_trie_unmut_ref
            .nonce_with_proof(&env_2.address)
            .unwrap()
            .0
    );
    assert_eq!(
        account_trie_unmut_ref
            .nonce_with_proof(&env_2.address)
            .unwrap()
            .1,
        1_u64
    );
    assert_eq!(
        account_trie_unmut_ref
            .nonce_with_proof(&env_2.address)
            .unwrap()
            .1,
        1_u64
    );
    assert_eq!(
        account_trie_unmut_ref
            .balance_with_proof(&env_2.address)
            .unwrap()
            .1,
        100_000_u64
    );
    assert_eq!(
        std::str::from_utf8(
            account_trie_unmut_ref
                .code_with_proof(&env_2.address)
                .unwrap()
                .1
                .unwrap()
                .as_ref()
        )
        .unwrap(),
        code_str
    );
    assert_eq!(
        account_trie_unmut_ref
            .cbi_version_with_proof(&env_2.address)
            .unwrap()
            .1,
        Some(1_u32)
    );
    println!(
        "proof of storage hash {:?}",
        account_trie_unmut_ref
            .storage_hash_with_proof(&env_2.address)
            .unwrap()
            .0
    );
    let storage_trie_unmut_ref = new_ws_v2.storage_trie(&env_2.address).unwrap();
    println!(
        "proof: {:?}",
        storage_trie_unmut_ref.get_with_proof(&key_apple).unwrap().0
    );
    assert_eq!(
        storage_trie_unmut_ref
            .get_with_proof(&key_apple)
            .unwrap()
            .1
            .unwrap(),
        value_apple
    );
}

#[test]
pub fn upgrade() {
    let code_str = "Hello world";
    let code_vec = code_str.as_bytes().to_vec();
    let key_apple: Key = [
        4, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
        1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
        2, 2, 2,
    ]
    .to_vec();
    let value_apple: Value = b"1234".to_vec();
    let key_banana: Key = b"banana".to_vec();
    let value_banana: Value = b"12345".to_vec();

    // init account trie with 2 accounts
    let mut env = TestEnvWithSeveralAccounts::default();
    let mut ws_1 = WorldState::<DummyStorage, V1>::new(&env.db);
    // account 1
    let account_trie_mut_ref = ws_1.account_trie_mut();
    account_trie_mut_ref
        .set_nonce(&env.addresses.get(0).unwrap(), 1_u64)
        .unwrap();
    account_trie_mut_ref
        .set_balance(&env.addresses.get(0).unwrap(), 100_000_u64)
        .unwrap();
    account_trie_mut_ref
        .set_cbi_version(&env.addresses.get(0).unwrap(), 1_u32)
        .unwrap();
    account_trie_mut_ref
        .set_code(&env.addresses.get(0).unwrap(), code_vec.clone())
        .unwrap();
    // account 2
    account_trie_mut_ref
        .set_nonce(&env.addresses.get(1).unwrap(), 1_u64)
        .unwrap();
    account_trie_mut_ref
        .set_balance(&env.addresses.get(1).unwrap(), 200_000_u64)
        .unwrap();
    account_trie_mut_ref
        .set_cbi_version(&env.addresses.get(1).unwrap(), 2_u32)
        .unwrap();
    account_trie_mut_ref
        .set_code(&env.addresses.get(1).unwrap(), code_vec.clone())
        .unwrap();
    // storage 1
    let storage_trie_mut_ref = ws_1
        .storage_trie_mut(env.addresses.get(0).unwrap())
        .unwrap();
    storage_trie_mut_ref
        .set(&key_apple, value_apple.clone())
        .unwrap();
    storage_trie_mut_ref
        .set(&key_banana, value_banana.clone())
        .unwrap();
    // close changes
    let ws_change_1 = ws_1.close().unwrap();
    env.db
        .apply_changes(ws_change_1.inserts, ws_change_1.deletes);
    // iter the AccountTrie and StorageTrie
    let mut new_ws_1 = WorldState::<DummyStorage, V1>::open(&env.db, ws_change_1.new_root_hash);
    let accounts_trie_ref = new_ws_1.account_trie();
    println!("=======================iter account trie after insertion==========================");
    accounts_trie_ref
        .all()
        .unwrap()
        .into_iter()
        .for_each(|(key, value)| println!("account_address: {:?}, account_info: {:?}", key, value));
    println!("=======================test storage trie after insertion==========================");
    let storage_trie_ref = new_ws_1
        .storage_trie(&env.addresses.get(0).unwrap())
        .unwrap();
    assert!(storage_trie_ref.contains(&key_apple).unwrap());
    assert_eq!(
        storage_trie_ref.get(&key_apple).unwrap(),
        Some(value_apple.clone())
    );
    assert!(storage_trie_ref.contains(&key_banana).unwrap());
    assert_eq!(
        storage_trie_ref.get(&key_banana).unwrap(),
        Some(value_banana.clone())
    );
    println!("======================db after insertion =================================");
    println!("{:?}", &env.db);
    // upgrade

    let mut ws_2 = new_ws_1.upgrade().unwrap();

    let ws_2_changes = ws_2.close().unwrap();
    env.db
        .apply_changes(ws_2_changes.inserts, ws_2_changes.deletes);
    // open ws_2
    let mut ws_2 = WorldState::<DummyStorage, V2>::open(&env.db, ws_2_changes.new_root_hash);
    let accounts_trie_ref = ws_2.account_trie();
    println!("=======================iter account trie after upgrade==========================");
    accounts_trie_ref
        .all()
        .unwrap()
        .into_iter()
        .for_each(|(key, value)| println!("account_address: {:?}, account_info: {:?}", key, value));
    println!("=======================test storage trie after upgrade==========================");
    let storage_trie_ref = ws_2.storage_trie(&env.addresses.get(0).unwrap()).unwrap();
    assert!(storage_trie_ref.contains(&key_apple).unwrap());
    assert_eq!(
        storage_trie_ref.get(&key_apple).unwrap(),
        Some(value_apple.clone())
    );
    assert!(storage_trie_ref.contains(&key_banana).unwrap());
    assert_eq!(
        storage_trie_ref.get(&key_banana).unwrap(),
        Some(value_banana.clone())
    );
    println!("======================db after upgrade =================================");
    println!("{:?}", &env.db);
}

#[test]
pub fn remove_storage_info() {
    // variables
    let key_apple: Key = b"apple".to_vec();
    let value_apple: Value = b"1234".to_vec();
    let key_banana: Key = b"banana".to_vec();
    let value_banana: Value = b"12345".to_vec();

    //================ Version1 ================
    // init trie
    let mut env_1 = TestEnv::default();
    let mut genesis_ws_v1 = WorldState::<DummyStorage, V1>::new(&env_1.db);
    let storage_trie_mut_ref = genesis_ws_v1.storage_trie_mut(&env_1.address).unwrap();
    storage_trie_mut_ref
        .set(&key_apple, value_apple.clone())
        .unwrap();
    storage_trie_mut_ref
        .set(&key_banana, value_banana.clone())
        .unwrap();
    // close change
    let ws_changes_v1 = genesis_ws_v1.close().unwrap();
    env_1
        .db
        .apply_changes(ws_changes_v1.inserts, ws_changes_v1.deletes);
    // check
    let mut new_ws_v1 =
        WorldState::<DummyStorage, V1>::open(&env_1.db, ws_changes_v1.new_root_hash);
    println!("======Account info=======");
    new_ws_v1
        .account_trie()
        .all()
        .unwrap()
        .into_iter()
        .for_each(|(key, value)| println!("account_address: {:?}, account_info: {:?}", key, value));
    println!("======Account Storage <Key, value>=======");
    let storage_trie_ref = new_ws_v1.storage_trie(&env_1.address).unwrap();
    assert!(storage_trie_ref.contains(&key_apple).unwrap());
    assert_eq!(
        storage_trie_ref.get(&key_apple).unwrap(),
        Some(value_apple.clone())
    );
    assert!(storage_trie_ref.contains(&key_banana).unwrap());
    assert_eq!(
        storage_trie_ref.get(&key_banana).unwrap(),
        Some(value_banana.clone())
    );
    // remove storage
    new_ws_v1
        .storage_trie_mut(&env_1.address)
        .unwrap()
        .remove_trie()
        .unwrap();
    let ws_changes_v1 = new_ws_v1.close().unwrap();
    env_1
        .db
        .apply_changes(ws_changes_v1.inserts, ws_changes_v1.deletes);
    // check
    let mut ws_after_delete_v1 =
        WorldState::<DummyStorage, V1>::open(&env_1.db, ws_changes_v1.new_root_hash);
    println!("======Account info=======");
    ws_after_delete_v1
        .account_trie()
        .all()
        .unwrap()
        .into_iter()
        .for_each(|(key, value)| println!("account_address: {:?}, account_info: {:?}", key, value));
    println!("======Account Storage <Key, value>=======");
    let storage_trie_ref = ws_after_delete_v1.storage_trie(&env_1.address).unwrap();
    assert!(!storage_trie_ref.contains(&key_apple).unwrap());
    assert!(!storage_trie_ref.contains(&key_banana).unwrap());
    //================ Version2 ================
    // init trie
    let mut env_2 = TestEnv::default();
    let mut genesis_ws_v2 = WorldState::<DummyStorage, V2>::new(&env_2.db);
    let storage_trie_mut_ref = genesis_ws_v2.storage_trie_mut(&env_2.address).unwrap();
    storage_trie_mut_ref
        .set(&key_apple, value_apple.clone())
        .unwrap();
    storage_trie_mut_ref
        .set(&key_banana, value_banana.clone())
        .unwrap();
    // close change
    let ws_changes_v2 = genesis_ws_v2.close().unwrap();
    env_2
        .db
        .apply_changes(ws_changes_v2.inserts, ws_changes_v2.deletes);
    // check
    let mut new_ws_v2 =
        WorldState::<DummyStorage, V2>::open(&env_2.db, ws_changes_v2.new_root_hash);
    println!("======Account info=======");
    new_ws_v2
        .account_trie()
        .all()
        .unwrap()
        .into_iter()
        .for_each(|(key, value)| println!("account_address: {:?}, account_info: {:?}", key, value));
    println!("======Account Storage <Key, value> test=======");
    let storage_trie_ref = new_ws_v2.storage_trie(&env_2.address).unwrap();
    assert!(storage_trie_ref.contains(&key_apple).unwrap());
    assert_eq!(storage_trie_ref.get(&key_apple).unwrap(), Some(value_apple));
    assert!(storage_trie_ref.contains(&key_banana).unwrap());
    assert_eq!(
        storage_trie_ref.get(&key_banana).unwrap(),
        Some(value_banana)
    );
    // remove storage
    new_ws_v2
        .storage_trie_mut(&env_2.address)
        .unwrap()
        .remove_trie()
        .unwrap();
    let ws_changes_v2 = new_ws_v2.close().unwrap();
    env_2
        .db
        .apply_changes(ws_changes_v2.inserts, ws_changes_v2.deletes);
    // check
    let mut ws_after_delete_v2 =
        WorldState::<DummyStorage, V2>::open(&env_2.db, ws_changes_v2.new_root_hash);
    println!("======Account info=======");
    ws_after_delete_v2
        .account_trie()
        .all()
        .unwrap()
        .into_iter()
        .for_each(|(key, value)| println!("account_address: {:?}, account_info: {:?}", key, value));
    println!("======Account Storage <Key, value> test=======");
    let storage_trie_ref = ws_after_delete_v2.storage_trie(&env_1.address).unwrap();
    assert!(!storage_trie_ref.contains(&key_apple).unwrap());
    assert!(!storage_trie_ref.contains(&key_banana).unwrap());
}

/// The following tests are for network functions
///
/// This part does not change during upgrading to protocal V0.5
pub struct StorageWorldState<'a, S, V>
where
    S: DB + Send + Sync + Clone,
    V: VersionProvider + Send + Sync + Clone,
{
    inner: WorldState<'a, S, V>,
}

impl<'a, S, V> StorageWorldState<'a, S, V>
where
    S: DB + Send + Sync + Clone,
    V: VersionProvider + Send + Sync + Clone,
{
    fn initialize(storage: &'a S) -> Self {
        Self {
            inner: WorldState::<'a, S, V>::new(storage),
        }
    }
}

impl<'a, S, V> NetworkAccountStorage for StorageWorldState<'a, S, V>
where
    S: DB + Send + Sync + Clone,
    V: VersionProvider + Send + Sync + Clone,
{
    fn get(&mut self, key: &[u8]) -> Option<Vec<u8>> {
        let key = key.to_vec();
        self.inner
            .storage_trie(&constants::NETWORK_ADDRESS)
            .unwrap()
            .get(&key)
            .unwrap()
    }

    fn contains(&mut self, key: &[u8]) -> bool {
        let key = key.to_vec();

        self.inner
            .storage_trie(&constants::NETWORK_ADDRESS)
            .unwrap()
            .contains(&key)
            .unwrap()
    }

    fn set(&mut self, key: &[u8], value: Vec<u8>) {
        let key = key.to_vec();
        self.inner
            .storage_trie_mut(&constants::NETWORK_ADDRESS)
            .unwrap()
            .set(&key, value)
            .unwrap();
    }

    fn delete(&mut self, key: &[u8]) {
        let key = key.to_vec();
        self.inner
            .storage_trie_mut(&constants::NETWORK_ADDRESS)
            .unwrap()
            .set(&key, Vec::new())
            .unwrap();
    }
}

#[test]
fn test_network_account() {
    let env = TestEnv::default();
    let mut ws = StorageWorldState::<DummyStorage, V2>::initialize(&env.db);

    // No values are set at initialization
    assert_eq!(NetworkAccount::new(&mut ws).current_epoch(), 0);
    assert_eq!(NetworkAccount::pvp(&mut ws).length(), 0);
    assert_eq!(NetworkAccount::vp(&mut ws).length(), 0);
    assert_eq!(NetworkAccount::nvp(&mut ws).length(), 0);

    let mut network_account = NetworkAccount::new(&mut ws);
    network_account.set_current_epoch(1);
    assert_eq!(network_account.current_epoch(), 1);

    // Check Pool
    let mut pool = NetworkAccount::pools(&mut ws, [1u8; 32]);
    assert!(pool.power().is_none());
    pool.set_power(123);
    assert_eq!(pool.power().unwrap(), 123_u64);

    let mut pool = NetworkAccount::pools(&mut ws, [1u8; 32]);
    pool.set_commission_rate(1);
    pool.set_operator([1u8; 32]);

    assert_eq!(pool.operator().unwrap(), [1u8; 32]);
    assert_eq!(pool.power().unwrap(), 123);
    assert_eq!(pool.commission_rate().unwrap(), 1);
    assert!(pool.delegated_stakes().length() == 0); // empty because no stakes set to that pool

    let mut pools = NetworkAccount::pools(&mut ws, [1u8; 32]);
    let mut stakes = pools.delegated_stakes();
    assert_eq!(stakes.length(), 0);
    stakes
        .push(StakeValue::new(Stake {
            owner: [2u8; 32],
            power: 10,
        }))
        .unwrap();
    assert_eq!(stakes.length(), 1);

    let mut pools = NetworkAccount::pools(&mut ws, [1u8; 32]);
    let mut stakes = pools.delegated_stakes();
    assert_eq!(stakes.length(), 1);
    let stake = stakes.get_by(&[2u8; 32]).unwrap();
    assert_eq!(stake.owner, [2u8; 32]);
    assert_eq!(stake.power, 10);

    // Check Deposit
    let mut deposit = NetworkAccount::deposits(&mut ws, [1u8; 32], [2u8; 32]);
    // all values are unset now. This stake is different to pools.states.
    assert!(deposit.balance().is_none());
    assert!(deposit.auto_stake_rewards().is_none());

    deposit.set_balance(987_u64);
    deposit.set_auto_stake_rewards(true);

    assert_eq!(deposit.balance().unwrap(), 987);
    assert_eq!(deposit.auto_stake_rewards().unwrap(), true);
}

#[test]
fn test_network_account_validator_set() {
    let env = TestEnv::default();
    let mut ws = StorageWorldState::<DummyStorage, V1>::initialize(&env.db);

    let pool_1 = Pool {
        operator: [1u8; 32],
        power: 600,
        commission_rate: 5,
        operator_stake: Some(Stake {
            owner: [1u8; 32],
            power: 600,
        }),
    };
    let pool_2 = Pool {
        operator: [5u8; 32],
        power: 60,
        commission_rate: 2,
        operator_stake: Some(Stake {
            owner: [1u8; 32],
            power: 60,
        }),
    };
    NetworkAccount::vp(&mut ws)
        .push(pool_1.clone(), vec![])
        .unwrap();
    NetworkAccount::vp(&mut ws)
        .push(pool_2.clone(), vec![])
        .unwrap();
    assert_eq!(NetworkAccount::vp(&mut ws).length(), 2);
    assert_eq!(NetworkAccount::pvp(&mut ws).length(), 0);
    NetworkAccount::pvp(&mut ws)
        .push(pool_1.clone(), vec![])
        .unwrap();
    assert_eq!(NetworkAccount::vp(&mut ws).length(), 2);
    assert_eq!(NetworkAccount::pvp(&mut ws).length(), 1);

    let mut nvp = NetworkAccount::nvp(&mut ws);
    nvp.insert(PoolKey::new(pool_2.operator, pool_2.power))
        .unwrap();
    nvp.insert(PoolKey::new(pool_1.operator, pool_1.power))
        .unwrap();
    assert_eq!(nvp.length(), 2);
    let top = nvp.extract().unwrap();
    assert_eq!(nvp.length(), 1);
    assert_eq!(top.operator, pool_2.operator);
    assert_eq!(top.power, pool_2.power);
}
