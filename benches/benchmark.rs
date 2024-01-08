use criterion::{criterion_group, criterion_main, Bencher, BenchmarkId, Criterion};
use hash_db::Hasher;
use keccak_hasher::KeccakHasher;
use pchain_types::cryptography::PublicAddress;
use pchain_world_state::{db::KeyInstrumentedDB, Mpt, V1, V2};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use rocksdb::{DBWithThreadMode, MultiThreaded};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, RwLock},
};

fn mpt_write_benchmark(c: &mut Criterion) {
    // benchmark MPT V1
    let dataset = generate_random_value_pair(100000);
    let db_paths_v1 = Arc::new(RwLock::new(Vec::<PathBuf>::new()));
    let input_v1 = (dataset.clone(), Arc::clone(&db_paths_v1));
    c.bench_with_input(
        BenchmarkId::new("mpt_v1_insert", "100,000 <key, value> iteration"),
        &input_v1,
        |b, input_v1| b.iter(|| mpt_v1_insert(&input_v1.0, Arc::clone(&input_v1.1))),
    );
    for path in db_paths_v1.read().unwrap().iter() {
        std::fs::remove_dir_all(path).unwrap();
    }

    // benchmark MPT V2
    // go through db, hash the key if the length of key larger than 32 bytes
    let mut modified_dataset: HashMap<Vec<u8>, Vec<u8>> = HashMap::new();
    for (key, value) in dataset {
        if key.len() > 32_usize {
            modified_dataset.insert(KeccakHasher::hash(&key).to_vec(), value);
        } else {
            modified_dataset.insert(key, value);
        }
    }
    let db_paths_v2 = Arc::new(RwLock::new(Vec::<PathBuf>::new()));
    let input_v2 = (modified_dataset.clone(), Arc::clone(&db_paths_v2));
    c.bench_with_input(
        BenchmarkId::new("mpt_v2_insert", "100,000 <key, value> iteration"),
        &input_v2,
        |b, input_v2| b.iter(|| mpt_v2_insert(&input_v2.0, Arc::clone(&input_v2.1))),
    );
    for path in db_paths_v2.read().unwrap().iter() {
        std::fs::remove_dir_all(path).unwrap();
    }
}

fn mpt_read_benchmark(c: &mut Criterion) {}

fn storage_trie_write_benchmark(c: &mut Criterion) {
    {}
}

fn mpt_v1_insert(dataset: &HashMap<Vec<u8>, Vec<u8>>, db_paths: DbPaths) {
    // open RocksDB
    let path = generate_random_dir_path(db_paths);
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
}

fn mpt_v2_insert(dataset: &HashMap<Vec<u8>, Vec<u8>>, db_paths: DbPaths) {
    // open RocksDB
    let path = generate_random_dir_path(db_paths);
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
}

/// `generate random_value_pair` is to generate `target_num` pairs of <Vec<u8>, Vec<u8>> as input dataset for benchmark test
pub fn generate_random_value_pair(target_num: u64) -> HashMap<Vec<u8>, Vec<u8>> {
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

/// dir paths for RocksDB instances
type DbPaths = Arc<RwLock<Vec<PathBuf>>>;

/// `generate_random_dir_path` random generating dir pathes for rockets db
fn generate_random_dir_path(db_paths: DbPaths) -> PathBuf {
    let rand_path_str: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect();
    let rand_path = temp_dir::TempDir::new()
        .unwrap()
        .child(format!("./.{}", rand_path_str));
    db_paths.write().unwrap().push(rand_path.clone());
    rand_path
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
    // mpt_read_benchmark,
    // storage_trie_write_benchmark
);
criterion_main!(benches);
