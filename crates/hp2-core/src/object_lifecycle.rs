//! Conditional lifecycle and garbage-purge policy over live engine storage.
//!
//! The policy is generic over shared owner operations; C++ service tables remain
//! only as adapters for callers that have not migrated to the direct Rust owner.

use crate::object_collections::ObjectCollection;
use std::ffi::c_void;

// EObjectFlags in Core/Inc/UnObjBas.h; the native seam asserts these values.
pub const RF_UNREACHABLE: u32 = 0x0000_0002;
pub const RF_DESTROYED: u32 = 0x0080_0000;
pub const RF_NEED_POST_LOAD: u32 = 0x0100_0000;
pub const RF_NATIVE: u32 = 0x0400_0000;
pub const RF_DEBUG_POST_LOAD: u32 = 0x2000_0000;
pub const RF_DEBUG_DESTROY: u32 = 0x8000_0000;

#[repr(C)]
pub struct Hp2ObjectLifecycleServices {
    pub index: unsafe extern "C" fn(*mut c_void) -> u32,
    pub flags: unsafe extern "C" fn(*mut c_void) -> u32,
    pub set_flags: unsafe extern "C" fn(*mut c_void, u32) -> i32,
    pub clear_flags: unsafe extern "C" fn(*mut c_void, u32) -> i32,
    pub destroy: unsafe extern "C" fn(*mut c_void) -> i32,
    pub register: unsafe extern "C" fn(*mut c_void) -> i32,
    pub post_load: unsafe extern "C" fn(*mut c_void) -> i32,
    /// `post_load` is zero for a failed Destroy route, one for PostLoad.
    pub route_error: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub table_len: unsafe extern "C" fn(*mut c_void) -> i32,
    pub table_at: unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_void) -> i32,
    pub assert_registrant: unsafe extern "C" fn(i32) -> i32,
}

fn status(value: i32) -> Result<(), i32> {
    if value == 0 { Ok(()) } else { Err(value) }
}

/// Engine-thread operations needed by conditional object lifecycle policy.
///
/// Implementations keep object and class records canonical and perform hooks
/// through short-lived snapshots; this trait never owns a parallel registry.
pub trait LifecycleOps {
    type Error;

    fn index(&self, object: *mut c_void) -> Result<u32, Self::Error>;
    fn flags(&self, object: *mut c_void) -> Result<u32, Self::Error>;
    fn set_flags(&self, object: *mut c_void, flags: u32) -> Result<(), Self::Error>;
    fn clear_flags(&self, object: *mut c_void, flags: u32) -> Result<(), Self::Error>;
    fn destroy(&self, object: *mut c_void) -> Result<(), Self::Error>;
    fn post_load(&self, object: *mut c_void) -> Result<(), Self::Error>;
    fn route_error(&self, object: *mut c_void, post_load: i32) -> Result<(), Self::Error>;
}

/// Applies the engine's exactly-once ConditionalDestroy routing policy.
pub fn conditional_destroy_with<O: LifecycleOps>(
    object: *mut c_void,
    ops: &O,
) -> Result<i32, O::Error> {
    if ops.index(object)? == u32::MAX || ops.flags(object)? & RF_DESTROYED != 0 {
        return Ok(0);
    }
    ops.set_flags(object, RF_DESTROYED)?;
    ops.clear_flags(object, RF_DEBUG_DESTROY)?;
    ops.destroy(object)?;
    if ops.flags(object)? & RF_DEBUG_DESTROY == 0 {
        ops.route_error(object, 0)?;
    }
    Ok(1)
}

/// Whether conditional registration performs the development consistency check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistrationDebug {
    Skip,
    Check,
}

/// Checks the current registration state without retaining a table borrow.
pub trait RegistrationOps {
    type Error;

    fn index(&self, object: *mut c_void) -> Result<u32, Self::Error>;
    fn contains_registrant(&self, object: *mut c_void) -> Result<bool, Self::Error>;
    fn assert_registrant(&self, found: bool) -> Result<(), Self::Error>;
    fn register(&self, object: *mut c_void) -> Result<(), Self::Error>;
}

/// Registers an object only when its slot has not yet been published.
pub fn conditional_register_with<O: RegistrationOps>(
    object: *mut c_void,
    debug: RegistrationDebug,
    ops: &O,
) -> Result<(), O::Error> {
    if ops.index(object)? == u32::MAX {
        if debug == RegistrationDebug::Check {
            ops.assert_registrant(ops.contains_registrant(object)?)?;
        }
        ops.register(object)?;
    }
    Ok(())
}

/// Applies the engine's post-load flag clearing and debug-route policy.
pub fn conditional_post_load_with<O: LifecycleOps>(
    object: *mut c_void,
    ops: &O,
) -> Result<(), O::Error> {
    if ops.flags(object)? & RF_NEED_POST_LOAD != 0 {
        ops.clear_flags(object, RF_NEED_POST_LOAD | RF_DEBUG_POST_LOAD)?;
        ops.post_load(object)?;
        if ops.flags(object)? & RF_DEBUG_POST_LOAD == 0 {
            ops.route_error(object, 1)?;
        }
    }
    Ok(())
}

struct LegacyLifecycle<'a> {
    services: &'a Hp2ObjectLifecycleServices,
}

impl LifecycleOps for LegacyLifecycle<'_> {
    type Error = i32;

    fn index(&self, object: *mut c_void) -> Result<u32, Self::Error> {
        Ok(unsafe { (self.services.index)(object) })
    }

    fn flags(&self, object: *mut c_void) -> Result<u32, Self::Error> {
        Ok(unsafe { (self.services.flags)(object) })
    }

    fn set_flags(&self, object: *mut c_void, flags: u32) -> Result<(), Self::Error> {
        unsafe { status((self.services.set_flags)(object, flags)) }
    }

    fn clear_flags(&self, object: *mut c_void, flags: u32) -> Result<(), Self::Error> {
        unsafe { status((self.services.clear_flags)(object, flags)) }
    }

    fn destroy(&self, object: *mut c_void) -> Result<(), Self::Error> {
        unsafe { status((self.services.destroy)(object)) }
    }

    fn post_load(&self, object: *mut c_void) -> Result<(), Self::Error> {
        unsafe { status((self.services.post_load)(object)) }
    }

    fn route_error(&self, object: *mut c_void, post_load: i32) -> Result<(), Self::Error> {
        unsafe { status((self.services.route_error)(object, post_load)) }
    }
}

struct LegacyRegistration<'a> {
    registrants: *mut ObjectCollection,
    services: &'a Hp2ObjectLifecycleServices,
}

impl RegistrationOps for LegacyRegistration<'_> {
    type Error = i32;

    fn index(&self, object: *mut c_void) -> Result<u32, Self::Error> {
        Ok(unsafe { (self.services.index)(object) })
    }

    fn contains_registrant(&self, object: *mut c_void) -> Result<bool, Self::Error> {
        Ok(unsafe { (*self.registrants).contains(object) })
    }

    fn assert_registrant(&self, found: bool) -> Result<(), Self::Error> {
        unsafe { status((self.services.assert_registrant)(i32::from(found))) }
    }

    fn register(&self, object: *mut c_void) -> Result<(), Self::Error> {
        unsafe { status((self.services.register)(object)) }
    }
}

/// # Safety
/// Object and services must remain valid throughout the hooks, including any
/// reentrant call. Callbacks that can throw must return the captured status.
pub unsafe fn conditional_destroy(
    object: *mut c_void,
    services: &Hp2ObjectLifecycleServices,
) -> Result<i32, i32> {
    conditional_destroy_with(object, &LegacyLifecycle { services })
}

/// # Safety
/// The object, live registrant table and callbacks must remain valid. The table
/// is accessed only when debug checking is enabled and the object is unregistered.
pub unsafe fn conditional_register(
    object: *mut c_void,
    registrants: *mut ObjectCollection,
    debug: i32,
    services: &Hp2ObjectLifecycleServices,
) -> Result<(), i32> {
    conditional_register_with(
        object,
        if debug != 0 {
            RegistrationDebug::Check
        } else {
            RegistrationDebug::Skip
        },
        &LegacyRegistration {
            registrants,
            services,
        },
    )
}

/// # Safety
/// Object and callbacks must remain valid during the hook, including reentry.
pub unsafe fn conditional_post_load(
    object: *mut c_void,
    services: &Hp2ObjectLifecycleServices,
) -> Result<(), i32> {
    conditional_post_load_with(object, &LegacyLifecycle { services })
}


#[repr(C)]
pub struct Hp2ObjectPurgeServices {
    pub object_len: unsafe extern "C" fn(*mut c_void) -> i32,
    pub object_at: unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_void) -> i32,
    pub object_flags: unsafe extern "C" fn(*mut c_void) -> u32,
    pub exit_purge: unsafe extern "C" fn(*mut c_void) -> i32,
    /// Log the current target before reloading its slot for ConditionalDestroy.
    pub log_destroy: unsafe extern "C" fn(*mut c_void, i32, *mut c_void) -> i32,
    pub destroy: unsafe extern "C" fn(*mut c_void, i32, *mut c_void) -> i32,
    pub delete_object: unsafe extern "C" fn(*mut c_void, i32, *mut c_void) -> i32,
    pub name_len: unsafe extern "C" fn(*mut c_void) -> i32,
    pub name_at: unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_void) -> i32,
    pub name_flags: unsafe extern "C" fn(*mut c_void) -> u32,
    /// Logs and deletes the selected name entry.
    pub delete_name: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub log_begin: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub log_counts: unsafe extern "C" fn(*mut c_void, i32, i32) -> i32,
}

/// Shared-reference operations used by the canonical live-table purge policy.
pub trait PurgeOps {
    type Error;

    fn log_begin(&self, no_gc: i32) -> Result<(), Self::Error>;
    fn object_len(&self) -> Result<i32, Self::Error>;
    fn object_at(&self, index: i32) -> Result<*mut c_void, Self::Error>;
    fn object_flags(&self, object: *mut c_void) -> Result<u32, Self::Error>;
    fn exit_purge(&self) -> Result<bool, Self::Error>;
    /// Log the copied current target before the algorithm re-fetches its slot.
    fn log_destroy(&self, index: i32, object: *mut c_void) -> Result<(), Self::Error>;
    /// Perform ConditionalDestroy only; direct owners should use `conditional_destroy_with`.
    fn destroy(&self, index: i32, object: *mut c_void) -> Result<(), Self::Error>;
    /// Free the object in the separate reverse pass after every Destroy hook ran.
    fn delete_object(&self, index: i32, object: *mut c_void) -> Result<(), Self::Error>;
    fn name_len(&self) -> Result<i32, Self::Error>;
    fn name_at(&self, index: i32) -> Result<*mut c_void, Self::Error>;
    fn name_flags(&self, name: *mut c_void) -> Result<u32, Self::Error>;
    fn delete_name(&self, index: i32) -> Result<(), Self::Error>;
    fn log_counts(&self, before: i32, remaining: i32) -> Result<(), Self::Error>;
}

/// Runs destroy hooks before deletion, then frees objects and names in the
/// engine's original traversal order.
pub fn purge_garbage_with<O: PurgeOps>(no_gc: i32, ops: &O) -> Result<(), O::Error> {
    ops.log_begin(no_gc)?;
    if no_gc != 0 {
        return Ok(());
    }

    let mut before: i32 = 0;
    let mut purged: i32 = 0;
    let mut index = ops.object_len()? - 1;
    while index >= 0 {
        let mut object = ops.object_at(index)?;
        if !object.is_null() {
            before = before.wrapping_add(1);
            if ops.object_flags(object)? & RF_UNREACHABLE != 0
                && (ops.object_flags(object)? & RF_NATIVE == 0 || ops.exit_purge()?)
            {
                object = ops.object_at(index)?;
                ops.log_destroy(index, object)?;
                // Logging may replace this slot or grow the table.
                object = ops.object_at(index)?;
                ops.destroy(index, object)?;
                // Already-destroyed objects still contribute to this count.
                purged = purged.wrapping_add(1);
            }
        }
        index -= 1;
    }

    index = ops.object_len()? - 1;
    while index >= 0 {
        let mut object = ops.object_at(index)?;
        if !object.is_null()
            && ops.object_flags(object)? & RF_UNREACHABLE != 0
            && ops.object_flags(object)? & RF_NATIVE == 0
        {
            object = ops.object_at(index)?;
            ops.delete_object(index, object)?;
        }
        index -= 1;
    }

    index = 0;
    while index < ops.name_len()? {
        let name = ops.name_at(index)?;
        if !name.is_null()
            && ops.name_flags(name)? & RF_UNREACHABLE != 0
            && ops.name_flags(name)? & RF_NATIVE == 0
        {
            ops.delete_name(index)?;
        }
        index += 1;
    }
    ops.log_counts(before, before.wrapping_sub(purged))
}

struct LegacyPurge<'a> {
    context: *mut c_void,
    services: &'a Hp2ObjectPurgeServices,
}

impl PurgeOps for LegacyPurge<'_> {
    type Error = i32;

    fn log_begin(&self, no_gc: i32) -> Result<(), Self::Error> {
        unsafe { status((self.services.log_begin)(self.context, no_gc)) }
    }

    fn object_len(&self) -> Result<i32, Self::Error> {
        Ok(unsafe { (self.services.object_len)(self.context) })
    }

    fn object_at(&self, index: i32) -> Result<*mut c_void, Self::Error> {
        let mut object = std::ptr::null_mut();
        unsafe {
            status((self.services.object_at)(self.context, index, &mut object))?;
        }
        Ok(object)
    }

    fn object_flags(&self, object: *mut c_void) -> Result<u32, Self::Error> {
        Ok(unsafe { (self.services.object_flags)(object) })
    }

    fn exit_purge(&self) -> Result<bool, Self::Error> {
        Ok(unsafe { (self.services.exit_purge)(self.context) != 0 })
    }

    fn log_destroy(&self, index: i32, object: *mut c_void) -> Result<(), Self::Error> {
        unsafe { status((self.services.log_destroy)(self.context, index, object)) }
    }

    fn destroy(&self, index: i32, object: *mut c_void) -> Result<(), Self::Error> {
        unsafe { status((self.services.destroy)(self.context, index, object)) }
    }

    fn delete_object(&self, index: i32, object: *mut c_void) -> Result<(), Self::Error> {
        unsafe { status((self.services.delete_object)(self.context, index, object)) }
    }

    fn name_len(&self) -> Result<i32, Self::Error> {
        Ok(unsafe { (self.services.name_len)(self.context) })
    }

    fn name_at(&self, index: i32) -> Result<*mut c_void, Self::Error> {
        let mut name = std::ptr::null_mut();
        unsafe {
            status((self.services.name_at)(self.context, index, &mut name))?;
        }
        Ok(name)
    }

    fn name_flags(&self, name: *mut c_void) -> Result<u32, Self::Error> {
        Ok(unsafe { (self.services.name_flags)(name) })
    }

    fn delete_name(&self, index: i32) -> Result<(), Self::Error> {
        unsafe { status((self.services.delete_name)(self.context, index)) }
    }

    fn log_counts(&self, before: i32, remaining: i32) -> Result<(), Self::Error> {
        unsafe { status((self.services.log_counts)(self.context, before, remaining)) }
    }
}

/// Runs every Destroy before deleting any object. Reverse object scans capture
/// only their initial bound; slots and flags are read live. The name bound is
/// reread on every ascending iteration, matching FName's original traversal.
///
/// # Safety
/// Context, tables, entries and callbacks must satisfy the original engine
/// iteration contract: callbacks may mutate slots/flags but must not invalidate
/// remaining indices. No callback may unwind across this boundary.
pub unsafe fn purge_garbage(
    context: *mut c_void,
    no_gc: i32,
    services: &Hp2ObjectPurgeServices,
) -> Result<(), i32> {
    purge_garbage_with(
        no_gc,
        &LegacyPurge {
            context,
            services,
        },
    )
}
