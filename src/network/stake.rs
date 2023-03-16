/*
    Copyright © 2023, ParallelChain Lab 
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Definition of Key Format of Stake in Storage of Network Account

use std::ops::{Deref, DerefMut};

use pchain_types::{Deserializable, Serializable};

use super::network_account::KeySpaced;


/// A Wrapper struct on [pchain_types::Stake] with implementation on Traits for orderring in data structures 
/// of Network Account such as IndexMap and IndexHeap.
#[derive(Clone, Eq)]
pub struct StakeValue {
    inner: pchain_types::Stake,
}

impl StakeValue {
    pub fn new(stake: pchain_types::Stake) -> Self {
        Self { inner: stake }
    }
}

impl KeySpaced for StakeValue {
    fn key(&self) -> &[u8] {
        // StakeValue belongs to Pools.stakes. Operator must be the same, hence owner is the only key
        &self.inner.owner
    }
}

impl PartialEq for StakeValue {
    fn eq(&self, other: &Self) -> bool {
        self.inner.power.eq(&other.inner.power)
    }
}

impl PartialOrd for StakeValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.inner.power.partial_cmp(&other.inner.power)
    }
}

impl Ord for StakeValue {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.inner.power.cmp(&other.inner.power)
    }
}

impl From<StakeValue> for Vec<u8> {
    fn from(stake_value: StakeValue) -> Self {
        pchain_types::Stake::serialize(&stake_value.inner)
    }
}

impl From<Vec<u8>> for StakeValue {
    fn from(bytes: Vec<u8>) -> Self {
        Self {
            inner: pchain_types::Stake::deserialize(&bytes).unwrap()
        }
    }
}

impl From<StakeValue> for pchain_types::Stake {
    fn from(stake_value: StakeValue) -> Self {
        stake_value.inner
    }
}

impl Deref for StakeValue {
    type Target = pchain_types::Stake;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for StakeValue {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

#[test]
fn test_stake() {
    let pool_1 = StakeValue {
        inner: pchain_types::Stake {
            power: 100,
            owner: [2u8; 32]
        }
    };
    let mut pool_2 = StakeValue {
        inner: pchain_types::Stake {
            power: 99,
            owner: [3u8; 32]
        }
    };

    assert!(pool_1.key() != pool_2.key());
    assert!(pool_1 > pool_2);
    assert!(pool_1 != pool_2);
    assert!(!(pool_1 <= pool_2));
    pool_2.inner.power = pool_1.inner.power;
    assert!(pool_1 == pool_2);
    assert!(!(pool_1 != pool_2));
    assert!(pool_1 >= pool_2);
    assert!(pool_1 <= pool_2);
    pool_2.inner.power = pool_1.inner.power + 1;
    assert!(pool_1 < pool_2);
    assert!(pool_1 != pool_2);
    assert!(!(pool_1 >= pool_2));

    let bytes_1: Vec<u8> = pool_1.into();
    let bytes_2: Vec<u8> = pool_2.into();
    let pool_1 = StakeValue::from(bytes_1);
    let pool_2 = StakeValue::from(bytes_2);
    assert!(pool_1 < pool_2);
}