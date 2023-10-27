/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! The protocol defined constants and parameters used across ParallelChain F components.

/// Address of Network Account
pub const NETWORK_ADDRESS: pchain_types::cryptography::PublicAddress = [0u8; 32];
/// Maximum number of stakes delegated to a pool
pub const MAX_STAKES_PER_POOL: u16 = 128; // = 2^7
/// Maximum number of validators
pub const MAX_VALIDATOR_SET_SIZE: u16 = 64; // = 2^6
