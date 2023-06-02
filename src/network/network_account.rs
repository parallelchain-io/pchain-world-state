/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! network_account defines key formatting for read-write operations to Network Account Storage.
//! It is a sub-format under keyspace format of [WorldState](crate::states::WorldState).

use std::convert::TryInto;

use pchain_types::cryptography::PublicAddress;

use super::{
    constants::{MAX_STAKES_PER_POOL, MAX_VALIDATOR_SET_SIZE},
    deposit::DepositDict,
    index_heap::IndexHeap,
    pool::{PoolDict, PoolKey, ValidatorPool},
};

/// Network Account with space size following to constants defined in protocol
pub type NetworkAccount<'a, S> =
    NetworkAccountSized<'a, S, { MAX_VALIDATOR_SET_SIZE }, { MAX_STAKES_PER_POOL }>;

/// A trait for key-value data source implementation of Network Account Storage.
pub trait NetworkAccountStorage {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
    fn contains(&self, key: &[u8]) -> bool;
    fn set(&mut self, key: &[u8], value: Vec<u8>);
    fn delete(&mut self, key: &[u8]);
}

/// A trait of key definition that is used in Network Account specific structures such as IndexHeap and IndexMap.
pub trait KeySpaced {
    fn key(&self) -> &[u8];
}

/// Network Account with generic constants.
pub struct NetworkAccountSized<'a, S, const N: u16, const M: u16>
where
    S: NetworkAccountStorage,
{
    world_state: &'a mut S,
}

impl<'a, S, const N: u16, const M: u16> NetworkAccountSized<'a, S, N, M>
where
    S: NetworkAccountStorage,
{
    pub fn new(world_state: &'a mut S) -> Self {
        Self { world_state }
    }

    /// Previous Validator Pools
    pub fn pvp(world_state: &mut S) -> ValidatorPool<S, N, M> {
        ValidatorPool::new(
            world_state,
            network_account_data::PREV_VALIDATOR_POOLS.to_vec(),
        )
    }

    /// Current Validator Pools
    pub fn vp(world_state: &mut S) -> ValidatorPool<S, N, M> {
        ValidatorPool::new(world_state, network_account_data::VALIDATOR_POOLS.to_vec())
    }

    /// Next Validator Pools. It returns [PoolKey] instead of a complete structure of a pool.
    /// The pools information can be access via method pools().
    pub fn nvp(world_state: &mut S) -> IndexHeap<S, PoolKey> {
        IndexHeap::<S, PoolKey>::new(
            network_account_data::NEXT_VALIDATOR_POOLS.to_vec(),
            world_state,
            N as u32,
        )
    }

    pub fn pools(world_state: &mut S, operator: PublicAddress) -> PoolDict<S, M> {
        let prefix_key = [network_account_data::POOLS.as_slice(), &operator].concat();
        PoolDict {
            prefix_key,
            world_state,
        }
    }

    pub fn deposits(
        world_state: &mut S,
        operator: PublicAddress,
        owner: PublicAddress,
    ) -> DepositDict<S> {
        let prefix_key = [network_account_data::DEPOSITS.as_slice(), &operator, &owner].concat();
        DepositDict {
            prefix_key,
            world_state,
        }
    }

    pub fn current_epoch(&self) -> u64 {
        let value = self.world_state.get(&network_account_data::CURRENT_EPOCH);
        match value {
            Some(bytes) => u64::from_le_bytes(bytes.try_into().unwrap()),
            None => 0,
        }
    }

    pub fn set_current_epoch(&mut self, current_epoch: u64) {
        self.world_state.set(
            &network_account_data::CURRENT_EPOCH,
            current_epoch.to_le_bytes().to_vec(),
        );
    }
}

mod network_account_data {
    pub const PREV_VALIDATOR_POOLS: [u8; 1] = [0x00];
    pub const VALIDATOR_POOLS: [u8; 1] = [0x01];
    pub const NEXT_VALIDATOR_POOLS: [u8; 1] = [0x02];
    pub const POOLS: [u8; 1] = [0x03]; // = NEXT_VALIDATOR_POOLS_
    pub const DEPOSITS: [u8; 1] = [0x04];
    pub const CURRENT_EPOCH: [u8; 1] = [0x05];
}
