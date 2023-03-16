/*
    Copyright © 2023, ParallelChain Lab 
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! IndexHeap implements minimum binary heap over an IndexMap. It is used for change of validator pools in Network Account.

use std::{
    ops::{Deref, DerefMut}
};

use super::{
    network_account::{NetworkAccountStorage, KeySpaced}, 
    index_map::IndexMap
};

/// IndexHeap supports below operations in addition to [IndexMap].
/// - extract - extract minimum item
/// - insert - insert item to heap
/// - remove_item - remove item from heap
/// - change_key - change key of an item 
pub struct IndexHeap<'a, T, V> 
    where 
        T: NetworkAccountStorage,
        V: Clone + PartialEq + Eq + PartialOrd + Ord + Into<Vec<u8>> + From<Vec<u8>> + KeySpaced
{
    inner: IndexMap<'a, T, V>,
}

impl<'a, T, V> IndexHeap<'a, T, V> 
    where 
        T: NetworkAccountStorage,
        V: Clone + PartialEq + Eq + PartialOrd + Ord + Into<Vec<u8>> + From<Vec<u8>> + KeySpaced
{
    pub(in crate::network) fn new(domain: Vec<u8>, store: &'a mut T, capacity: u32) -> Self {
        Self {
            inner: IndexMap::new(domain, store, capacity),
        }
    }

    /// extract the least value. Return None is no value to extract (i.e. empty heap)
    pub fn extract(&mut self) -> Option<V> {
        let length = self.length();
        if length == 0 { return None }
        let ret = self.get(0).unwrap();
        if length == 1 {
            self.set_length(0);
            self.delete(0, ret.key());
            return Some(ret);
        }

        let first_v = self.get(0).unwrap();
        let last_v = self.get(length - 1).unwrap();
        
        self.replace(0, first_v, length - 1, last_v);
        self.set_length(length - 1);

        self.make_heap(0, length - 1);
        
        Some(ret)
    }

    /// insert value to heap. Return Err if the heap is full 
    pub fn insert(&mut self, value: V) -> Result<(), IndexHeapOperationError> {
        let length = self.length();
        if self.capacity == length {
            return Err(IndexHeapOperationError)
        }

        let mut index = length;
        self.set(index, value);
        self.set_length(length + 1);

        loop {
            if index == 0 { break }
            let parent =  (index - 1) /2;
            let parent_v = self.get(parent).unwrap();
            let value = self.get(index).unwrap();
            if value < parent_v {
                self.swap(index, value, parent, parent_v);
                index = parent;
            } else {
                break;
            }
        }

        Ok(())
    }

    /// insert value to heap. If heap is full and the value should be inserted, extract the first and then insert.
    /// Ok(None) if inserted without extracting the smallest one
    /// Ok(Some) if inserted and extracted the smallest one
    /// Err if failed to insert because the value is smaller than smallest
    pub fn insert_extract(&mut self, value: V) -> Result<Option<V>, IndexHeapOperationError>{
        let length = self.length();
        if length == 0 { 
            self.insert(value).unwrap();
            return Ok(None)
        }
        let first_v = self.get(0).unwrap();

        let replaced = 
        if length == self.capacity {
            if value < first_v {
                // smaller than smallest, should not insert
                return Err(IndexHeapOperationError)
            }
            self.extract()
        } else {
            None
        };
        self.insert(value).unwrap();
        Ok(replaced)
    }

    /// Change key of a value that exists in the heap.
    pub fn change_key(&mut self, value: V) {
        let length = self.length();
        let mut index = match self.index_of_key(value.key()){
            Some(index) if index < length => index,
            _ => return
        };
        let old_value = self.get(index).unwrap();
        
        // increase key
        if old_value < value {
            self.set(index, value);
            self.make_heap(index, length);
        } else if value < old_value {
            // decrease key
            self.set(index, value);
            loop {
                if index == 0 { break }
                let parent = (index - 1) / 2;
                let value = self.get(index).unwrap();
                let parent_v = self.get(parent).unwrap();
                if value < parent_v {
                    self.swap(index, value, parent, parent_v);
                    index = parent;
                } else {
                    break;
                }
            }
        } // else unchanged

    }

    /// Return values by iterating over the index. Prefix with `unordered` to avoid confuse about the ordering of the values.
    pub fn unordered_values(&self) -> Vec<V> {
        let mut values = Vec::new();
        let length = self.length();
        for i in 0..length {
            values.push(self.get(i).unwrap());
        }
        values
    }

    /// Remove an keyed item.
    pub fn remove_item(&mut self, key: &[u8]) {
        let length = self.length();
        let mut index = match self.index_of_key(key){
            Some(index) if index < length => index,
            _ => return
        };

        if index == 0 { self.extract(); return }

        let this_v = self.get(index).unwrap();
        let last_v = self.get(length - 1).unwrap();

        self.replace(index, this_v, length - 1, last_v);
        self.set_length(length - 1);

        loop {
            if index == 0 { break }
            let parent = (index - 1) / 2;
            let value = self.get(index).unwrap();
            let parent_v = self.get(parent).unwrap();
            if value < parent_v {
                self.swap(index, value, parent, parent_v);
                index = parent;
            } else {
                break;
            }
        }

    }

    /// Create heap structures given an index.
    fn make_heap(&mut self, mut index: u32, length: u32) {
        loop {
            let left = 2 * index + 1;
            let right = 2 * index + 2;
            let mut head = index;
            if left < length {
                let value = self.get(head).unwrap();
                let left_v = self.get(left).unwrap();
                if left_v < value {
                    head = left;
                }
            }
            if right < length {
                let value = self.get(head).unwrap();
                let right_v = self.get(right).unwrap();
                if right_v < value {
                    head = right;
                }
            }
            if head != index {
                let value = self.get(index).unwrap();
                let head_v = self.get(head).unwrap();
                self.swap(index, value, head, head_v);
                index = head;
            } else {
                break;
            }
        }
    }

    /// replace old value with new value. Different from swap, replace will remove the old value.
    /// ### Safty
    /// indexes and values must ready exist in the heap
    fn replace(&mut self, to_index: u32, to_v: V, from_index: u32, from_v: V) {
        // delete IV[from_index] 
        let key_iv = [self.domain.as_slice(), &IndexMap::<'a, T, V>::PREFIX_INDEX_VALUE, from_index.to_le_bytes().as_ref()].concat();
        self.store.delete(&key_iv);
        // delete KI[from_key] (unnecessary)
        // delete IV[to_index] (unnecessary)
        // delete KI[to_key]
        let key_ki = [self.domain.as_slice(), &IndexMap::<'a, T, V>::PREFIX_KEY_INDEX, to_v.key()].concat();
        self.store.delete(&key_ki);
        // set IV[to_index] = from_value
        // set KI[from_key] = to_index
        self.set(to_index, from_v);
    }

    /// Swap values of V_i and V_j.
    /// ### Safty
    /// indexes and values must ready exist in the heap
    fn swap(&mut self, i: u32, i_v: V, j: u32, j_v: V) {
        self.set(i, j_v);
        self.set(j, i_v);
    }
}

#[derive(Debug)]
pub struct IndexHeapOperationError;

impl<'a, T, V> Deref for IndexHeap<'a, T, V> 
    where 
        T: NetworkAccountStorage,
        V: Clone + PartialEq + Eq + PartialOrd + Ord + Into<Vec<u8>> + From<Vec<u8>> + KeySpaced
{
    type Target = IndexMap<'a, T, V>;
    fn deref(&self) -> &Self::Target {
        &self.inner    
    }
}

impl<'a, T, V> DerefMut for IndexHeap<'a, T, V> 
    where 
        T: NetworkAccountStorage,
        V: Clone + PartialEq + Eq + PartialOrd + Ord + Into<Vec<u8>> + From<Vec<u8>> + KeySpaced
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[test]
fn test_binary_heap() {
    use std::collections::HashMap;
    #[derive(Clone, Debug, Eq, Ord)]
    struct TestU32 { name: String, data: u32}
    impl Into<Vec<u8>> for TestU32 {
        fn into(self) -> Vec<u8> {
            use pchain_types::Serializable;
            <(Vec<u8>, u32)>::serialize(&(
                self.name.as_bytes().to_vec(),
                self.data
            ))
        }
    }
    impl From<Vec<u8>> for TestU32 {
        fn from(bytes: Vec<u8>) -> Self {
            use pchain_types::Deserializable;
            let r = <(Vec<u8>, u32)>::deserialize(&bytes).unwrap();
            Self { name: String::from_utf8(r.0).unwrap(), data: r.1 }
        }
    }
    impl PartialEq for TestU32 {
        fn eq(&self, other: &Self) -> bool {
            self.data.eq(&other.data)
        }
    }
    impl PartialOrd for TestU32 {
        fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
            self.data.partial_cmp(&other.data)
        }
    }
    impl KeySpaced for TestU32 {
        fn key(&self) -> &[u8] {
            self.name.as_bytes()
        }
    }
    #[derive(Clone)]
    struct KVStore {
        inner: HashMap<Vec<u8>, Vec<u8>>
    }
    impl NetworkAccountStorage for KVStore {
        fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
            match self.inner.get(&key.to_vec()) {
                Some(v) => Some(v.to_owned()),
                None => None
            }
        }
        fn contains(&self, key: &[u8]) -> bool {
            self.inner.contains_key(key)
        }
        fn set(&mut self, key: &[u8], value: Vec<u8>) {
            self.inner.insert(key.to_vec(), value);
        }
        fn delete(&mut self, key: &[u8]) {
            self.inner.remove(key);
        }
    }
    
    let mut kv = KVStore { inner: HashMap::new() };

    // Insert elements
    {
        let mut heap = IndexHeap::<KVStore, TestU32>::new(vec![], &mut kv, 32);

        heap.insert(TestU32 { name: "apple".to_string(), data: 5 }).unwrap();
        heap.insert(TestU32 { name: "boy".to_string(), data: 25 }).unwrap();
        heap.insert(TestU32 { name: "cat".to_string(), data: 12 }).unwrap();
        heap.insert(TestU32 { name: "duck".to_string(), data: 17 }).unwrap();
        heap.insert(TestU32 { name: "egg".to_string(), data: 36 }).unwrap();
        heap.insert(TestU32 { name: "fan".to_string(), data: 100 }).unwrap();
        heap.insert(TestU32 { name: "hammer".to_string(), data: 19 }).unwrap();
        assert_eq!(heap.length(), 7);
    }
    // one key for length, two keys for each record
    assert_eq!(kv.inner.len(), 1 + 7 * 2 );

    // kv.inner.iter().for_each(|kv|{
    //     if kv.0[0] == 2 {
    //         println!("K: {:?} V: {:?}", kv.0, TestU32::from(kv.1.clone()));
    //     }

    // });

    // Extract elements
    {
        let mut heap = IndexHeap::<KVStore, TestU32>::new(vec![], &mut kv, 32);
        let v = heap.extract().unwrap();
        assert_eq!(TestU32::from(v), TestU32 { data: 5, name: "apple".to_string() });
        let v = heap.extract().unwrap();
        assert_eq!(TestU32::from(v), TestU32 { data: 12, name: "cat".to_string() });
        let v = heap.extract().unwrap();
        assert_eq!(TestU32::from(v), TestU32 { data: 17, name: "duck".to_string() });
        assert_eq!(heap.length(), 4);
    }
    assert_eq!(kv.inner.len(), 1 + 4 * 2 );

    // Change Key (decrease key)
    {
        let mut heap = IndexHeap::<KVStore, TestU32>::new(vec![], &mut kv, 32);
        heap.change_key(TestU32 { name: "egg".to_string(), data: 17 });
        assert_eq!(TestU32::from(heap.get(0).unwrap()), TestU32 { data: 17, name: "egg".to_string() });
        assert_eq!(heap.get_by("egg".as_bytes()).unwrap().data, 17);
        assert_eq!(heap.length(), 4);

        let values = heap.unordered_values();
        assert_eq!(values[0], TestU32 { data: 17, name: "egg".to_string() });
    }
    assert_eq!(kv.inner.len(), 1 + 4 * 2 );

    // Change Key (increase key)
    {
        let mut heap = IndexHeap::<KVStore, TestU32>::new(vec![], &mut kv, 32);
        heap.change_key(TestU32 { name: "egg".to_string(), data: 20 });
        assert_eq!(TestU32::from(heap.get(0).unwrap()), TestU32 { data: 19, name: "hammer".to_string() });
        assert_eq!(heap.get_by("egg".as_bytes()).unwrap().data, 20);
        assert_eq!(heap.length(), 4);

        let values = heap.unordered_values();
        assert_eq!(values[0], TestU32 { data: 19, name: "hammer".to_string() });
    }
    assert_eq!(kv.inner.len(), 1 + 4 * 2 );

    // Remove element
    {
        let mut heap = IndexHeap::<KVStore, TestU32>::new(vec![], &mut kv, 32);
        heap.remove_item("not exist".to_string().as_bytes());
        let values = heap.unordered_values();
        assert_eq!(values.len(), 4);
    }
    assert_eq!(kv.inner.len(), 1 + 4 * 2 );
    {
        let mut heap = IndexHeap::<KVStore, TestU32>::new(vec![], &mut kv, 32);
        heap.remove_item("duck".to_string().as_bytes()); // key existed before but removed later
        
        let values = heap.unordered_values();
        assert_eq!(values.len(), 4);
    }
    assert_eq!(kv.inner.len(), 1 + 4 * 2 );
    {
        let mut heap = IndexHeap::<KVStore, TestU32>::new(vec![], &mut kv, 32);
        heap.remove_item("egg".to_string().as_bytes()); // key to remove
        let values = heap.unordered_values();
        assert_eq!(values.len(), 3);
    }
    assert_eq!(kv.inner.len(), 1 + 3 * 2 );

    // Clear all
    {
        let mut heap = IndexHeap::<KVStore, TestU32>::new(vec![], &mut kv, 32);
        heap.clear();
        assert_eq!(heap.length(), 0);
    }
    assert_eq!(kv.inner.len(), 1);

    // Full Heap
    {
        let mut heap = IndexHeap::<KVStore, TestU32>::new(vec![], &mut kv, 65535);
        for i in 1..65536 {
            heap.insert(TestU32 { name: i.to_string(), data: i as u32 }).unwrap();
        }
        assert_eq!(heap.length(), 65535);
        assert!(heap.insert(TestU32 { name: 0.to_string(), data: 0 as u32 }).is_err());
        assert_eq!(heap.length(), 65535);
    }
    assert_eq!(kv.inner.len(), 1 + 65535 * 2 );

    // Insert Extract
    {
        let mut heap = IndexHeap::<KVStore, TestU32>::new(vec![], &mut kv, 65535);
        let result = heap.insert_extract(TestU32 { name: 0_u64.to_string(), data: 0 });
        assert!(result.is_err());
        assert_eq!(heap.length(), 65535);
        let result = heap.insert_extract(TestU32 { name: 65536_u32.to_string(), data: 65536 }).unwrap().unwrap();
        assert_eq!(result.name, 1_u32.to_string());
        assert_eq!(result.data, 1_u32);
        assert_eq!(heap.length(), 65535);
    }
    assert_eq!(kv.inner.len(), 1 + 65535 * 2 );
}