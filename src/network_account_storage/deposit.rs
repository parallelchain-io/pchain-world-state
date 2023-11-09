/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Definition of Key Format of Deposit in Storage of Network Account

use std::convert::TryInto;

use pchain_types::serialization::{Deserializable, Serializable};

use super::network_account::NetworkAccountStorage;

/// Deposit is the locked balance of an account for a particular pool.
/// It determines the limit of voting power that the owner can delegate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub struct Deposit {
    /// Balance of this deposit
    pub balance: u64,
    /// Flag to indicate whether the received reward in epoch transaction should be automatically
    /// staked to the pool
    pub auto_stake_rewards: bool,
}

impl Serializable for Deposit {}
impl Deserializable for Deposit {}

/// DepositDict defines key formatting for dictionary-like read-write operations to Deposit state in a Network Account.
pub struct DepositDict<'a, S>
where
    S: NetworkAccountStorage,
{
    pub(in crate::network_account_storage) prefix_key: Vec<u8>,
    pub(in crate::network_account_storage) world_state: &'a mut S,
}

impl<'a, S> DepositDict<'a, S>
where
    S: NetworkAccountStorage,
{
    pub fn exists(&mut self) -> bool {
        let key = [self.prefix_key.as_slice(), &deposit_data::BALANCE].concat();
        self.world_state.contains(&key)
    }

    pub fn balance(&mut self) -> Option<u64> {
        let bytes = self
            .world_state
            .get(&[self.prefix_key.as_slice(), &deposit_data::BALANCE].concat())?;
        match bytes.try_into() {
            Ok(value) => Some(u64::from_le_bytes(value)),
            Err(_) => None,
        }
    }

    pub fn set_balance(&mut self, balance: u64) {
        self.world_state.set(
            &[self.prefix_key.as_slice(), &deposit_data::BALANCE].concat(),
            balance.to_le_bytes().to_vec(),
        );
    }

    pub fn auto_stake_rewards(&mut self) -> Option<bool> {
        self.world_state
            .get(
                &[
                    self.prefix_key.as_slice(),
                    &deposit_data::AUTO_STAKE_REWARDS,
                ]
                .concat(),
            )
            .map(|bytes| bytes == [1u8; 1])
    }

    pub fn set_auto_stake_rewards(&mut self, auto_stake_rewards: bool) {
        self.world_state.set(
            &[
                self.prefix_key.as_slice(),
                &deposit_data::AUTO_STAKE_REWARDS,
            ]
            .concat(),
            if auto_stake_rewards {
                [1u8; 1]
            } else {
                [0u8; 1]
            }
            .to_vec(),
        );
    }

    pub fn delete(&mut self) {
        for k in vec![
            [self.prefix_key.as_slice(), &deposit_data::BALANCE].concat(),
            [
                self.prefix_key.as_slice(),
                &deposit_data::AUTO_STAKE_REWARDS,
            ]
            .concat(),
        ] {
            self.world_state.delete(k.as_slice());
        }
    }
}

mod deposit_data {
    pub const BALANCE: [u8; 1] = [0x0];
    pub const AUTO_STAKE_REWARDS: [u8; 1] = [0x1];
}
