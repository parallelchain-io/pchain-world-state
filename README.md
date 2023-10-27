# ParallelChain World State

In ParallelChain, we call the user-visible state that Mainnet maintains the “World State”. The world state is a set of key-value tuples representing the state of every “Account”, including both External accounts and Contract accounts, stored inside a Merkle Patricia Trie (MPT) [paritytech/trie-db](https://github.com/paritytech/trie). This library provides set of functions to read and update world state.

## Modules
 - world_state: Definition of "World State" and interfaces for operations on the current "World State"
 - version: Definition of identification for the difference between the old version WorldState and new version WorldState.
 - account_trie: Definition of "Account" and interfaces for operations on "Account" 
 - storage_trie: Definition of "Account Storage" and interfaces for operations on "Account Storage"
 - network_account_storage: data formatting scheme to store network-wide state in world state.
 - error: error handling when accessing the world state.

## Basic usage
```rust
// Here demonstrates how to create empty world state in Version 1, update account information
// And save the world state into database(hashmap).
// The operation for Version 2 is similar

// This is an example address. Don't use this for real transactions.
let address: pchain_types::cryptography::PublicAddress = [200, 49, 188, 70, 13, 208, 8, 5, 148, 104, 28, 81, 229, 202, 203, 180, 220, 187, 48, 162, 53, 122, 83, 233, 166, 97, 173, 217, 25, 172, 106, 53];

// Step 1. prepare database that implement WorldStateStorage trait.
#[derive(Clone)]
struct DummyStorage(HashMap<Key, Value>);

impl DB for DummyStorage{
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

// Step 2. Start an empty world state in Version 1.
let ws = WorldState::<DummyStorage, V1>::new(&db);

// Step 3. Some world state operations
ws.account_trie_mut().set_balance(address, 100_u64);
ws.account_trie_mut().set_nonce(address, 1_u64);
let key_apple: Key = b"apple".to_vec();
let value_apple: Value = b"1234".to_vec();
ws.storage_trie_mut().unwrap().set(key, value);
// Step 4. Commit and close the world state to get the WorldStateChanges
// WorldStateChanges contains the trie node changes since opening the world state and the new trie root hash (a.k.a state hash)
let db_changes: WorldStateChanges = ws.close();

// Setp 5. Save the WorldStateChanges to database
db.apply_changes(db_changes);

```

## Versioning

The version of this library reflects the version of the ParallelChain Protocol which it implements. For example, the current version is 0.5, and this implements protocol version 0.5. Patch version increases are not guaranteed to be non-breaking.

## Opening an issue

Open an issue in GitHub if you:
1. Have a feature request / feature idea,
2. Have any questions (particularly software related questions),
3. Think you may have discovered a bug.

Please try to label your issues appropriately.