//! Object allocation/replacement and construction policy over live native objects.
//!
//! Native continuations retain the two FPushMemTag scopes. Rust owns mirrored
//! field access, constructor and scalar snapshots, publication and restoration
//! ordering, and invokes the live checked constructor after allocation.

use crate::object_bootstrap::{self, Hp2ObjectHeaderKind};
use crate::object_model;
use hp2_abi::engine::generated::{UBoolProperty, UClass, UObject, UProperty, UStruct};
use std::ffi::c_void;
use std::ptr::null_mut;

/// Checked native instance constructor; zero succeeds, nonzero preserves the
/// native exception status without unwinding across the ABI boundary.
pub type Hp2ClassConstructor = unsafe extern "C" fn(*mut c_void) -> i32;

pub const CLASS_ABSTRACT: u32 = 0x00001;
pub const CLASS_TRANSIENT: u32 = 0x00008;
pub const CLASS_PER_OBJECT_CONFIG: u32 = 0x00400;
pub const RF_TRANSIENT: u32 = 0x00004000;
pub const RF_KEEP: u32 = 0x0c000000;

#[repr(C)]
pub struct Hp2ObjectAllocationRequest {
    pub in_class: *mut c_void,
    pub outer: *mut c_void,
    pub in_template: *mut c_void,
    pub supplied: *mut c_void,
    pub flags: u32,
}

/// The context owns the live FName (also used by the native guard) and memory-tag
/// string. Native leaves retain lifecycle operations and diagnostics; plain
/// object, class and property fields are accessed through their ABI mirrors.
/// Every fallible leaf captures native exceptions and returns their status.
#[repr(C)]
pub struct Hp2ObjectAllocationServices {
    pub assert_error: unsafe extern "C" fn(*mut c_void) -> i32,
    pub assert_within: unsafe extern "C" fn(*mut c_void) -> i32,
    pub assert_constructor: unsafe extern "C" fn(*mut c_void) -> i32,
    pub register_count: unsafe extern "C" fn(*mut c_void) -> i32,
    pub package_class: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    /// 0 empty, 1 unregistered, 2 abstract, 3 unpackaged, 4 not-within.
    pub validation_error: unsafe extern "C" fn(*mut c_void, i32, *mut c_void, *mut c_void) -> i32,
    pub name: unsafe extern "C" fn(*mut c_void) -> i32,
    pub unique_name: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
    pub check_conflicts: unsafe extern "C" fn(*mut c_void) -> i32,
    pub hash_head: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut *mut c_void) -> i32,
    pub conflict: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
    pub select_tag: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) -> i32,
    pub with_tag: unsafe extern "C" fn(
        *mut c_void,
        *const Hp2ObjectAllocationRequest,
        *const Hp2ObjectAllocationServices,
        *mut *mut c_void,
    ) -> i32,
    pub find: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut *mut c_void) -> i32,
    /// 0 UClass, 1 UProperty, 2 UBoolProperty; called even for a null object.
    /// Native casts retain lazy StaticClass resolution and its exception boundary.
    pub cast: unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_void) -> i32,
    pub assert_replacement: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub replacing: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub no_replace: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
    pub allocate: unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_void) -> i32,
    pub destroy: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub reserve: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub init_properties:
        unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void) -> i32,
    pub add: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) -> i32,
    pub assert_valid: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub load_config: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub load_localized: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
}

fn status(value: i32) -> Result<(), i32> {
    if value == 0 { Ok(()) } else { Err(value) }
}

/// Operations used by the generic allocation/replacement policy.

/// Direct implementations operate on canonical Rust-owned records and return
/// their runtime error unchanged. The legacy adapter below alone translates
/// callback statuses to `i32`.
pub trait ObjectAllocationOps {
    /// Error returned by the owning runtime.
    type Error;
    /// Copyable replacement snapshot, including any class-operation identity.
    type ReplacementSnapshot: Copy;

    fn assert_error(&self) -> Result<(), Self::Error>;
    fn assert_within(&self, class: *mut c_void) -> Result<(), Self::Error>;
    fn assert_constructor(&self, class: *mut c_void) -> Result<(), Self::Error>;
    fn register_count(&self) -> i32;
    fn package_class(&self) -> Result<*mut c_void, Self::Error>;
    fn validation_error(
        &self,
        code: i32,
        class: *mut c_void,
        outer: *mut c_void,
    ) -> Result<(), Self::Error>;
    fn name(&self) -> i32;
    fn unique_name(&self, class: *mut c_void, outer: *mut c_void)
    -> Result<(), Self::Error>;
    fn check_conflicts(&self) -> bool;
    fn hash_head(&self, outer: *mut c_void) -> Result<*mut c_void, Self::Error>;
    fn conflict(&self, object: *mut c_void, class: *mut c_void) -> Result<(), Self::Error>;
    fn select_tag(&self, class: *mut c_void, registered: bool) -> Result<(), Self::Error>;
    fn find(&self, class: *mut c_void, outer: *mut c_void)
    -> Result<*mut c_void, Self::Error>;
    fn cast(&self, object: *mut c_void, kind: i32) -> Result<*mut c_void, Self::Error>;
    fn assert_replacement(
        &self,
        supplied: *mut c_void,
        object: *mut c_void,
    ) -> Result<(), Self::Error>;
    fn replacing(&self, object: *mut c_void) -> Result<(), Self::Error>;
    fn no_replace(&self, object: *mut c_void, class: *mut c_void)
    -> Result<(), Self::Error>;
    fn snapshot_replacement(
        &self,
        class: *mut c_void,
        property: *mut c_void,
        bool_property: *mut c_void,
    ) -> Result<Self::ReplacementSnapshot, Self::Error>;
    fn restore_replacement(&self, snapshot: Self::ReplacementSnapshot);
    fn allocate_record(&self, properties_size: i32) -> Result<*mut c_void, Self::Error>;
    fn destroy(&self, object: *mut c_void) -> Result<(), Self::Error>;
    fn reserve(&self, index: i32) -> Result<(), Self::Error>;
    fn init_properties(
        &self,
        object: *mut c_void,
        class: *mut c_void,
        template: *mut c_void,
    ) -> Result<(), Self::Error>;
    fn add(&self, object: *mut c_void, index: i32) -> Result<(), Self::Error>;
    fn assert_valid(&self, object: *mut c_void) -> Result<(), Self::Error>;
    fn load_config(&self, object: *mut c_void) -> Result<(), Self::Error>;
    fn load_localized(&self, object: *mut c_void) -> Result<(), Self::Error>;
}

/// Validate, name, diagnose conflicts, and select the allocation tag once.
///
/// `Ok(false)` means the native validation diagnostic handled the request and
/// allocation must return a null object without entering a tag scope.
unsafe fn allocation_preflight<O: ObjectAllocationOps>(
    ops: &O,
    request: &Hp2ObjectAllocationRequest,
) -> Result<bool, O::Error> {
    ops.assert_error()?;
    ops.assert_within(request.in_class)?;
    ops.assert_constructor(request.in_class)?;
    let error = if request.in_class.is_null() {
        Some(0)
    } else if unsafe { (*request.in_class.cast::<UObject>()).index } == -1
        && ops.register_count() == 0
    {
        Some(1)
    } else if unsafe { (*request.in_class.cast::<UClass>()).class_flags } & CLASS_ABSTRACT != 0 {
        Some(2)
    } else {
        let unpackaged = if request.outer.is_null() {
            ops.package_class()? != request.in_class
        } else {
            false
        };
        if unpackaged {
            Some(3)
        } else if !request.outer.is_null()
            && !unsafe {
                object_model::is_a(
                    request.outer,
                    (*request.in_class.cast::<UClass>()).class_within,
                )
            }
        {
            Some(4)
        } else {
            None
        }
    };
    if let Some(error) = error {
        ops.validation_error(error, request.in_class, request.outer)?;
        return Ok(false);
    }
    if ops.name() == 0 {
        ops.unique_name(request.in_class, request.outer)?;
    }
    if ops.check_conflicts() && ops.name() != 0 {
        let mut object = ops.hash_head(request.outer)?;
        while !object.is_null() {
            if i32::from_ne_bytes(unsafe { (*object.cast::<UObject>()).name }) == ops.name()
                && unsafe { (*object.cast::<UObject>()).outer } == request.outer
                && unsafe { (*object.cast::<UObject>()).class } != request.in_class
            {
                ops.conflict(object, request.in_class)?;
            }
            // The conflict logger may replace this object's hash-next link.
            object = unsafe { (*object.cast::<UObject>()).hash_next };
        }
    }
    ops.select_tag(request.in_class, ops.register_count() > 0)?;
    Ok(true)
}

/// Execute the shared allocation policy within a direct owner's tag scope.
///
/// # Errors
/// Returns the first owning-runtime error without translating its diagnostic.
///
/// # Safety
/// All request pointers remain valid through the tag scope and synchronous
/// callbacks. `with_tag` invokes its supplied continuation exactly once.
pub unsafe fn allocate_with_ops<O, F>(
    ops: &O,
    request: &Hp2ObjectAllocationRequest,
    with_tag: F,
) -> Result<*mut c_void, O::Error>
where
    O: ObjectAllocationOps,
    F: FnOnce(
        &O,
        &Hp2ObjectAllocationRequest,
        unsafe fn(&O, &Hp2ObjectAllocationRequest) -> Result<*mut c_void, O::Error>,
    ) -> Result<*mut c_void, O::Error>,
{
    if !unsafe { allocation_preflight(ops, request)? } {
        return Ok(null_mut());
    }
    with_tag(ops, request, allocate_tagged_with_ops::<O>)
}

/// Continue the shared allocation policy after the memory tag is active.
///
/// # Errors
/// Returns the first owning-runtime failure; already-published mutations remain.
///
/// # Safety
/// Request pointers remain valid and all owner hooks obey the reentry contract.
pub unsafe fn allocate_tagged_with_ops<O: ObjectAllocationOps>(
    ops: &O,
    request: &Hp2ObjectAllocationRequest,
) -> Result<*mut c_void, O::Error> {
    let mut object = ops.find(request.in_class, request.outer)?;
    let class = ops.cast(object, 0)?;
    let property = ops.cast(object, 1)?;
    let bool_property = ops.cast(object, 2)?;
    let mut index = -1;
    let mut flags = request.flags;
    let snapshot = if object.is_null() {
        if request.supplied.is_null() {
            object = ops.allocate_record(unsafe {
                (*request.in_class.cast::<UClass>()).properties_size
            })?;
        } else {
            object = request.supplied;
        }
        None
    } else {
        ops.assert_replacement(request.supplied, object)?;
        ops.replacing(object)?;
        if unsafe { (*object.cast::<UObject>()).class } != request.in_class {
            ops.no_replace(object, request.in_class)?;
        }
        flags |= unsafe { (*object.cast::<UObject>()).object_flags } & RF_KEEP;
        index = unsafe { (*object.cast::<UObject>()).index };
        let snapshot = ops.snapshot_replacement(class, property, bool_property)?;
        ops.destroy(object)?;
        ops.reserve(index)?;
        Some(snapshot)
    };
    if unsafe { (*request.in_class.cast::<UClass>()).class_flags } & CLASS_TRANSIENT != 0 {
        flags |= RF_TRANSIENT;
    }
    unsafe {
        object_bootstrap::initialize_header(
            object,
            request.in_class,
            request.outer,
            ops.name(),
            flags,
            Hp2ObjectHeaderKind::Allocated,
        );
    }
    ops.init_properties(object, request.in_class, request.in_template)?;
    ops.add(object, index)?;
    ops.assert_valid(object)?;
    if unsafe { (*request.in_class.cast::<UClass>()).class_flags } & CLASS_PER_OBJECT_CONFIG != 0 {
        ops.load_config(object)?;
        ops.load_localized(object)?;
    }
    if let Some(snapshot) = snapshot {
        ops.restore_replacement(snapshot);
    }
    Ok(object)
}

#[derive(Clone, Copy)]
struct LegacyReplacementSnapshot {
    class: *mut UClass,
    within: *mut c_void,
    class_flags: u32,
    constructor: *mut c_void,
    properties_size: i32,
    properties_align: i32,
    property: *mut UProperty,
    offset: i32,
    cpp_property: i32,
    bool_property: *mut UBoolProperty,
    bit_mask: u32,
}

struct LegacyAllocationOps<'a> {
    context: *mut c_void,
    services: &'a Hp2ObjectAllocationServices,
}

impl ObjectAllocationOps for LegacyAllocationOps<'_> {
    type Error = i32;
    type ReplacementSnapshot = LegacyReplacementSnapshot;

    fn assert_error(&self) -> Result<(), i32> {
        status(unsafe { (self.services.assert_error)(self.context) })
    }
    fn assert_within(&self, class: *mut c_void) -> Result<(), i32> {
        status(unsafe { (self.services.assert_within)(class) })
    }
    fn assert_constructor(&self, class: *mut c_void) -> Result<(), i32> {
        status(unsafe { (self.services.assert_constructor)(class) })
    }
    fn register_count(&self) -> i32 {
        unsafe { (self.services.register_count)(self.context) }
    }
    fn package_class(&self) -> Result<*mut c_void, i32> {
        let mut package = null_mut();
        status(unsafe { (self.services.package_class)(self.context, &mut package) })?;
        Ok(package)
    }
    fn validation_error(
        &self,
        code: i32,
        class: *mut c_void,
        outer: *mut c_void,
    ) -> Result<(), i32> {
        status(unsafe {
            (self.services.validation_error)(self.context, code, class, outer)
        })
    }
    fn name(&self) -> i32 {
        unsafe { (self.services.name)(self.context) }
    }
    fn unique_name(&self, class: *mut c_void, outer: *mut c_void) -> Result<(), i32> {
        status(unsafe { (self.services.unique_name)(self.context, class, outer) })
    }
    fn check_conflicts(&self) -> bool {
        unsafe { (self.services.check_conflicts)(self.context) != 0 }
    }
    fn hash_head(&self, outer: *mut c_void) -> Result<*mut c_void, i32> {
        let mut object = null_mut();
        status(unsafe { (self.services.hash_head)(self.context, outer, &mut object) })?;
        Ok(object)
    }
    fn conflict(&self, object: *mut c_void, class: *mut c_void) -> Result<(), i32> {
        status(unsafe { (self.services.conflict)(self.context, object, class) })
    }
    fn select_tag(&self, class: *mut c_void, registered: bool) -> Result<(), i32> {
        status(unsafe { (self.services.select_tag)(self.context, class, i32::from(registered)) })
    }
    fn find(&self, class: *mut c_void, outer: *mut c_void) -> Result<*mut c_void, i32> {
        let mut object = null_mut();
        status(unsafe { (self.services.find)(self.context, class, outer, &mut object) })?;
        Ok(object)
    }
    fn cast(&self, object: *mut c_void, kind: i32) -> Result<*mut c_void, i32> {
        let mut result = null_mut();
        status(unsafe { (self.services.cast)(object, kind, &mut result) })?;
        Ok(result)
    }
    fn assert_replacement(&self, supplied: *mut c_void, object: *mut c_void) -> Result<(), i32> {
        status(unsafe { (self.services.assert_replacement)(supplied, object) })
    }
    fn replacing(&self, object: *mut c_void) -> Result<(), i32> {
        status(unsafe { (self.services.replacing)(self.context, object) })
    }
    fn no_replace(&self, object: *mut c_void, class: *mut c_void) -> Result<(), i32> {
        status(unsafe { (self.services.no_replace)(self.context, object, class) })
    }
    fn snapshot_replacement(
        &self,
        class: *mut c_void,
        property: *mut c_void,
        bool_property: *mut c_void,
    ) -> Result<Self::ReplacementSnapshot, i32> {
        let class = class.cast::<UClass>();
        let property = property.cast::<UProperty>();
        let bool_property = bool_property.cast::<UBoolProperty>();
        Ok(unsafe {
            LegacyReplacementSnapshot {
                class,
                within: if class.is_null() { null_mut() } else { (*class).class_within },
                class_flags: if class.is_null() { 0 } else { (*class).class_flags & CLASS_ABSTRACT },
                constructor: if class.is_null() { null_mut() } else { (*class).class_constructor },
                properties_size: if class.is_null() { 0 } else { (*class).properties_size },
                properties_align: if class.is_null() { 0 } else { (*class.cast::<UStruct>()).properties_align },
                property,
                offset: if property.is_null() { 0 } else { (*property).offset },
                cpp_property: if property.is_null() { 0 } else { (*property).cpp_property },
                bool_property,
                bit_mask: if bool_property.is_null() { 0 } else { (*bool_property).bit_mask },
            }
        })
    }
    fn restore_replacement(&self, snapshot: Self::ReplacementSnapshot) {
        unsafe {
            if !snapshot.class.is_null() {
                (*snapshot.class).class_within = snapshot.within;
                (*snapshot.class).class_flags |= snapshot.class_flags;
                (*snapshot.class).class_constructor = snapshot.constructor;
                (*snapshot.class).properties_size = snapshot.properties_size;
                (*snapshot.class.cast::<UStruct>()).properties_align = snapshot.properties_align;
            }
            if !snapshot.property.is_null() {
                (*snapshot.property).offset = snapshot.offset;
                (*snapshot.property).cpp_property = snapshot.cpp_property;
                if !snapshot.bool_property.is_null() {
                    (*snapshot.bool_property).bit_mask = snapshot.bit_mask;
                }
            }
        }
    }
    fn allocate_record(&self, size: i32) -> Result<*mut c_void, i32> {
        let mut object = null_mut();
        status(unsafe { (self.services.allocate)(self.context, size, &mut object) })?;
        Ok(object)
    }
    fn destroy(&self, object: *mut c_void) -> Result<(), i32> {
        status(unsafe { (self.services.destroy)(self.context, object) })
    }
    fn reserve(&self, index: i32) -> Result<(), i32> {
        status(unsafe { (self.services.reserve)(self.context, index) })
    }
    fn init_properties(
        &self,
        object: *mut c_void,
        class: *mut c_void,
        template: *mut c_void,
    ) -> Result<(), i32> {
        status(unsafe { (self.services.init_properties)(self.context, object, class, template) })
    }
    fn add(&self, object: *mut c_void, index: i32) -> Result<(), i32> {
        status(unsafe { (self.services.add)(self.context, object, index) })
    }
    fn assert_valid(&self, object: *mut c_void) -> Result<(), i32> {
        status(unsafe { (self.services.assert_valid)(self.context, object) })
    }
    fn load_config(&self, object: *mut c_void) -> Result<(), i32> {
        status(unsafe { (self.services.load_config)(self.context, object) })
    }
    fn load_localized(&self, object: *mut c_void) -> Result<(), i32> {
        status(unsafe { (self.services.load_localized)(self.context, object) })
    }
}

/// Apply allocation validation and enter the legacy native tag continuation.
///
/// # Safety
/// The request and service callbacks remain valid through reentry; native
/// layouts match the generated ABI definitions.
pub unsafe fn allocate(
    context: *mut c_void,
    request: &Hp2ObjectAllocationRequest,
    services: &Hp2ObjectAllocationServices,
) -> Result<*mut c_void, i32> {
    let ops = LegacyAllocationOps { context, services };
    // SAFETY: The legacy ABI guarantees valid request/service pointers; the
    // shared preflight copies snapshots before calling reentrant services.
    unsafe {
        if !allocation_preflight(&ops, request)? {
            return Ok(null_mut());
        }
        let mut result = null_mut();
        status((services.with_tag)(context, request, services, &mut result))?;
        Ok(result)
    }
}

/// Continue the shared allocation policy within an active native memory tag.
///
/// # Safety
/// The same pointer/layout requirements as [`allocate`] apply; the native tag
/// is already active and its staged continuation invokes this once.
pub unsafe fn allocate_tagged(
    context: *mut c_void,
    request: &Hp2ObjectAllocationRequest,
    services: &Hp2ObjectAllocationServices,
) -> Result<*mut c_void, i32> {
    // SAFETY: The legacy adapter preserves native lifecycle callbacks; this
    // continuation now executes the same generic policy as direct Core owners.
    unsafe { allocate_tagged_with_ops(&LegacyAllocationOps { context, services }, request) }
}

#[repr(C)]
pub struct Hp2ObjectConstructionServices {
    pub assert_error: unsafe extern "C" fn(*mut c_void) -> i32,
    pub with_tag: unsafe extern "C" fn(
        *mut c_void,
        *const Hp2ObjectAllocationRequest,
        *const Hp2ObjectConstructionServices,
        *mut *mut c_void,
    ) -> i32,
    pub allocate: unsafe extern "C" fn(
        *mut c_void,
        *const Hp2ObjectAllocationRequest,
        *mut *mut c_void,
    ) -> i32,
}
/// Operations needed by the shared post-allocation constructor policy.
///
/// The constructor value is copied after allocation, so callbacks may reenter
/// without retaining a class-record borrow. Direct Rust owners use their
/// immutable class-operation identity here; only the legacy adapter stores a
/// native constructor pointer.
pub trait ObjectConstructionOps {
    /// Error returned by the selected runtime owner.
    type Error;
    /// Stable, copyable identity for the required constructor operation.
    type Constructor: Copy;

    /// Check the active runtime error state.
    fn assert_error(&self) -> Result<(), Self::Error>;
    /// Allocate and publish an instance using the shared allocation policy.
    ///
    /// # Safety
    /// The request's pointers must remain valid for the allocation operation.
    unsafe fn allocate(
        &self,
        request: &Hp2ObjectAllocationRequest,
    ) -> Result<*mut c_void, Self::Error>;
    /// Snapshot the current constructor identity from the live class metadata.
    fn constructor_snapshot(&self, class: *mut c_void) -> Result<Self::Constructor, Self::Error>;
    /// Invoke the previously snapshotted constructor identity.
    ///
    /// # Safety
    /// `object` must be the live allocated instance returned by `allocate`.
    unsafe fn invoke_constructor(
        &self,
        constructor: Self::Constructor,
        object: *mut c_void,
    ) -> Result<(), Self::Error>;
}

/// Execute allocation followed by the live class-operation constructor.
///
/// # Errors
/// Returns the first allocation or constructor error. Allocation publication is
/// retained if constructor invocation fails.
///
/// # Safety
/// The request and all pointers reachable from it must remain valid through
/// allocation, constructor invocation, and any synchronous reentry.
pub unsafe fn construct_tagged_with_ops<O: ObjectConstructionOps>(
    ops: &O,
    request: &Hp2ObjectAllocationRequest,
) -> Result<*mut c_void, O::Error> {
    ops.assert_error()?;
    unsafe { construct_after_check(ops, request) }
}

unsafe fn construct_after_check<O: ObjectConstructionOps>(
    ops: &O,
    request: &Hp2ObjectAllocationRequest,
) -> Result<*mut c_void, O::Error> {
    let object = unsafe { ops.allocate(request)? };
    if !object.is_null() {
        // Lookup is deliberately after allocation; replacement/restoration may
        // have changed the live class metadata during that operation.
        let constructor = ops.constructor_snapshot(request.in_class)?;
        unsafe { ops.invoke_constructor(constructor, object)? };
    }
    Ok(object)
}

/// Run the shared constructor policy inside a direct owner's memory-tag scope.
///
/// The scope receives the checked continuation; it must invoke that operation
/// exactly once. Legacy callers retain their staged native continuation.
///
/// # Errors
/// Returns the first validation, tag-scope, allocation, or constructor error.
///
/// # Safety
/// The request and all pointers reachable from it must remain valid through the
/// tag scope, constructor invocation, and synchronous reentry.
pub unsafe fn construct_with_tag_ops<O, F>(
    ops: &O,
    request: &Hp2ObjectAllocationRequest,
    with_tag: F,
) -> Result<*mut c_void, O::Error>
where
    O: ObjectConstructionOps,
    F: FnOnce(
        &O,
        &Hp2ObjectAllocationRequest,
        unsafe fn(&O, &Hp2ObjectAllocationRequest) -> Result<*mut c_void, O::Error>,
    ) -> Result<*mut c_void, O::Error>,
{
    ops.assert_error()?;
    with_tag(ops, request, construct_after_check::<O>)
}

struct LegacyConstructionOps<'a> {
    context: *mut c_void,
    services: &'a Hp2ObjectConstructionServices,
}

impl ObjectConstructionOps for LegacyConstructionOps<'_> {
    type Error = i32;
    type Constructor = Hp2ClassConstructor;

    fn assert_error(&self) -> Result<(), Self::Error> {
        status(unsafe { (self.services.assert_error)(self.context) })
    }

    unsafe fn allocate(
        &self,
        request: &Hp2ObjectAllocationRequest,
    ) -> Result<*mut c_void, Self::Error> {
        let mut result = null_mut();
        status(unsafe { (self.services.allocate)(self.context, request, &mut result) })?;
        Ok(result)
    }

    fn constructor_snapshot(
        &self,
        class: *mut c_void,
    ) -> Result<Self::Constructor, Self::Error> {
        let address = unsafe { (*class.cast::<UClass>()).class_constructor };
        // SAFETY: The allocation policy's validation checked this live native
        // constructor; this snapshot is intentionally taken after allocation.
        Ok(unsafe { std::mem::transmute(address) })
    }

    unsafe fn invoke_constructor(
        &self,
        constructor: Self::Constructor,
        object: *mut c_void,
    ) -> Result<(), Self::Error> {
        status(unsafe { constructor(object) })
    }
}


/// # Safety
/// Context and immutable request/services must survive callbacks. `with_tag`
/// enters `construct_tagged` with InClass->GetName()'s native memory tag alive.
/// As in the native entry, a null class is not a valid construction request.
/// A successfully allocated nonnull object must have a live checked constructor
/// whose code remains valid through invocation and reentry. The constructor
/// captures exceptions as statuses; no unwind may cross this ABI.
pub unsafe fn construct(
    context: *mut c_void,
    request: &Hp2ObjectAllocationRequest,
    services: &Hp2ObjectConstructionServices,
) -> Result<*mut c_void, i32> {
    // SAFETY: The caller guarantees the request, context and callbacks remain
    // valid through the native memory-tag continuation and any reentry.
    unsafe {
        status((services.assert_error)(context))?;
        let mut result = null_mut();
        status((services.with_tag)(context, request, services, &mut result))?;
        Ok(result)
    }
}

/// # Safety
/// Same requirements as `construct`, within its native memory-tag scope.
/// Constructor lookup is live after allocation (which can restore or mutate
/// the class); a nonzero constructor status leaves publication intact.
pub unsafe fn construct_tagged(
    context: *mut c_void,
    request: &Hp2ObjectAllocationRequest,
    services: &Hp2ObjectConstructionServices,
) -> Result<*mut c_void, i32> {
    // SAFETY: The legacy adapter preserves the existing allocation callback
    // and native checked-constructor boundary. Constructor selection remains
    // a post-allocation lookup shared with direct Rust owners.
    unsafe {
        construct_after_check(&LegacyConstructionOps { context, services }, request)
    }
}
