/*
    Copyright © 2023, ParallelChain Lab 
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! IndexMap implements a reverse indexed Map over a key-value data source (see. [NetworkAccountStorage]).

use std::{marker::PhantomData, convert::TryInto};

use super::network_account::{NetworkAccountStorage, KeySpaced};

/// IndexMap is an indexed Map that supports:
/// - get - get item by index 
/// - get_by - get item by key
/// - push - push item to the Map
pub struct IndexMap<'a, T, V> 
where 
    T: NetworkAccountStorage,
    V: Clone + PartialEq + Eq + Into<Vec<u8>> + From<Vec<u8>> + KeySpaced
{
    pub(in crate::network) store: &'a mut T,
    pub(in crate::network) domain: Vec<u8>,
    pub(in crate::network) capacity: u32,
    _phantom: PhantomData<V>
}

impl<'a, T, V> IndexMap<'a, T, V> 
where 
    T: NetworkAccountStorage,
    V: Clone + PartialEq + Eq + Into<Vec<u8>> + From<Vec<u8>> + KeySpaced
{
    pub(in crate::network) const PREFIX_LEN: [u8; 1] = [0u8];
    pub(in crate::network) const PREFIX_KEY_INDEX: [u8; 1] = [1u8];
    pub(in crate::network) const PREFIX_INDEX_VALUE: [u8; 1] = [2u8];


    pub(in crate::network) fn new(domain: Vec<u8>, store: &'a mut T, capacity: u32) -> Self {
        Self {
            store, domain, capacity, _phantom: PhantomData
        }
    }
    
    /// length of the IndexMap. Lenght of an empty or uninitialized IndexMap = 0.
    pub fn length(&self) -> u32 {
        let key_len = [self.domain.as_slice(), &Self::PREFIX_LEN].concat();
        self.store.get(&key_len).map_or(0, |length_bytes|{
            u32::from_le_bytes(length_bytes.try_into().unwrap())
        })
    }

    /// get value by key from IndexMap
    pub fn get_by(&self, key: &[u8]) -> Option<V> {
        let index = self.index_of_key(key)?;
        self.get(index)
    }

    /// push value to IndexMap
    pub fn push(&mut self, value: V) -> Result<(), IndexMapOperationError> {
        let len = self.length();
        if len >= self.capacity {
            return Err(IndexMapOperationError)
        }

        self.set(len, value);
        self.set_length(len + 1);

        Ok(())
    }

    /// Set all values to the index map from beginning. Equivalent to clear all and then push all.
    pub fn reset(&mut self, value: Vec<V>) -> Result<(), IndexMapOperationError> {
        let v_len = value.len() as u32;
        if v_len > self.capacity {
            return Err(IndexMapOperationError)
        }
        
        // Clear the previous values
        self.clear();

        // Set the new values
        value.into_iter().enumerate().for_each(|(i, v)|{
            self.set(i as u32, v);
        });
        self.set_length(v_len);

        Ok(())
    }

    pub(in crate::network) fn set_length(&mut self, length: u32) -> u32 {
        let key_len = [self.domain.as_slice(), &Self::PREFIX_LEN].concat();
        self.store.set(&key_len, length.to_le_bytes().to_vec());
        length
    }
    
    pub(in crate::network) fn index_of_key(&self, key: &[u8]) -> Option<u32> {
        let key_ki = [self.domain.as_slice(), &Self::PREFIX_KEY_INDEX, key].concat();
        self.store.get(&key_ki).map(|index_bytes| u32::from_le_bytes(index_bytes.try_into().unwrap()))
    }

    /// get value by index from IndexMap. Return None if 
    /// 1. inputted index exceeds capacity
    /// 2. item is not found (unreachable)
    pub fn get(&self, index: u32) -> Option<V> {
        if index >= self.capacity { return None }

        let key_iv = [self.domain.as_slice(), &Self::PREFIX_INDEX_VALUE, index.to_le_bytes().as_ref()].concat();
        self.store.get(&key_iv).map(|value_types| value_types.into())
    }

    /// Set performs writes:
    /// - KI\[value.key\] = index
    /// - IV\[index\] = value
    pub(in crate::network) fn set(&mut self, index: u32, value: V) {
        let key_iv = [self.domain.as_slice(), &Self::PREFIX_INDEX_VALUE, index.to_le_bytes().as_ref()].concat();
        let key_ki = [self.domain.as_slice(), &Self::PREFIX_KEY_INDEX, value.key()].concat();

        self.store.set(&key_ki, index.to_le_bytes().to_vec());
        self.store.set(&key_iv, value.into());
    }

    pub(in crate::network) fn delete(&mut self, index: u32, key: &[u8]) {
        let key_iv = [self.domain.as_slice(), &Self::PREFIX_INDEX_VALUE, index.to_le_bytes().as_ref()].concat();
        let key_ki = [self.domain.as_slice(), &Self::PREFIX_KEY_INDEX, key].concat();

        self.store.delete(&key_ki);
        self.store.delete(&key_iv);
    }

    pub fn clear(&mut self) {
        let len = self.length();
        for i in 0..len {
            let v = self.get(i).unwrap();
            self.delete(i, v.key());
        }
        self.set_length(0);
    }

}

#[derive(Debug)]
pub struct IndexMapOperationError;

#[test]
fn test_index_map() {
    use std::collections::HashMap;
    #[derive(Clone, Debug, Eq)]
    struct TestString { name: String}
    impl Into<Vec<u8>> for TestString {
        fn into(self) -> Vec<u8> {
            use pchain_types::Serializable;
            Vec::<u8>::serialize(&self.name.as_bytes().to_vec())
        }
    }
    impl From<Vec<u8>> for TestString {
        fn from(bytes: Vec<u8>) -> Self {
            use pchain_types::Deserializable;
            let r = Vec::<u8>::deserialize(&bytes).unwrap();
            Self { name: String::from_utf8(r).unwrap() }
        }
    }
    impl PartialEq for TestString {
        fn eq(&self, other: &Self) -> bool {
            self.name.eq(&other.name)
        }
    }
    impl KeySpaced for TestString {
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

    // push item
    {
        let mut map = IndexMap::<KVStore, TestString>::new(vec![], &mut kv, 32);
        assert_eq!(map.length(), 0);
        assert_eq!(map.get(0), None);
        map.push(TestString { name: "apple".to_string() }).unwrap();
        assert_eq!(map.length(), 1);
        assert_eq!(map.get(0).unwrap().name, "apple".to_string());
        assert!(map.get_by("apple".as_bytes()).is_some());
        assert!(map.get_by("not exist".as_bytes()).is_none());
    }
    assert_eq!(kv.inner.len(), 1 + 1 * 2);

    // delete item
    {
        let mut map = IndexMap::<KVStore, TestString>::new(vec![], &mut kv, 32);
        map.delete(0, "apple".as_bytes());
        assert!(map.get_by("apple".as_bytes()).is_none());

        map.set_length(0);
        assert_eq!(map.length(), 0);
    }
    assert_eq!(kv.inner.len(), 1 + 0 * 2);

    // full map
    {
        let mut map = IndexMap::<KVStore, TestString>::new(vec![], &mut kv, 65535);
        for i in 0..65535 {
            map.push(TestString { name: i.to_string() }).unwrap();
        }
        assert_eq!(map.length(), 65535);
    }
    assert_eq!(kv.inner.len(), 1 + 65535 * 2);

    // failed to insert
    {
        let mut map = IndexMap::<KVStore, TestString>::new(vec![], &mut kv, 65535);
        assert!(map.push(TestString { name: "MAX".to_string() }).is_err());
        assert_eq!(map.length(), 65535);
    }
    assert_eq!(kv.inner.len(), 1 + 65535 * 2);


}