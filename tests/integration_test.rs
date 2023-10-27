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
    fn size(&self) -> usize {
        return self.0.len();
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

pub fn generate_spec_account() -> (PublicAddress, Account) {
    let mut account = Account::default();
    let account_addr = base64url::decode(PUBLIC_KEY).unwrap().try_into().unwrap();
    account.nonce = 1_u64;
    account.balance = 100_000_u64;
    let mut storages = HashMap::new();
    let specific_key = b"apple".to_vec();
    let specific_value = b"apple_value".to_vec();
    storages.insert(specific_key, specific_value);
    account.set_storages(storages);
    (account_addr, account)
}

pub fn generate_accounts() -> HashMap<PublicAddress, Account> {
    let mut accounts_map: HashMap<PublicAddress, Account> = HashMap::new();
    // generate 100000 Accounts
    for index in 1..100000 {
        let mut account = Account::default();
        let account_address = generate_public_addr();
        account.nonce = index.try_into().unwrap();
        account.balance = (index * 1000).try_into().unwrap();
        account.cbi_version = 1_u32;
        account.code = account_address.to_vec();
        // generate 255 storage <key, value> pairs
        let mut storages = HashMap::new();
        for index_1 in 1..255 {
            let key = RefHasher::hash(&[index_1]);
            storages.insert(key.to_vec(), account_address.to_vec());
        }
        account.set_storages(storages);
        accounts_map.insert(account_address, account);
    }
    let spec_account = generate_spec_account();
    accounts_map.insert(spec_account.0, spec_account.1);
    accounts_map
}

#[test]
#[ignore]
pub fn build_test() {
    // with 100000 accounts, and each accounts contains 255 <key, value> pair data in storage
    // the whole test finished in 3350.84s
    println!("start to build in v1....");
    let mut db_1 = DummyStorage(HashMap::new());
    let mut ws_1 = WorldState::<DummyStorage, V1>::new(&db_1);
    let ws_1_changes = ws_1.build(generate_accounts()).unwrap();
    db_1.apply_changes(ws_1_changes.inserts, ws_1_changes.deletes);
    println!("db1 {}", db_1.size()); // number of <key, value> pairs in db is 34837001
    let mut ws_1_new = WorldState::<DummyStorage, V1>::open(&db_1, ws_1_changes.new_root_hash);
    let start_1 = Instant::now();
    println!("start to iter...");
    let account_addr: PublicAddress = base64url::decode(PUBLIC_KEY).unwrap().try_into().unwrap();
    ws_1_new
        .storage_trie(&account_addr)
        .unwrap()
        .get(&b"apple".to_vec())
        .unwrap();
    let accounts = ws_1_new.account_trie().all().unwrap();
    for (address, _) in accounts.iter() {
        ws_1_new.storage_trie(&address).unwrap();
    }
    let end_1 = Instant::now();
    println!(
        "Time cost: {} milliseconds",
        end_1.duration_since(start_1).as_millis()
    );

    let mut db_2 = DummyStorage(HashMap::new());
    let mut ws_2 = WorldState::<DummyStorage, V2>::new(&db_2);
    let ws_2_changes = ws_2.build(generate_accounts()).unwrap();
    db_2.apply_changes(ws_2_changes.inserts, ws_2_changes.deletes);
    println!("db2 {}", db_2.size()); // number of <key, value> pairs in db is 35138406
    println!("start to iter...");
    let start_2 = Instant::now();
    let mut ws_2_new = WorldState::<DummyStorage, V2>::open(&db_2, ws_2_changes.new_root_hash);
    let account_addr: PublicAddress = base64url::decode(PUBLIC_KEY).unwrap().try_into().unwrap();
    ws_2_new
        .storage_trie(&account_addr)
        .unwrap()
        .get(&b"apple".to_vec())
        .unwrap();
    let accounts = ws_2_new.account_trie().all().unwrap();
    for (address, _) in accounts.iter() {
        ws_2_new.storage_trie(&address).unwrap();
    }
    let end_2 = Instant::now();
    println!(
        "Time cost: {} milliseconds",
        end_2.duration_since(start_2).as_millis()
    );
}

#[test]
#[ignore]
pub fn delete_and_bulid() {
    println!("start to build in v1....");
    let mut db_1 = DummyStorage(HashMap::new());
    let mut ws_1: WorldState<'_, DummyStorage, V1> = WorldState::<DummyStorage, V1>::new(&db_1);
    let ws_1_changes = ws_1.build(generate_accounts()).unwrap();
    db_1.apply_changes(ws_1_changes.inserts, ws_1_changes.deletes);
    println!("finished build...");

    println!("start destroy..");
    let start_time = Instant::now();
    let mut ws1_new = WorldState::<DummyStorage, V1>::open(&db_1, ws_1_changes.new_root_hash);
    let ws1_destory = ws1_new.destroy().unwrap();
    db_1.apply_changes(ws1_destory.inserts, ws1_destory.deletes);
    println!("{:?}", &db_1);
    let end_time = Instant::now();
    println!(
        "Destory cost: {} milliseconds",
        end_time.duration_since(start_time).as_millis()
    ); // 919583 milliseconds

    println!("start rebuild...");
    let start_time = Instant::now();
    let mut db_2 = DummyStorage(HashMap::new());
    let mut ws_2 = WorldState::<DummyStorage, V2>::new(&db_2);
    let ws_2_changes = ws_2.build(ws1_destory.accounts).unwrap();
    db_2.apply_changes(ws_2_changes.inserts, ws_2_changes.deletes);
    let end_time = Instant::now();
    println!(
        "Rebuild cost: {} milliseconds",
        end_time.duration_since(start_time).as_millis()
    ); // 1258858 milliseconds

    println!("start to iter...");
    let mut ws_2_new = WorldState::<DummyStorage, V2>::open(&db_2, ws_2_changes.new_root_hash);
    let start_time = Instant::now();
    let accounts = ws_2_new.account_trie().all().unwrap();
    for (address, _) in accounts.iter() {
        ws_2_new.storage_trie(&address).unwrap();
    }
    let end_time: Instant = Instant::now();
    println!(
        "Iter cost: {} milliseconds",
        end_time.duration_since(start_time).as_millis()
    ); // 5812 milliseconds
}
