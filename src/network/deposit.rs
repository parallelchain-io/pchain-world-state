/*
    Copyright © 2023, ParallelChain Lab 
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Definition of Key Format of Deposit in Storage of Network Account

use std::{convert::TryInto};

use super::{network_account::NetworkAccountStorage};

/// DepositDict defines key formatting for dictionary-like read-write operations to Deposit state in a Network Account.
pub struct DepositDict<'a, S>
    where S: NetworkAccountStorage
{
    pub(in crate::network) prefix_key: Vec<u8>,
    pub(in crate::network) world_state: &'a mut S
}

impl<'a, S> DepositDict<'a, S>
    where S: NetworkAccountStorage
{
    pub fn exists(&self) -> bool {
        let key = [self.prefix_key.as_slice(), &deposit_data::BALANCE].concat();
        self.world_state.contains(&key)
    }

    pub fn balance(&self) -> Option<u64> {
        let bytes = self.world_state.get(&[self.prefix_key.as_slice(), &deposit_data::BALANCE].concat())?;
        match bytes.try_into() {
            Ok(value) => Some(u64::from_le_bytes(value)),
            Err(_) => None
        }
    }

    pub fn set_balance(&mut self, balance: u64) {
        self.world_state.set(&[self.prefix_key.as_slice(), &deposit_data::BALANCE].concat(), balance.to_le_bytes().to_vec());
    }

    pub fn auto_stake_rewards(&self) -> Option<bool> {
        self.world_state.get(&[self.prefix_key.as_slice(), &deposit_data::AUTO_STAKE_REWARDS].concat()).map(|bytes|{
            bytes == [1u8; 1]
        })
    }

    pub fn set_auto_stake_rewards(&mut self, auto_stake_rewards: bool) {
        self.world_state.set(&[self.prefix_key.as_slice(), &deposit_data::AUTO_STAKE_REWARDS].concat(), if auto_stake_rewards { [1u8; 1] } else { [0u8; 1] }.to_vec() );
    }

    pub fn delete(&mut self) {
        for k in vec![
            [self.prefix_key.as_slice(), &deposit_data::BALANCE].concat(),
            [self.prefix_key.as_slice(), &deposit_data::AUTO_STAKE_REWARDS].concat(),
        ] {
            self.world_state.delete(k.as_slice());
        }
    }
}


mod deposit_data {
    pub const BALANCE: [u8; 1] = [0x0];
    pub const AUTO_STAKE_REWARDS: [u8; 1] = [0x1];
}