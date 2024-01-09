use criterion::{criterion_group, criterion_main, Bencher, BenchmarkId, Criterion};
use hash_db::Hasher;
use keccak_hasher::KeccakHasher;
use pchain_types::cryptography::{PublicAddress, Sha256Hash};
use pchain_world_state::{db::KeyInstrumentedDB, Mpt, MptError, WorldState, V1, V2};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use rocksdb::{DBWithThreadMode, MultiThreaded};
use statrs::statistics::Statistics;
use std::{
    collections::{HashMap, HashSet},
    env::temp_dir,
    path::PathBuf,
    sync::{Arc, RwLock},
};
use temp_dir::TempDir;
/// `mpt_write_benchmark` is benchmark test for MPT insert
fn mpt_write_benchmark(c: &mut Criterion) {
    // generate 100,000 pair of <key, value> dataset
    let dataset = generate_random_value_pair(100000);
    // benchmark MPT V1
    let db_dir_v1 = generate_temp_dir();
    let db_paths_v1 = Arc::new(RwLock::new(Vec::<PathBuf>::new()));
    let input_v1 = (
        dataset.clone(),
        Arc::clone(&db_paths_v1),
        Arc::clone(&db_dir_v1),
    );
    c.bench_with_input(
        BenchmarkId::new("mpt_v1_insert", "100,000 <key, value> iteration"),
        &input_v1,
        |b, input_v1| {
            b.iter(|| mpt_v1_insert(&input_v1.0, Arc::clone(&input_v1.1), Arc::clone(&db_dir_v1)))
        },
    );
    // measure db instances' status
    measure_db_stats(Arc::clone(&db_paths_v1));
    remove_all_db(db_paths_v1, db_dir_v1);

    // benchmark MPT V2
    // go through dataset, hash the key to 32 bytes is its length great than 32 bytes
    let modified_dataset = limit_key_length_by_keccak_hasher(&dataset);
    let db_dir_v2 = generate_temp_dir();
    let db_paths_v2 = Arc::new(RwLock::new(Vec::<PathBuf>::new()));
    let input_v2 = (
        modified_dataset.clone(),
        Arc::clone(&db_paths_v2),
        Arc::clone(&db_dir_v2),
    );
    c.bench_with_input(
        BenchmarkId::new("mpt_v2_insert", "100,000 <key, value> iteration"),
        &input_v2,
        |b, input_v2| {
            b.iter(|| {
                mpt_v2_insert(
                    &input_v2.0,
                    Arc::clone(&input_v2.1),
                    Arc::clone(&input_v2.2),
                )
            })
        },
    );
    // measure db instances' status
    measure_db_stats(Arc::clone(&db_paths_v2));
    remove_all_db(db_paths_v2, db_dir_v2);
}

/// `mpt_read_benchmark` is benchmark test for MPT read
fn mpt_read_benchmark(c: &mut Criterion) {
    // generate 100,000 pair of <key, value> dataset
    let dataset = generate_random_value_pair(100000);
    // insert into MPT V1
    let db_dir_v1 = generate_temp_dir();
    let db_paths_v1 = Arc::new(RwLock::new(Vec::<PathBuf>::new()));
    let root_hash_v1 = mpt_v1_insert(&dataset, Arc::clone(&db_paths_v1), Arc::clone(&db_dir_v1));
    let input_v1 = (root_hash_v1, Arc::clone(&db_paths_v1));
    // benchmark MPT V1
    c.bench_with_input(
        BenchmarkId::new("mpt_v1_iter", "100,000 <key, value> iteration"),
        &input_v1,
        |b, input_v1| b.iter(|| mpt_v1_iter(input_v1.0, Arc::clone(&input_v1.1))),
    );
    remove_all_db(db_paths_v1, db_dir_v1);
    // go through dataset, hash the key to 32 bytes is its length great than 32 bytes
    let modified_dataset = limit_key_length_by_keccak_hasher(&dataset);
    // insert into MPT V2
    let db_dir_v2 = generate_temp_dir();
    let db_paths_v2 = Arc::new(RwLock::new(Vec::<PathBuf>::new()));
    let root_hash_v2 = mpt_v2_insert(
        &modified_dataset,
        Arc::clone(&db_paths_v2),
        Arc::clone(&db_dir_v2),
    );
    let input_v2 = (root_hash_v2, Arc::clone(&db_paths_v2));
    // benchmark MPT V2
    c.bench_with_input(
        BenchmarkId::new("mpt_v2_iter", "100,000 <key, value> iteration"),
        &input_v2,
        |b, input_v2| b.iter(|| mpt_v2_iter(input_v2.0, Arc::clone(&input_v2.1))),
    );
    remove_all_db(db_paths_v2, db_dir_v2);
}

/// `storage_trie_write_benchmark` is benchmark test for WorldState::StorageTrie write
fn storage_trie_write_benchmark(c: &mut Criterion) {
    // generate 100,000 pair of <key, value> dataset
    let dataset = generate_random_value_pair(100000);
    // benchmark test for storage_trie V1
    let db_dir_v1 = generate_temp_dir();
    let db_paths_v1 = Arc::new(RwLock::new(Vec::<PathBuf>::new()));
    let input_v1 = (
        dataset.clone(),
        Arc::clone(&db_paths_v1),
        Arc::clone(&db_dir_v1),
    );
    c.bench_with_input(
        BenchmarkId::new("storage_trie_v1_insert", "100,000 <key, value> iteration"),
        &input_v1,
        |b, input_v1| {
            b.iter(|| {
                storage_trie_v1_insert(&input_v1.0, Arc::clone(&input_v1.1), Arc::clone(&db_dir_v1))
            })
        },
    );
    // measure db instances' status
    measure_db_stats(Arc::clone(&db_paths_v1));
    remove_all_db(db_paths_v1, db_dir_v1);

    // benchmark test for storage_trie V1
    // go through dataset, hash the key to 32 bytes is its length great than 32 bytes
    let db_dir_v2 = generate_temp_dir();
    let db_paths_v2 = Arc::new(RwLock::new(Vec::<PathBuf>::new()));
    let input_v2 = (
        dataset.clone(),
        Arc::clone(&db_paths_v2),
        Arc::clone(&db_dir_v2),
    );
    c.bench_with_input(
        BenchmarkId::new("storage_trie_v2_insert", "100,000 <key, value> iteration"),
        &input_v2,
        |b, input_v2| {
            b.iter(|| {
                storage_trie_v2_insert(
                    &input_v2.0,
                    Arc::clone(&input_v2.1),
                    Arc::clone(&input_v2.2),
                )
            })
        },
    );
    // measure db instances' status
    measure_db_stats(Arc::clone(&db_paths_v2));
    remove_all_db(db_paths_v2, db_dir_v2);
}

/// `mpt_v1_insert` is helper function for MPT insertion benchmark test
fn mpt_v1_insert(
    dataset: &HashMap<Vec<u8>, Vec<u8>>,
    db_paths: DbPaths,
    db_dir: DbDir,
) -> Sha256Hash {
    // open RocksDB
    let path = generate_random_path(db_paths, db_dir);
    let test_db = TestDB::open_db(path.clone());
    // dummy account address
    let address: PublicAddress = base64url::decode("x7eiywH_8YVHQkSgjZk3EXdLU3FGo4VaV_6qi-hzOKI")
        .unwrap()
        .try_into()
        .unwrap();
    let key_instrumented_db =
        KeyInstrumentedDB::<TestDB, V1>::unsafe_new(&test_db, address.to_vec());
    let mut mpt = Mpt::<TestDB, V1>::unsafe_new(key_instrumented_db);
    mpt.batch_set(dataset).unwrap();
    let mpt_changes = mpt.close();
    test_db.write_batch(mpt_changes.0, mpt_changes.1);
    mpt_changes.2
}

/// `mpt_v1_insert` is helper function for MPT insertion benchmark test
fn mpt_v2_insert(
    dataset: &HashMap<Vec<u8>, Vec<u8>>,
    db_paths: DbPaths,
    db_dir: DbDir,
) -> Sha256Hash {
    // open RocksDB
    let path = generate_random_path(db_paths, db_dir);
    let test_db = TestDB::open_db(path.clone());
    // dummy account address
    let address: PublicAddress = base64url::decode("x7eiywH_8YVHQkSgjZk3EXdLU3FGo4VaV_6qi-hzOKI")
        .unwrap()
        .try_into()
        .unwrap();
    let key_instrumented_db =
        KeyInstrumentedDB::<TestDB, V2>::unsafe_new(&test_db, address.to_vec());
    let mut mpt = Mpt::<TestDB, V2>::unsafe_new(key_instrumented_db);
    mpt.batch_set(dataset).unwrap();
    let mpt_changes = mpt.close();
    test_db.write_batch(mpt_changes.0, mpt_changes.1);
    mpt_changes.2
}

/// `mpt_v1_iter` is helper function for MPT iteration benchmark test
fn mpt_v1_iter(root_hash: Sha256Hash, db_paths: DbPaths) {
    let db_path = db_paths.read().unwrap().get(0).unwrap().to_owned();
    let test_db = TestDB::open_db(db_path);
    // dummy account address
    let address: PublicAddress = base64url::decode("x7eiywH_8YVHQkSgjZk3EXdLU3FGo4VaV_6qi-hzOKI")
        .unwrap()
        .try_into()
        .unwrap();
    let key_instrumented_db =
        KeyInstrumentedDB::<TestDB, V1>::unsafe_new(&test_db, address.to_vec());
    let mpt = Mpt::<TestDB, V1>::open(key_instrumented_db, root_hash);
    let _ = mpt.iterate_all(|mut key, mut value| {
        // simulate a operation
        key.append(&mut value);
        Ok::<(), MptError>(())
    });
}

/// `mpt_v2_iter` is helper function for MPT iteration benchmark test
fn mpt_v2_iter(root_hash: Sha256Hash, db_paths: DbPaths) {
    let db_path = db_paths.read().unwrap().get(0).unwrap().to_owned();
    let test_db = TestDB::open_db(db_path);
    // dummy account address
    let address: PublicAddress = base64url::decode("x7eiywH_8YVHQkSgjZk3EXdLU3FGo4VaV_6qi-hzOKI")
        .unwrap()
        .try_into()
        .unwrap();
    let key_instrumented_db =
        KeyInstrumentedDB::<TestDB, V2>::unsafe_new(&test_db, address.to_vec());
    let mpt = Mpt::<TestDB, V2>::open(key_instrumented_db, root_hash);
    let _ = mpt.iterate_all(|mut key, mut value| {
        // simulate a operation
        key.append(&mut value);
        Ok::<(), MptError>(())
    });
}

/// `storage_trie_v1_insert` is helper function for StorageTrie insert benchmark test
fn storage_trie_v1_insert(
    dataset: &HashMap<Vec<u8>, Vec<u8>>,
    db_paths: DbPaths,
    db_dir: DbDir,
) -> Sha256Hash {
    let path = generate_random_path(db_paths, db_dir);
    let test_db = TestDB::open_db(path.clone());
    // create a dummy account address
    let address: PublicAddress = base64url::decode("x7eiywH_8YVHQkSgjZk3EXdLU3FGo4VaV_6qi-hzOKI")
        .unwrap()
        .try_into()
        .unwrap();
    let mut ws = WorldState::<TestDB, V1>::new(&test_db);
    ws.storage_trie_mut(&address)
        .unwrap()
        .batch_set(dataset)
        .unwrap();
    let ws_changes = ws.close().unwrap();
    test_db.write_batch(ws_changes.inserts, ws_changes.deletes);
    ws_changes.new_root_hash
}

/// `storage_trie_v2_insert` is helper function for StorageTrie insert benchmark test
fn storage_trie_v2_insert(
    dataset: &HashMap<Vec<u8>, Vec<u8>>,
    db_paths: DbPaths,
    db_dir: DbDir,
) -> Sha256Hash {
    let path = generate_random_path(db_paths, db_dir);
    let test_db = TestDB::open_db(path.clone());
    // create a dummy account address
    let address: PublicAddress = base64url::decode("x7eiywH_8YVHQkSgjZk3EXdLU3FGo4VaV_6qi-hzOKI")
        .unwrap()
        .try_into()
        .unwrap();
    let mut ws = WorldState::<TestDB, V2>::new(&test_db);
    ws.storage_trie_mut(&address)
        .unwrap()
        .batch_set(dataset)
        .unwrap();
    let ws_changes = ws.close().unwrap();
    test_db.write_batch(ws_changes.inserts, ws_changes.deletes);
    ws_changes.new_root_hash
}

/// `measure_db_stats` provide db stats of RocksDB instance after insertion
fn measure_db_stats(db_paths: DbPaths) {
    let mut num_of_keys: Vec<f64> = Vec::new();
    let mut live_data_size: Vec<f64> = Vec::new();
    for path in db_paths.read().unwrap().iter() {
        let test_db = TestDB::open_db(path.clone());
        let num_of_key = test_db
            .db
            .property_value(rocksdb::properties::ESTIMATE_NUM_KEYS)
            .unwrap()
            .unwrap();
        let data_size = test_db
            .db
            .property_value(rocksdb::properties::ESTIMATE_LIVE_DATA_SIZE)
            .unwrap()
            .unwrap();
        num_of_keys.push(num_of_key.parse().unwrap());
        live_data_size.push(data_size.parse().unwrap());
    }
    println!("mean of number of keys: {}", Statistics::mean(num_of_keys));
    println!(
        "mean of live data size in KB: {}",
        (Statistics::mean(live_data_size) / 1024_f64)
    )
}

/// `generate random_value_pair` is to generate `target_num` pairs of <Vec<u8>, Vec<u8>> as input dataset for benchmark test
fn generate_random_value_pair(target_num: u64) -> HashMap<Vec<u8>, Vec<u8>> {
    let mut dataset: HashMap<Vec<u8>, Vec<u8>> = HashMap::with_capacity(target_num as usize);
    for _ in 0..target_num {
        // generate length of the key/value
        let len: usize = thread_rng().gen_range(1..64);
        // generate random key, value
        let key = generate_random_vec_u8(len);
        let value = generate_random_vec_u8(len);
        dataset.insert(key, value);
    }
    dataset
}

/// `limit_key_length_by_keccak_hasher` hash the key length to 32 bytes long if its length greater than 32 bytes
fn limit_key_length_by_keccak_hasher(
    dataset: &HashMap<Vec<u8>, Vec<u8>>,
) -> HashMap<Vec<u8>, Vec<u8>> {
    let mut modified_dataset: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
    for (key, value) in dataset {
        if key.len() > 32_usize {
            modified_dataset.insert(KeccakHasher::hash(&key).to_vec(), value.to_owned());
        } else {
            modified_dataset.insert(key.to_owned(), value.to_owned());
        }
    }
    modified_dataset
}

/// dir paths for RocksDB instances
type DbPaths = Arc<RwLock<Vec<PathBuf>>>;
/// dir to contain all RocksDB instances
type DbDir = Arc<RwLock<TempDir>>;

/// `generate_temp_dir` is to create the tmp dir for db instances
fn generate_temp_dir() -> DbDir {
    Arc::new(RwLock::new(temp_dir::TempDir::new().unwrap()))
}

/// `generate_random_dir_path` random generating dir pathes for rockets db
fn generate_random_path(db_paths: DbPaths, db_dir: DbDir) -> PathBuf {
    let rand_path_str: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect();
    let rand_path = db_dir
        .read()
        .unwrap()
        .child(format!("./.{}", rand_path_str));
    db_paths.write().unwrap().push(rand_path.clone());
    rand_path
}

/// `remvoe_all_db` is to clean all existing DB
fn remove_all_db(db_paths: DbPaths, db_dir: DbDir) {
    for path in db_paths.read().unwrap().iter() {
        std::fs::remove_dir_all(path).expect("failed to clear db");
    }
    db_dir.read().unwrap().clone().cleanup().unwrap();
}

type RocketsDB = DBWithThreadMode<MultiThreaded>;
#[derive(Debug, Clone)]
struct TestDB {
    db: Arc<RocketsDB>,
}

impl pchain_world_state::DB for TestDB {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        match self.db.get(key) {
            Ok(value) => value,
            Err(_) => None,
        }
    }
}

impl TestDB {
    fn open_db(path: PathBuf) -> Self {
        let db = RocketsDB::open_default(path).expect("Configuration error: Failed to open db");
        Self { db: Arc::new(db) }
    }

    fn write_batch(self, inserts: HashMap<Vec<u8>, Vec<u8>>, deletes: HashSet<Vec<u8>>) {
        let mut batch = rocksdb::WriteBatch::default();
        for (key, value) in inserts {
            batch.put(&key, &value);
        }
        for key in deletes {
            batch.delete(key);
        }
        self.db.write(batch).unwrap();
    }
}

/// `create_random_vec_u8` random generating Vec<u8> with length as input param
fn generate_random_vec_u8(length: usize) -> Vec<u8> {
    let ret: Vec<u8> = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .collect();
    return ret;
}

criterion_group!(
    benches,
    mpt_write_benchmark,
    mpt_read_benchmark,
    storage_trie_write_benchmark
);
criterion_main!(benches);
