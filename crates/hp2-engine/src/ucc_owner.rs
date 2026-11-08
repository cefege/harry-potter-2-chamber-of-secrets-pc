//! Rust UCC execution on the canonical Core + Engine owner graph.

use hp2_core::core_access::RuntimeResult;
use hp2_core::core_ucc::{UccOwner, run_with_owner};
use std::ffi::OsString;

impl UccOwner for crate::core_owner::EngineOwner {
    fn access(&self) -> hp2_core::core_access::CoreAccess<'_> {
        crate::core_owner::EngineOwner::access(self)
    }

    fn pre_exit(&self) -> RuntimeResult<()> {
        crate::core_owner::EngineOwner::pre_exit(self)
    }
}

/// Runs the shared Rust UCC session over the full canonical Engine object graph.
pub fn run(args: impl IntoIterator<Item = OsString>) -> i32 {
    run_with_owner(args, crate::core_owner::EngineOwner::new)
}
