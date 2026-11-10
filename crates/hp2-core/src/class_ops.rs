//! Per-dispatch-root native-override records ("ops"), one flattened copy per
//! native `UClass`, resolved once at class registration/link time and read
//! directly by Rust call sites instead of walking the C++ virtual-dispatch
//! Super chain or crossing back into C++ for a method the port already owns.
//!
//! This is the M1 "ops-first virtual bridge" infrastructure
//! (`local://m1-dispatch-design.md` §3.2): as classes in a dispatch-root
//! family (`AActor`, ...) move to Rust one at a time, a Rust-side caller that
//! already holds the object can skip the remaining C++ virtual shells by
//! checking this table instead. C++ callers are unaffected -- the C++
//! `UClass::ClassConstructor`/vtable substitution from `object_construction.rs`
//! still drives them unchanged.
//!
//! Keyed by the native `UClass*` (stable for the process lifetime, never
//! relocated or freed), not by name: no string lookup on the hot path. Native
//! classes register explicitly, once, right after `UClass` construction.
//! Script subclasses never call the register entry point directly; instead
//! [`crate::property_link::bind`] copies the nearest bound ancestor's record
//! at the exact point it already copies `ClassConstructor` from that ancestor
//! (`UClass::Bind`), so a script subclass of a Rust-flipped class gets the
//! same flattened dispatch for free.
//!
//! Function pointers are `extern "C"` only so this module never needs to name
//! an upper-layer crate's Rust types (`hp2-core` has no dependency on
//! `hp2-engine`); callers in `hp2-engine` store small `extern "C"` adapters
//! around their own typed functions. Calling through one of these pointers
//! never crosses into C++ -- it is a same-process, same-binary indirect call,
//! not an FFI hop.
//!
//! **This module is the class->ops registry, standalone, not a field on
//! [`crate::class_metadata::MetadataStorage`].** An earlier draft of
//! `local://m1-dispatch-design.md` §3.2 assumed `MetadataStorage` already
//! held one record per native class and proposed adding the ops field there;
//! that premise is false. `MetadataStorage` (`class_metadata.rs`) is a
//! *loading-scope* struct -- "loading-layout scope shared only by calls
//! using this storage instance" -- a single transient value threaded through
//! one load/link call, not a per-`UClass` table. It has no mechanism to look
//! up a record by `UClass*` after the fact, which is this module's entire
//! purpose. This table, keyed directly by the existing `UClass*`, is
//! therefore the only class->ops registry and is not slated to migrate into
//! `class_metadata.rs`.

use crate::fast_hash::FastMap as HashMap;
use std::ffi::c_void;
use std::sync::{LazyLock, Mutex};

/// Dispatch-root ops for the `AActor` family. One flattened copy per native
/// `AActor`-rooted `UClass`. Slots default to `None` (defer to the existing
/// C++ virtual call) and are filled in only for the methods that class (or
/// an ancestor whose record it inherited) actually overrides in Rust.
#[derive(Clone, Copy, Default)]
pub struct ActorOps {
    /// `AActor::UpdateFrequency` override, if this class (or an inherited
    /// ancestor record) replaces it. Takes the live actor object.
    pub update_frequency: Option<unsafe extern "C" fn(*const c_void) -> f32>,
}

static ACTOR_OPS_TABLE: LazyLock<Mutex<HashMap<usize, ActorOps>>> =
    LazyLock::new(|| Mutex::new(HashMap::default()));

fn lock_actor_ops_table() -> std::sync::MutexGuard<'static, HashMap<usize, ActorOps>> {
    ACTOR_OPS_TABLE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Registers `class`'s flattened `ActorOps` record: `ancestor`'s already-
/// flattened record (or every slot `None`, for the first Rust-registered
/// class in the family) with `overrides`'s non-`None` slots applied on top.
///
/// Must run exactly once, right after `class`'s `UClass` is constructed and
/// registered -- the same point the deleted C++ class's own
/// `IMPLEMENT_CLASS`/`StaticClass()` registration used to run.
///
/// # Safety
/// `class` must be the live native `UClass*` just constructed for this
/// class, with a stable address for the remainder of the process. `ancestor`
/// must be null (no Rust-registered ancestor) or a live native `UClass*`
/// already registered via this function.
pub unsafe fn register_actor_ops(class: *mut c_void, ancestor: *mut c_void, overrides: ActorOps) {
    let mut table = lock_actor_ops_table();
    let mut base = if ancestor.is_null() {
        ActorOps::default()
    } else {
        table.get(&(ancestor as usize)).copied().unwrap_or_default()
    };
    if overrides.update_frequency.is_some() {
        base.update_frequency = overrides.update_frequency;
    }
    table.insert(class as usize, base);
}

/// Copies `ancestor`'s flattened `ActorOps` record (if any) onto `class`
/// verbatim. Called from [`crate::property_link::bind`] at the exact point
/// `UClass::Bind` already inherits `ClassConstructor` from `ancestor` for a
/// freshly-loaded script subclass. A no-op if `ancestor` has no record (the
/// family has no Rust-registered class in `class`'s ancestry yet).
///
/// # Safety
/// `class` and `ancestor` must each be null or a live native `UClass*` with
/// a stable address for the remainder of the process.
pub unsafe fn inherit_actor_ops(class: *mut c_void, ancestor: *mut c_void) {
    if class.is_null() || ancestor.is_null() {
        return;
    }
    let mut table = lock_actor_ops_table();
    match table.get(&(ancestor as usize)).copied() {
        Some(ops) => {
            table.insert(class as usize, ops);
        }
        None => {
            // No Rust-flipped class in the ancestor's chain (yet) -- or
            // `class`'s address was recycled from a since-GC'd `UClass` that
            // did have a stale record (map-local script classes reuse
            // addresses). Either way `class` must not keep a record that no
            // longer belongs to it.
            table.remove(&(class as usize));
        }
    }
}

/// Looks up `class`'s flattened `ActorOps` record. Returns `None` if `class`
/// has no Rust-registered native ancestor yet, in which case the caller must
/// fall back to the existing (C++) dispatch path.
pub fn actor_ops(class: *mut c_void) -> Option<ActorOps> {
    if class.is_null() {
        return None;
    }
    lock_actor_ops_table().get(&(class as usize)).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    unsafe extern "C" fn probe_a(_actor: *const c_void) -> f32 {
        1.0
    }
    unsafe extern "C" fn probe_b(_actor: *const c_void) -> f32 {
        2.0
    }

    #[test]
    fn unregistered_class_has_no_record() {
        let class = 0x1000 as *mut c_void;
        assert!(actor_ops(class).is_none());
    }

    #[test]
    fn first_registered_class_in_a_family_starts_from_defaults() {
        let class = 0x1010 as *mut c_void;
        unsafe {
            register_actor_ops(
                class,
                std::ptr::null_mut(),
                ActorOps {
                    update_frequency: Some(probe_a),
                },
            );
        }
        let ops = actor_ops(class).expect("registered");
        assert_eq!(
            ops.update_frequency.unwrap() as usize,
            probe_a as *const () as usize
        );
    }

    #[test]
    fn unset_slots_are_inherited_from_the_ancestor_record() {
        let ancestor = 0x1020 as *mut c_void;
        let child = 0x1021 as *mut c_void;
        unsafe {
            register_actor_ops(
                ancestor,
                std::ptr::null_mut(),
                ActorOps {
                    update_frequency: Some(probe_a),
                },
            );
            // The child overrides nothing of its own: its record must still
            // carry the ancestor's update_frequency slot forward.
            register_actor_ops(child, ancestor, ActorOps::default());
        }
        let ops = actor_ops(child).expect("registered");
        assert_eq!(
            ops.update_frequency.unwrap() as usize,
            probe_a as *const () as usize
        );
    }

    #[test]
    fn set_slots_override_the_ancestor_record() {
        let ancestor = 0x1030 as *mut c_void;
        let child = 0x1031 as *mut c_void;
        unsafe {
            register_actor_ops(
                ancestor,
                std::ptr::null_mut(),
                ActorOps {
                    update_frequency: Some(probe_a),
                },
            );
            register_actor_ops(
                child,
                ancestor,
                ActorOps {
                    update_frequency: Some(probe_b),
                },
            );
        }
        let ops = actor_ops(child).expect("registered");
        assert_eq!(
            ops.update_frequency.unwrap() as usize,
            probe_b as *const () as usize
        );
    }

    #[test]
    fn script_subclass_inherits_the_nearest_native_ancestor_record() {
        let native = 0x1040 as *mut c_void;
        let script_child = 0x1041 as *mut c_void;
        unsafe {
            register_actor_ops(
                native,
                std::ptr::null_mut(),
                ActorOps {
                    update_frequency: Some(probe_a),
                },
            );
            // This is the call `property_link::bind` makes for a freshly
            // loaded script class, not `register_actor_ops`.
            inherit_actor_ops(script_child, native);
        }
        let ops = actor_ops(script_child).expect("inherited");
        assert_eq!(
            ops.update_frequency.unwrap() as usize,
            probe_a as *const () as usize
        );
    }

    #[test]
    fn inherit_from_an_unregistered_ancestor_is_a_no_op() {
        let ancestor = 0x1050 as *mut c_void;
        let child = 0x1051 as *mut c_void;
        unsafe {
            inherit_actor_ops(child, ancestor);
        }
        assert!(actor_ops(child).is_none());
    }

    #[test]
    fn inherit_from_an_unregistered_ancestor_clears_a_stale_record_at_a_reused_address() {
        let ancestor = 0x1060 as *mut c_void;
        let reused = 0x1061 as *mut c_void;
        unsafe {
            // `reused` previously held a record under a different, since-GC'd
            // `UClass` that was itself registered.
            register_actor_ops(
                reused,
                std::ptr::null_mut(),
                ActorOps {
                    update_frequency: Some(probe_a),
                },
            );
            // The address is now a plain script class with no Rust-flipped
            // ancestor: the stale record must not leak into it.
            inherit_actor_ops(reused, ancestor);
        }
        assert!(actor_ops(reused).is_none());
    }
}
