/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! This mod provide struct and implementation of Version

/// `Version` is to identify the different between the old version WorldState and new version WorldState.
/// V1 is the old version and V2 is the new version
#[derive(Debug, Clone)]
pub enum Version {
    V1,
    V2,
}

/// Define the methods that a type must implemented to be version indication of [WorldState](crate::world_state::WorldState).
/// The method `version` must be implemented in order indicat which [WorldState](crate::world_state::WorldState) version is
pub trait VersionProvider {
    fn version() -> Version;
}

/// Old version [WorldState](crate::world_state::WorldState)
#[derive(Debug, Clone)]
pub struct V1;

impl VersionProvider for V1 {
    fn version() -> Version {
        Version::V1
    }
}

/// New version [WorldState](crate::world_state::WorldState)
#[derive(Debug, Clone)]
pub struct V2;

impl VersionProvider for V2 {
    fn version() -> Version {
        Version::V2
    }
}
