# ParallelChain World State

In ParallelChain, we call the user-visible state that Mainnet maintains the “World State”. The world state is a set of key-value tuples representing the state of every “Account”, including both External accounts and Contract accounts, stored inside a Merkle Patricia Trie (MPT) [paritytech/trie-db](https://github.com/paritytech/trie). This library provides set of functions to read and update world state.

## Modules
 - [states](https://docs.rs/pchain-world-state/latest/pchain_world_state/error/): definition of WorldState and AccountStorageStage methods to read/write the MPT.
 - [network](https://docs.rs/pchain-world-state/latest/pchain_world_state/network/): data formatting scheme to store network-wide state in world state.
 - [keys](https://docs.rs/pchain-world-state/latest/pchain_world_state/keys/): definition of keys that Parallelchain-F are used to writes into persistent storage.
 - [storage](https://docs.rs/pchain-world-state/latest/pchain_world_state/storage/): definition of trait for accessing persistent storage from world state, and data structure of the world state changes.
 - [error](https://docs.rs/pchain-world-state/latest/pchain_world_state/error/): error handling when accessing the world state.

## Basic usage
```rust
// Here demonstrates how to create empty world state, update account information 
// and save the world state into database(hashmap).

// This is example address. Don't use this for real transaction.
let address: pchain_types::cryptography::PublicAddress = [200, 49, 188, 70, 13, 208, 8, 5, 148, 104, 28, 81, 229, 202, 203, 180, 220, 187, 48, 162, 53, 122, 83, 233, 166, 97, 173, 217, 25, 172, 106, 53];

// Step 1. prepare database that implement WorldStateStorage trait.
#[derive(Clone)]
struct DummyStorage(HashMap<Key, Value>);

impl WorldStateStorage for DummyStorage{
    fn get(&self, key: &Key) -> Option<Value>{
        match self.0.get(key){
            Some(value) => Some(value.to_owned()),
            None => None
        }
    }
}

impl DummyStorage{
    fn apply_changes(&mut self, changes: WorldStateChanges){
        for key in changes.deletes.iter() {
            self.remove(key);
        }

        for (key, value) in changes.inserts.iter() {
            self.insert(key.clonel(), value.clone());
        }
    }
}

// Create new database
let mut db = DummyStorage(HashMap::new());

// Step 2. Start an empty world state.
let ws = WorldState::initialize(db);

// Step 3. Some world state operations
// 3.1 Operations that immediately commit to the trie.
ws.with_commit().set_balance(address, 100);
ws.with_commit().set_nonce(address, 1);
ws.with_commit().set_storage_value(address, AppKey::new(key), value);
// 3.2 Operations that saved in cached set.
ws.cached().set_balance(address, 100);
ws.cached().set_nonce(address, 1);
ws.cached().set_storage_value(address, AppKey::new(key), value);
ws.commit(); // you can commit the cached changes to MPT

// Step 4. Commit and close the world state to get the WorldStateChanges
// WorldStateChanges contains the trie node changes since opening the world state and the new trie root hash (a.k.a state hash)
let db_changes: WorldStateChanges = ws.commit_and_close();

// Setp 5. Save the WorldStateChanges to database
db.apply_changes(db_changes);

```

## Versioning

The version of this library reflects the version of the ParallelChain Protocol which it implements. For example, the current version is 0.4.2, and this implements protocol version 0.4. Patch version increases are not guaranteed to be non-breaking.

## Opening an issue

Open an issue in GitHub if you:
1. Have a feature request / feature idea,
2. Have any questions (particularly software related questions),
3. Think you may have discovered a bug.

Please try to label your issues appropriately.