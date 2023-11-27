/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Definition of Key Format of Pool in Storage of Network Account

use pchain_types::{
    cryptography::PublicAddress,
    serialization::{Deserializable, Serializable},
};
use std::{
    convert::{TryFrom, TryInto},
    ops::Deref,
};

use super::{
    index_heap::IndexHeap,
    index_map::{IndexMap, IndexMapOperationError},
    network_account::{KeySpaced, NetworkAccountStorage},
    stake::{Stake, StakeValue},
};

/// Pool is the place that stake owners can stake to.
#[derive(Debug, Clone, PartialEq, Eq, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub struct Pool {
    /// Address of the pool's operator
    pub operator: PublicAddress,
    /// Commission rate (in unit of percentage) is the portion that
    /// the owners of its delegated stakes should pay from the reward in an epoch transaction.
    pub commission_rate: u8,
    /// Pool's power that determines the eligibility to be one of the validators
    pub power: u64,
    /// Operator's own stake
    pub operator_stake: Option<Stake>,
}

impl Serializable for Pool {}
impl Deserializable for Pool {}

/// PoolDict defines key formatting for dictionary-like read-write operations to Pool state in a Network Account.
pub struct PoolDict<'a, S, const M: u16>
where
    S: NetworkAccountStorage,
{
    pub(in crate::network_account_storage) prefix_key: Vec<u8>,
    pub(in crate::network_account_storage) world_state: &'a mut S,
}

impl<'a, S, const M: u16> PoolDict<'a, S, M>
where
    S: NetworkAccountStorage,
{
    pub fn exists(&mut self) -> bool {
        let key = [self.prefix_key.as_slice(), &pool_data::OPERATOR].concat();
        self.world_state.contains(&key)
    }

    pub fn operator(&mut self) -> Option<PublicAddress> {
        let bytes = self
            .world_state
            .get(&[self.prefix_key.as_slice(), &pool_data::OPERATOR].concat())?;
        match bytes.try_into() {
            Ok(address) => Some(address),
            Err(_) => None,
        }
    }

    pub fn set_operator(&mut self, operator: PublicAddress) {
        self.world_state.set(
            &[self.prefix_key.as_slice(), &pool_data::OPERATOR].concat(),
            operator.to_vec(),
        );
    }

    pub fn power(&mut self) -> Option<u64> {
        let bytes = self
            .world_state
            .get(&[self.prefix_key.as_slice(), &pool_data::POWER].concat())?;
        match bytes.try_into() {
            Ok(value) => Some(u64::from_le_bytes(value)),
            Err(_) => None,
        }
    }

    pub fn set_power(&mut self, power: u64) {
        self.world_state.set(
            &[self.prefix_key.as_slice(), &pool_data::POWER].concat(),
            power.to_le_bytes().to_vec(),
        );
    }

    pub fn commission_rate(&mut self) -> Option<u8> {
        let bytes = self
            .world_state
            .get(&[self.prefix_key.as_slice(), &pool_data::COMMISSION_RATE].concat())?;
        match bytes.try_into() {
            Ok(value) => Some(u8::from_le_bytes(value)),
            Err(_) => None,
        }
    }

    pub fn set_commission_rate(&mut self, commission_rate: u8) {
        self.world_state.set(
            &[self.prefix_key.as_slice(), &pool_data::COMMISSION_RATE].concat(),
            commission_rate.to_le_bytes().to_vec(),
        );
    }

    pub fn operator_stake(&mut self) -> Option<Option<Stake>> {
        self.world_state
            .get(&[self.prefix_key.as_slice(), &pool_data::OPERATOR_STAKE].concat())
            .map(|bytes| Option::<Stake>::deserialize(&bytes).unwrap())
    }

    pub fn set_operator_stake(&mut self, stake: Option<Stake>) {
        self.world_state.set(
            &[self.prefix_key.as_slice(), &pool_data::OPERATOR_STAKE].concat(),
            Option::<Stake>::serialize(&stake),
        );
    }

    pub fn delegated_stakes(&mut self) -> IndexHeap<S, StakeValue> {
        IndexHeap::<S, StakeValue>::new(
            [self.prefix_key.as_slice(), &pool_data::DELEGATED_STAKES].concat(),
            self.world_state,
            M as u32,
        )
    }

    pub fn delete(&'a mut self) {
        for k in [
            [self.prefix_key.as_slice(), &pool_data::OPERATOR].concat(),
            [self.prefix_key.as_slice(), &pool_data::POWER].concat(),
            [self.prefix_key.as_slice(), &pool_data::COMMISSION_RATE].concat(),
            [self.prefix_key.as_slice(), &pool_data::OPERATOR_STAKE].concat(),
        ] {
            self.world_state.delete(k.as_slice());
        }

        self.delegated_stakes().clear();
    }
}

impl<'a, S, const M: u16> TryFrom<PoolDict<'a, S, M>> for Pool
where
    S: NetworkAccountStorage,
{
    type Error = ();
    fn try_from(mut pool: PoolDict<'a, S, M>) -> Result<Self, Self::Error> {
        Ok(Pool {
            operator: pool.operator().ok_or(())?,
            commission_rate: pool.commission_rate().ok_or(())?,
            power: pool.power().ok_or(())?,
            operator_stake: pool.operator_stake().ok_or(())?,
        })
    }
}

/// ValidatorPool defines the pool value to be stored in state of a Network Account.
/// Different from [PoolDict], fields are stored as a single value in the Key-Value storage,
/// rather than assigning keyspaces to each field as a dictionary.
pub struct ValidatorPool<'a, S, const N: u16, const M: u16>
where
    S: NetworkAccountStorage,
{
    inner: IndexMap<'a, S, PoolAddress>,
}

impl<'a, S, const N: u16, const M: u16> ValidatorPool<'a, S, N, M>
where
    S: NetworkAccountStorage,
{
    /// A single byte prefix key to partition the Pool values (as a nested IndexMap) and IndexMap ValidatorPool itself.
    /// ### Cautions
    /// The value 3u8 is chosen because the value 0u8, 1u8 and 2u8 are already chosen in IndexMap.
    pub(in crate::network_account_storage) const PREFIX_NESTED_MAP: [u8; 1] = [3u8];

    pub(in crate::network_account_storage) fn new(
        world_state: &'a mut S,
        prefix_key: Vec<u8>,
    ) -> Self {
        Self {
            inner: IndexMap::<S, PoolAddress>::new(prefix_key, world_state, N as u32),
        }
    }

    pub fn length(&mut self) -> u32 {
        self.inner.length()
    }

    pub fn pool(&mut self, operator: PublicAddress) -> Option<PoolDict<S, M>> {
        self.inner.get_by(PoolAddress(operator).key())?;

        Some(PoolDict {
            prefix_key: [
                self.inner.domain.as_slice(),
                &Self::PREFIX_NESTED_MAP,
                operator.as_slice(),
            ]
            .concat(),
            world_state: self.inner.store,
        })
    }

    pub fn pool_at(&mut self, index: u32) -> Option<PoolDict<S, M>> {
        let pool_address = self.inner.get(index)?;
        self.pool(pool_address.into())
    }

    /// Push pool value to Index Map with reset of delegated stakes.
    pub fn push(
        &'a mut self,
        pool: Pool,
        delegated_stakes: Vec<StakeValue>,
    ) -> Result<(), IndexMapOperationError> {
        // push pool address to the list first
        self.inner.push(PoolAddress(pool.operator))?;

        // set pool value
        let mut pool_dict = self.pool(pool.operator).unwrap();
        pool_dict.set_operator(pool.operator);
        pool_dict.set_power(pool.power);
        pool_dict.set_commission_rate(pool.commission_rate);
        pool_dict.set_operator_stake(pool.operator_stake);

        // set delegated stakes
        let _ = pool_dict.delegated_stakes().reset(delegated_stakes);
        Ok(())
    }

    /// Clear pool and its delegated stakes.
    pub fn clear(&'a mut self) {
        let pool_length = self.length();
        for i in 0..pool_length {
            let mut pool = self.pool_at(i).unwrap();
            pool.delegated_stakes().clear();
            pool.delete();
        }
        self.inner.set_length(0);
    }

    pub fn get(&mut self, index: u32) -> Option<PoolAddress> {
        self.inner.get(index)
    }
}

mod pool_data {
    pub const OPERATOR: [u8; 1] = [0x0];
    pub const POWER: [u8; 1] = [0x1];
    pub const COMMISSION_RATE: [u8; 1] = [0x2];
    pub const OPERATOR_STAKE: [u8; 1] = [0x3];
    pub const DELEGATED_STAKES: [u8; 1] = [0x4];
}

/// PoolAddress is the value store inside the IndexMap for Validator Set (Previous and Current).
/// Different with [PoolKey], power is not needed because PVP and VP do not need to implement a binary heap.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PoolAddress(pub PublicAddress);

impl KeySpaced for PoolAddress {
    fn key(&self) -> &[u8] {
        &self.0
    }
}

impl From<PoolAddress> for Vec<u8> {
    fn from(value: PoolAddress) -> Vec<u8> {
        value.0.to_vec()
    }
}

impl From<Vec<u8>> for PoolAddress {
    fn from(value: Vec<u8>) -> Self {
        Self(value.try_into().unwrap())
    }
}

impl From<PoolAddress> for PublicAddress {
    fn from(value: PoolAddress) -> Self {
        value.0
    }
}

impl Deref for PoolAddress {
    type Target = PublicAddress;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone)]
/// PoolKey is a small description of a pool. It affects the order of its representing pool in the Index Heap which is the state format of the next validator set.
pub struct PoolKey {
    pub operator: PublicAddress,
    pub power: u64,
}

impl PoolKey {
    pub fn new(operator: PublicAddress, power: u64) -> Self {
        Self { operator, power }
    }
}

impl KeySpaced for PoolKey {
    fn key(&self) -> &[u8] {
        self.operator.as_slice()
    }
}

impl Eq for PoolKey {}

impl PartialEq for PoolKey {
    fn eq(&self, other: &Self) -> bool {
        match self.power.eq(&other.power) {
            true => self.operator.eq(&other.operator),
            false => false,
        }
    }
}

impl PartialOrd for PoolKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.power.partial_cmp(&other.power) {
            Some(std::cmp::Ordering::Equal) => self.operator.partial_cmp(&other.operator),
            Some(compare) => Some(compare),
            None => None,
        }
    }
}

impl Ord for PoolKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.power.cmp(&other.power) {
            std::cmp::Ordering::Equal => self.operator.cmp(&other.operator),
            compare => compare,
        }
    }
}

impl From<PoolKey> for Vec<u8> {
    fn from(pool_key: PoolKey) -> Self {
        <(Vec<u8>, u64)>::serialize(&(pool_key.operator.to_vec(), pool_key.power))
    }
}

impl From<Vec<u8>> for PoolKey {
    fn from(bytes: Vec<u8>) -> Self {
        let (operator, power) = <(Vec<u8>, u64)>::deserialize(&bytes).unwrap();
        Self {
            operator: operator.try_into().unwrap(),
            power,
        }
    }
}
