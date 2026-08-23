//! Name pool, object arena, classes-from-data, tagged-property access,
//! UnrealScript VM, native dispatch, world snapshot hashing.

/// Day-one identity marker; replaced by runtime tests from Phase 2.
pub const CRATE_NAME: &str = "hp-uobject";

#[cfg(test)]
mod smoke {
    #[test]
    fn links_and_runs() {
        assert_eq!(super::CRATE_NAME, "hp-uobject");
    }
}
