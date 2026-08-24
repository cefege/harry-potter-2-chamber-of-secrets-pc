//! Name pool, object arena, classes-from-data, tagged-property access,
//! UnrealScript VM, native dispatch, world snapshot hashing.

pub mod arena;
pub mod bind;
pub mod bootstrap;
pub mod error;
pub mod loader;
pub mod name;
pub mod natives;
pub mod props;
pub mod snapshot;
pub mod value;
pub mod vm;

/// Day-one identity marker; replaced by runtime tests from Phase 2.
pub const CRATE_NAME: &str = "hp-uobject";
