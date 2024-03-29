use hash_db::Hasher;
use pchain_types::cryptography::{Keypair, PublicAddress};
use pchain_world_state::*;
use rand::rngs::OsRng;
use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};
use reference_trie::RefHasher;
use std::collections::{HashMap, HashSet};
use std::time::Instant;
pub type Key = Vec<u8>;
pub type Value = Vec<u8>;

const PUBLIC_KEY: &str = "ipy_VXNiwHNP9mx6-nKxht_ZJNfYoMAcCnLykpq4x_k";

#[derive(Debug, Clone, Default)]
pub struct AccountInstance {
    pub nonce: u64,
    pub balance: u64,
    pub code: Vec<u8>,
    pub cbi_version: Option<u32>,
    pub storage_hash: Vec<u8>,
    pub storages: HashMap<Vec<u8>, Vec<u8>>,
}

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

pub fn generate_public_addr() -> PublicAddress {
    let mut osrng = OsRng {};
    let mut chacha_20rng = ChaCha20Rng::from_rng(&mut osrng).unwrap();
    let public_key: PublicAddress = Keypair::generate(&mut chacha_20rng)
        .verifying_key()
        .as_bytes()
        .clone();
    public_key
}

pub fn generate_spec_account() -> (PublicAddress, AccountInstance) {
    let mut account = AccountInstance::default();
    let account_addr = base64url::decode(PUBLIC_KEY).unwrap().try_into().unwrap();
    account.nonce = 1_u64;
    account.balance = 100_000_u64;
    account.cbi_version = Some(1_u32);
    let mut storages = HashMap::new();
    let specific_key = b"apple".to_vec();
    let specific_value = b"apple_value".to_vec();
    storages.insert(specific_key, specific_value);
    account.storages = storages;
    (account_addr, account)
}

pub fn generate_accounts() -> HashMap<PublicAddress, AccountInstance> {
    let mut accounts_map: HashMap<PublicAddress, AccountInstance> = HashMap::new();
    // generate 10000 Accounts
    for index in 1..100 {
        let mut account = AccountInstance::default();
        let account_address = generate_public_addr();
        account.nonce = index.try_into().unwrap();
        account.balance = (index * 1000).try_into().unwrap();
        account.cbi_version = Some(1_u32);
        account.code = account_address.to_vec();
        // generate 255 storage <key, value> pairs
        let mut storages = HashMap::new();
        for index_1 in 1..255 {
            let key = RefHasher::hash(&[index_1]);
            storages.insert(key.to_vec(), account_address.to_vec());
        }
        account.storages = storages;
        accounts_map.insert(account_address, account);
    }
    let spec_account = generate_spec_account();
    accounts_map.insert(spec_account.0, spec_account.1);
    accounts_map
}

#[test]
pub fn upgrade() {
    println!("start to build in v1....");
    let mut db = DummyStorage(HashMap::new());
    let mut ws_1: WorldState<'_, DummyStorage, V1> = WorldState::<DummyStorage, V1>::new(&db);
    let accounts = generate_accounts();
    for (address, account) in accounts.into_iter() {
        let account_trie_mut = ws_1.account_trie_mut();
        account_trie_mut.set_nonce(&address, account.nonce).unwrap();
        account_trie_mut
            .set_balance(&address, account.balance)
            .unwrap();
        account_trie_mut
            .set_cbi_version(&address, account.cbi_version.unwrap())
            .unwrap();
        account_trie_mut
            .set_code(&address, account.code.clone())
            .unwrap();
        if !account.storages.is_empty() {
            let storage_trie_mut = ws_1.storage_trie_mut(&address).unwrap();
            storage_trie_mut.batch_set(&account.storages).unwrap();
        }
    }
    let ws_1_changes = ws_1.close().unwrap();
    db.apply_changes(ws_1_changes.inserts, ws_1_changes.deletes);
    println!("finished build...");

    println!("start upgrade..");
    let start_time = Instant::now();
    let ws1_new = WorldState::<DummyStorage, V1>::open(&db, ws_1_changes.new_root_hash);
    let mut ws_2 = ws1_new.upgrade().unwrap();
    let ws_2_changes = ws_2.close().unwrap();
    db.apply_changes(ws_2_changes.inserts, ws_2_changes.deletes);
    let end_time = Instant::now();
    println!(
        "upgrade cost: {} milliseconds", //181184 milliseconds
        end_time.duration_since(start_time).as_millis()
    );

    println!("start to iter...");
    let mut ws_2_new = WorldState::<DummyStorage, V2>::open(&db, ws_2_changes.new_root_hash);
    let start_time = Instant::now();
    let accounts = ws_2_new.account_trie().all().unwrap();
    for (address, _) in accounts.iter() {
        ws_2_new.storage_trie(&address).unwrap();
    }
    let end_time: Instant = Instant::now();
    println!(
        "Iter cost: {} milliseconds", //1296 milliseconds
        end_time.duration_since(start_time).as_millis()
    )
}

#[test]
pub fn test_upgrade_v1_to_v2() {
    let mut db = DummyStorage(HashMap::new());
    let mut ws_1 = WorldState::<_, V1>::new(&db);

    // Setup an account with some data in its storage
    ws_1.account_trie_mut()
        .set_balance(&[1u8; 32], 12345)
        .unwrap();
    for (key, value) in [
        (vec![31u8; 31], vec![1]),
        (vec![32u8; 32], vec![2, 2]),
        (vec![33u8; 33], vec![3, 3, 3]),
        (vec![64u8; 64], vec![4, 4, 4, 4]),
    ] {
        ws_1.storage_trie_mut(&[1u8; 32])
            .unwrap()
            .set(&key, value)
            .unwrap();
    }

    // Upgrade
    let mut ws_2 = ws_1.upgrade().unwrap();

    // Save to DB
    let ws_2_changes = ws_2.close().unwrap();
    let ws_2_root_hash = ws_2_changes.new_root_hash;
    db.apply_changes(ws_2_changes.inserts, ws_2_changes.deletes);

    // Open world state from new root hash
    let mut ws_2 = WorldState::<_, V2>::open(&db, ws_2_root_hash);

    // Check: there is only one account
    let ws_2_all_accounts = ws_2.account_trie().all().unwrap();
    assert_eq!(ws_2_all_accounts.len(), 1);

    // Check: account information is preserved
    let account = ws_2_all_accounts.get(&[1u8; 32]).unwrap();
    assert_eq!(
        (
            account.balance,
            account.cbi_version,
            account.code.is_empty(),
            account.nonce,
            account.storage_hash.is_empty()
        ),
        (12345, None, true, 0, false) // storage hash has updated when close() was called
    );

    // Check: storage data is preserved
    let storage = ws_2.storage_trie(&[1u8; 32]).unwrap();
    for (key, value) in [
        (vec![31u8; 31], vec![1]),
        (vec![32u8; 32], vec![2, 2]),
        (vec![33u8; 33], vec![3, 3, 3]),
        (vec![64u8; 64], vec![4, 4, 4, 4]),
    ] {
        assert_eq!(storage.get(&key).unwrap(), Some(value));
    }
}
