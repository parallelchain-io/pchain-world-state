/*
    Copyright © 2023, ParallelChain Lab 
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Unit Test on functionalities on this crate

use std::{convert::TryInto, collections::HashMap};

use pchain_types::{Stake, Pool};

use crate::{
    trie::{Value, Key}, 
    storage::WorldStateStorage, 
    keys::AppKey, 
    states::WorldState, 
    network::{network_account::{NetworkAccount, NetworkAccountStorage}, 
    pool::{PoolKey}, stake::StakeValue}
};

struct TestEnv{
    db: DummyStorage,
    address: pchain_types::PublicAddress
}

impl Default for TestEnv{
    fn default() -> Self {
        let db = DummyStorage(HashMap::new());
        const PUBLIC_KEY: &str = "ipy_VXNiwHNP9mx6-nKxht_ZJNfYoMAcCnLykpq4x_k";
        let address = pchain_types::Base64URL::decode(PUBLIC_KEY).unwrap().try_into().unwrap();

        Self { db, address }
    }
}


#[derive(Clone)]
struct DummyStorage(HashMap<Key, Value>);

impl DummyStorage{
    fn apply_changes(&mut self, write_set: HashMap<Vec<u8>, Vec<u8>>){
        self.0 = write_set;
    }
}

impl WorldStateStorage for DummyStorage{
    fn get(&self, key: &Key) -> Option<Value>{
        match self.0.get(key){
            Some(value) => Some(value.to_owned()),
            None => None
        }
    }
}

pub struct StorageWorldState <S> 
    where S: WorldStateStorage + Send + Sync + Clone 
{
    inner: WorldState<S>
}

impl<S> StorageWorldState<S>
    where S: WorldStateStorage + Send + Sync + Clone 
{
    fn initialize(storage: S) -> Self {
        Self {
            inner: WorldState::initialize(storage)
        }
    }
}

impl<S> NetworkAccountStorage for StorageWorldState<S> 
    where S: WorldStateStorage + Send + Sync + Clone 
{
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        let key = AppKey::new(key.to_vec());
        self.inner.storage_value(&pchain_types::NETWORK_ADDRESS, &key)
    }
    
    fn contains(&self, key: &[u8]) -> bool {
        let key = AppKey::new(key.to_vec());
        self.inner.contains().storage_value(&pchain_types::NETWORK_ADDRESS, &key)
    }

    fn set(&mut self, key: &[u8], value: Vec<u8>) {
        let key = AppKey::new(key.to_vec());
        self.inner.with_commit().set_storage_value(pchain_types::NETWORK_ADDRESS, key, value);
    }

    fn delete(&mut self, key: &[u8]) {
        let key = AppKey::new(key.to_vec());
        self.inner.with_commit().set_storage_value(pchain_types::NETWORK_ADDRESS, key, Vec::new());
    }
}

#[test]
fn update_nonce(){
    let mut env = TestEnv::default();
    let mut genesis_ws = WorldState::initialize(env.db.clone());

    assert_eq!(genesis_ws.nonce(env.address), 0);
    genesis_ws.cached().set_nonce(env.address, 1);
    let ws_changes = genesis_ws.commit_and_close();
    env.db.apply_changes(ws_changes.inserts);
    
    let new_ws = WorldState::open(env.db, ws_changes.next_state_hash);
    assert!(new_ws.is_ok());
    assert_eq!(new_ws.unwrap().nonce(env.address), 1);
}

#[test]
fn update_balance(){
    let mut env = TestEnv::default();
    let mut genesis_ws = WorldState::initialize(env.db.clone());

    assert_eq!(genesis_ws.balance(env.address), 0);
    genesis_ws.cached().set_balance(env.address, 100_000);
    let ws_changes = genesis_ws.commit_and_close();
    env.db.apply_changes(ws_changes.inserts);
    
    let new_ws = WorldState::open(env.db, ws_changes.next_state_hash);
    assert!(new_ws.is_ok());
    assert_eq!(new_ws.unwrap().balance(env.address), 100_000);
}

#[test]
fn update_code(){
    let mut env = TestEnv::default();
    let mut genesis_ws = WorldState::initialize(env.db.clone());

    assert!(genesis_ws.code(env.address).is_none());
    genesis_ws.cached().set_code(env.address, vec![1_u8;100]);
    let ws_changes = genesis_ws.commit_and_close();
    env.db.apply_changes(ws_changes.inserts);
    
    let new_ws = WorldState::open(env.db, ws_changes.next_state_hash);
    assert!(new_ws.is_ok());
    let code = new_ws.unwrap().code(env.address);
    assert!(code.is_some());
    assert_eq!(code.unwrap(), vec![1_u8;100]);
}

#[test]
fn update_storage(){
    let mut env = TestEnv::default();
    let mut genesis_ws = WorldState::initialize(env.db.clone());
    // genesis_ws.set_balance(env.address, 100_000);

    let app_key = AppKey::new(b"apple".to_vec());
    let app_value = b"1234".to_vec();
    assert!(genesis_ws.storage_value(&env.address, &app_key).is_none());

    genesis_ws.cached().set_storage_value(env.address, app_key.clone(), app_value.clone());
    let ws_changes = genesis_ws.commit_and_close();
    env.db.apply_changes(ws_changes.inserts);
    
    let new_ws = WorldState::open(env.db, ws_changes.next_state_hash);
    assert!(new_ws.is_ok());
    let mut new_ws = new_ws.unwrap();
    new_ws.cached().set_balance(env.address, 100_000);
    
    let value = new_ws.storage_value(&env.address, &app_key);
    assert!(value.is_some());
    assert_eq!(value.unwrap(), app_value);
}

#[test]
fn get_keys_from_account_storage(){
    let mut env = TestEnv::default();
    let mut genesis_ws = WorldState::initialize(env.db.clone());

    let app_key1 = AppKey::new(b"123".to_vec());
    let app_value1 = b"abc".to_vec();
    genesis_ws.cached().set_storage_value(env.address, app_key1.clone(), app_value1.clone());

    let app_key2 = AppKey::new(b"987".to_vec());
    let app_value2 = b"xyz".to_vec();
    genesis_ws.cached().set_storage_value(env.address, app_key2.clone(), app_value2.clone());
    
    let ws_changes = genesis_ws.commit_and_close();
    env.db.apply_changes(ws_changes.inserts);
    
    let new_ws = WorldState::open(env.db, ws_changes.next_state_hash);
    assert!(new_ws.is_ok());
    let new_ws = new_ws.unwrap();
    let values = new_ws.storage_values(&env.address, &vec![app_key1.clone(), app_key2.clone()]);
    assert_eq!(values.len(), 2);
    let value1 = values.get(&app_key1).unwrap();
    assert_eq!(value1.as_deref().unwrap(), app_value1);
    let value2 = values.get(&app_key2).unwrap();
    assert_eq!(value2.as_deref().unwrap(), app_value2);
}

#[test]
fn get_all_account_storage(){
    let mut env = TestEnv::default();
    let mut genesis_ws = WorldState::initialize(env.db.clone());

    for i in 1_u8..=5{
        let app_key = AppKey::new(i.to_le_bytes().to_vec());
        let app_value = i.to_le_bytes().to_vec();
        genesis_ws.cached().set_storage_value(env.address, app_key.clone(), app_value.clone());
    }
    
    let ws_changes = genesis_ws.commit_and_close();
    env.db.apply_changes(ws_changes.inserts);
    
    let new_ws = WorldState::open(env.db, ws_changes.next_state_hash);
    assert!(new_ws.is_ok());
    let new_ws = new_ws.unwrap();
    let storage_list = new_ws.account_storage(&env.address);
    assert!(storage_list.is_some());
    assert_eq!(storage_list.unwrap().len(), 5);
}

#[test]
fn get_data_after_update(){
    let mut env = TestEnv::default();
    let mut genesis_ws = WorldState::initialize(env.db.clone());
    // genesis_ws.set_balance(env.address, 100_000);

    let app_key = AppKey::new(b"apple".to_vec());
    let app_value = b"1234".to_vec();
    assert!(genesis_ws.storage_value(&env.address, &app_key).is_none());

    genesis_ws.cached().set_storage_value(env.address, app_key.clone(), app_value.clone());
    let ws_changes = genesis_ws.commit_and_close();
    env.db.apply_changes(ws_changes.inserts);
    
    // open updated world state and set new value
    let new_ws = WorldState::open(env.db, ws_changes.next_state_hash);
    assert!(new_ws.is_ok());
    let mut new_ws = new_ws.unwrap();
    let new_app_value = b"xyz".to_vec();
    new_ws.cached().set_storage_value(env.address, app_key.clone(), new_app_value.clone());
    new_ws.commit();
    let value = new_ws.storage_value(&env.address, &app_key);
    assert!(value.is_some());
    assert_eq!(value.unwrap(), new_app_value);

    //discard the changes
    new_ws.discard_pending_writes();
    let value = new_ws.storage_value(&env.address, &app_key);
    assert!(value.is_some());
    assert_eq!(value.unwrap(), app_value);
}

#[test]
fn test_network_account() {
    let env = TestEnv::default();
    let mut ws = StorageWorldState::initialize(env.db);

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
    stakes.push(StakeValue::new(pchain_types::Stake { owner: [2u8; 32], power: 10 })).unwrap();
    assert_eq!(stakes.length(), 1);

    let mut pools = NetworkAccount::pools(&mut ws, [1u8; 32]);
    let stakes = pools.delegated_stakes();
    assert_eq!(stakes.length(), 1);
    let stake = stakes.get_by(&[2u8; 32]).unwrap();
    assert_eq!(stake.owner, [2u8; 32]);
    assert_eq!(stake.power, 10);

    // Check Deposit
    let mut deposit = NetworkAccount::deposits(&mut ws, [1u8; 32], [2u8; 32]);
    // all values are unset now. This stake is different to pools.states.
    assert!(deposit.balance().is_none());
    assert!(deposit.auto_stake_rewards().is_none());

    deposit.set_balance(987);
    deposit.set_auto_stake_rewards(true);

    assert_eq!(deposit.balance().unwrap(), 987);
    assert_eq!(deposit.auto_stake_rewards().unwrap(), true);
}

#[test]
fn test_network_account_validator_set() {
    let env = TestEnv::default();
    let mut ws = StorageWorldState::initialize(env.db);

    let pool_1 = Pool { 
        operator: [1u8; 32], 
        power: 600, 
        commission_rate: 5, 
        operator_stake: Some(Stake{ owner: [1u8; 32], power: 600 })
    };
    let pool_2 = Pool { 
        operator: [5u8; 32], 
        power: 60, 
        commission_rate: 2, 
        operator_stake: Some(Stake{ owner: [1u8; 32], power: 60 })
    };
    NetworkAccount::vp(&mut ws).push(pool_1.clone(), vec![]).unwrap();
    NetworkAccount::vp(&mut ws).push(pool_2.clone(), vec![]).unwrap();
    assert_eq!(NetworkAccount::vp(&mut ws).length(), 2);
    assert_eq!(NetworkAccount::pvp(&mut ws).length(), 0);
    NetworkAccount::pvp(&mut ws).push(pool_1.clone(), vec![]).unwrap();
    assert_eq!(NetworkAccount::vp(&mut ws).length(), 2);
    assert_eq!(NetworkAccount::pvp(&mut ws).length(), 1);

    let mut nvp = NetworkAccount::nvp(&mut ws);
    nvp.insert(PoolKey::new(pool_2.operator, pool_2.power)).unwrap();
    nvp.insert(PoolKey::new(pool_1.operator, pool_1.power)).unwrap();
    assert_eq!(nvp.length(), 2);
    let top = nvp.extract().unwrap();
    assert_eq!(nvp.length(), 1);
    assert_eq!(top.operator, pool_2.operator);
    assert_eq!(top.power, pool_2.power);
}