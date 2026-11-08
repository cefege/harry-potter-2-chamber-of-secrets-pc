//! Canonical Rust dispatch for compiler-probed `TLazyArray` storage.
//!
//! The ABI descriptor supplies actual `TArray` and `FLazyLoader` base-subobject
//! slots. No C++ constructor, virtual call, or synthetic archive pointer is used.

use crate::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use crate::object_archive::ObjectArchive;
use hp2_abi::engine::generated::{TArrayRaw, UObject};
use std::ffi::c_void;
use std::ptr;

/// Raw compiler-probed slots for one live lazy-array record.
///
/// `array` points at the actual `TArray` base subobject; `loader` points at the
/// actual `FLazyLoader` base subobject and is only the canonical linker-list
/// identity. Saved fields point into the compiler-probed `FLazyLoader` base.
#[derive(Clone, Copy)]
pub struct LazyArraySlots {
    pub array: *mut TArrayRaw,
    pub loader: *mut c_void,
    pub saved_archive: *mut *mut c_void,
    pub saved_position: *mut i32,
}

/// Serializer for the real `TArray` body, invoked synchronously with Core access.
pub type LazyArrayBody =
    for<'a> unsafe fn(CoreAccess<'a>, *mut c_void, &ObjectArchive<'_>) -> RuntimeResult<()>;

/// Element destructor for unload; `None` is only valid for trivially
/// destructible element types.
pub type LazyArrayElementDestroy = unsafe fn(*mut u8) -> RuntimeResult<()>;

fn validate_slots(slots: LazyArraySlots) -> RuntimeResult<()> {
    if slots.array.is_null()
        || slots.loader.is_null()
        || slots.saved_archive.is_null()
        || slots.saved_position.is_null()
    {
        return Err(RuntimeError::new("Invalid lazy-array slot descriptor"));
    }
    Ok(())
}

/// Initializes only the real array header and saved fields; vptr bytes remain opaque.
///
/// # Safety
/// Every slot points into one writable, live compiler-probed lazy-array record,
/// and its array has no owned allocation or registered lazy source yet. Call
/// this from the owning type's real Rust constructor path, never on a live value.
pub unsafe fn lazy_array_initialize(
    _access: CoreAccess<'_>,
    slots: LazyArraySlots,
) -> RuntimeResult<()> {
    validate_slots(slots)?;
    unsafe {
        ptr::write_unaligned(
            slots.array,
            TArrayRaw {
                data: ptr::null_mut(),
                num: 0,
                max: 0,
            },
        );
        ptr::write_unaligned(slots.saved_archive, ptr::null_mut());
        ptr::write_unaligned(slots.saved_position, 0);
    }
    Ok(())
}

/// Applies Unreal's shared TLazyArray archive policy to one canonical record.
///
/// # Safety
/// `owner` is a live object in this Core owner; `slots` refer to its live lazy
/// array; `archive` is used only for this synchronous call. `body` implements
/// the actual element serialization and must not retain its arguments. A real
/// lazy attachment requires the exact shared bounded export and canonical
/// linker mapping. Other archives keep their native no-op AttachLazyLoader
/// behavior; they are never copied or eagerly materialized as a fallback.
pub unsafe fn lazy_array_serialize(
    access: CoreAccess<'_>,
    owner: *mut UObject,
    slots: LazyArraySlots,
    archive: &ObjectArchive<'_>,
    element_width: usize,
    body: LazyArrayBody,
) -> RuntimeResult<()> {
    validate_slots(slots)?;
    if element_width == 0 {
        return Err(RuntimeError::new("Lazy-array element width is zero"));
    }
    let mut context = PolicyContext {
        access,
        owner,
        slots,
        archive: Some(archive),
        element_width,
        body,
        binding: None,
        source_archive: false,
        element_destroy: None,
        error: None,
        policy_handle: ptr::null_mut(),
    };
    run_policy(
        &mut context,
        crate::array_archive::action::LAZY_SERIALIZE,
        0,
    )
}

/// Explicitly attaches this record to the real linker owning `owner`.
///
/// # Safety
/// The arguments obey `lazy_array_serialize`'s lifetime contract. The source
/// must be the exact shared bounded export range for `owner` and use its linker
/// mapping; borrowed memory archives have no lazy-attach capability.
pub unsafe fn lazy_array_attach(
    access: CoreAccess<'_>,
    owner: *mut UObject,
    slots: LazyArraySlots,
    archive: &ObjectArchive<'_>,
    element_width: usize,
    body: LazyArrayBody,
) -> RuntimeResult<()> {
    validate_slots(slots)?;
    if element_width == 0 {
        return Err(RuntimeError::new("Lazy-array element width is zero"));
    }
    crate::core_loader::attach_lazy_array(access, owner, slots, archive, element_width, body)
}

/// Materializes an attached package-backed array through the original signed SavedPos policy.
///
/// # Safety
/// `slots` identify a live canonical record attached to a live Core linker.
/// The registered typed body is used synchronously and is never retained from
/// this invocation.
pub unsafe fn lazy_array_load(access: CoreAccess<'_>, slots: LazyArraySlots) -> RuntimeResult<()> {
    validate_slots(slots)?;
    if unsafe { ptr::read_unaligned(slots.saved_position) } <= 0 {
        return Ok(());
    }
    let binding =
        unsafe { crate::core_loader::lazy_array_binding(access, slots)? }.ok_or_else(no_binding)?;
    let mut context = PolicyContext {
        access,
        owner: ptr::null_mut(),
        slots,
        archive: None,
        element_width: binding.element_width,
        body: binding.body,
        binding: Some(binding),
        source_archive: true,
        element_destroy: None,
        error: None,
        policy_handle: ptr::null_mut(),
    };
    run_policy(&mut context, crate::array_archive::action::LAZY_LOAD, 0)
}

/// Unloads a previously materialized array according to process-mode policy.
///
/// # Safety
/// `slots` identify a live canonical record. `element_destroy` must correctly
/// destroy every initialized element; use `None` only for trivial element types.
pub unsafe fn lazy_array_unload(
    access: CoreAccess<'_>,
    slots: LazyArraySlots,
    element_destroy: Option<LazyArrayElementDestroy>,
) -> RuntimeResult<()> {
    validate_slots(slots)?;
    if unsafe { ptr::read_unaligned(slots.saved_position) } >= 0 {
        return Ok(());
    }
    let binding = unsafe { crate::core_loader::lazy_array_binding(access, slots)? };
    let Some(binding) = binding else {
        return Ok(());
    };
    let mut context = PolicyContext {
        access,
        owner: ptr::null_mut(),
        slots,
        archive: None,
        element_width: binding.element_width,
        body: binding.body,
        binding: Some(binding),
        source_archive: true,
        element_destroy,
        error: None,
        policy_handle: ptr::null_mut(),
    };
    run_policy(&mut context, crate::array_archive::action::LAZY_UNLOAD, 1)
}

/// Detaches a record from its canonical linker's real lazy-loader list.
///
/// # Safety
/// `slots` point into a live canonical lazy-array record; no saved-slot borrow
/// may survive the call.
pub unsafe fn lazy_array_detach(
    access: CoreAccess<'_>,
    slots: LazyArraySlots,
) -> RuntimeResult<()> {
    validate_slots(slots)?;
    let mut context = PolicyContext {
        access,
        owner: ptr::null_mut(),
        slots,
        archive: None,
        element_width: 0,
        body: no_body,
        binding: None,
        source_archive: false,
        element_destroy: None,
        error: None,
        policy_handle: ptr::null_mut(),
    };
    run_policy(&mut context, crate::array_archive::action::LAZY_DESTROY, 0)
}

/// Runs the native `FLazyLoader::LazyDestroy` operation; array storage release
/// remains the owning type's typed Core allocator responsibility.
///
/// # Safety
/// Same requirements as [`lazy_array_detach`].
pub unsafe fn lazy_array_destroy(
    access: CoreAccess<'_>,
    slots: LazyArraySlots,
) -> RuntimeResult<()> {
    unsafe { lazy_array_detach(access, slots) }
}

unsafe fn no_body(
    _access: CoreAccess<'_>,
    _array: *mut c_void,
    _archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    Err(RuntimeError::new(
        "Detached lazy array has no serialization body",
    ))
}

struct PolicyContext<'access, 'archive, 'data> {
    access: CoreAccess<'access>,
    owner: *mut UObject,
    slots: LazyArraySlots,
    archive: Option<&'archive ObjectArchive<'data>>,
    element_width: usize,
    body: LazyArrayBody,
    binding: Option<crate::core_loader::LazyArrayBinding>,
    source_archive: bool,
    element_destroy: Option<LazyArrayElementDestroy>,
    error: Option<RuntimeError>,
    policy_handle: *mut c_void,
}

struct CallbackShell {
    data: *mut c_void,
    dispatch: unsafe fn(*mut c_void, *mut c_void, i32, i32, *mut c_void) -> i32,
}

unsafe extern "C" fn policy_callback(
    object: *mut c_void,
    archive: *mut c_void,
    operation: i32,
    argument: i32,
    value: *mut c_void,
) -> i32 {
    if object.is_null() {
        return 1;
    }
    let shell = unsafe { &mut *object.cast::<CallbackShell>() };
    unsafe { (shell.dispatch)(shell.data, archive, operation, argument, value) }
}

fn run_policy(
    context: &mut PolicyContext<'_, '_, '_>,
    action: i32,
    argument: i32,
) -> RuntimeResult<()> {
    let mut shell = CallbackShell {
        data: (context as *mut PolicyContext<'_, '_, '_>).cast(),
        dispatch: dispatch_policy,
    };
    let pointer = (&mut shell as *mut CallbackShell).cast::<c_void>();
    context.policy_handle = pointer;
    let mut width = context.element_width;
    let value = if action == crate::array_archive::action::LAZY_SERIALIZE {
        (&mut width as *mut usize).cast()
    } else {
        ptr::null_mut()
    };
    // This opaque token is private to the synchronous shared-policy call; it is
    // never written into FLazyLoader::SavedAr or used as an FArchive pointer.
    let result = unsafe {
        crate::array_archive::run(pointer, pointer, policy_callback, action, argument, value)
    };
    match result {
        Ok(()) => Ok(()),
        Err(_) => Err(context
            .error
            .take()
            .unwrap_or_else(|| RuntimeError::new("Lazy-array policy failed"))),
    }
}

unsafe fn dispatch_policy<'a>(
    data: *mut c_void,
    archive_handle: *mut c_void,
    operation: i32,
    argument: i32,
    value: *mut c_void,
) -> i32 {
    if data.is_null() {
        return 1;
    }
    // SAFETY: run_policy installs this pointer from a live stack context and the
    // shared policy invokes the trampoline synchronously without retaining it.
    let context = unsafe { &mut *data.cast::<PolicyContext<'a, 'a, 'a>>() };
    let saved_archive = unsafe { ptr::read_unaligned(context.slots.saved_archive) };
    if archive_handle != context.policy_handle && archive_handle != saved_archive {
        if context.error.is_none() {
            context.error = Some(RuntimeError::new(
                "Lazy policy used an unrelated archive handle",
            ));
        }
        return 1;
    }
    match dispatch(context, operation, argument, value) {
        Ok(()) => 0,
        Err(error) => {
            if context.error.is_none() {
                context.error = Some(error);
            }
            1
        }
    }
}

fn dispatch(
    context: &mut PolicyContext<'_, '_, '_>,
    operation: i32,
    argument: i32,
    value: *mut c_void,
) -> RuntimeResult<()> {
    use crate::array_archive as aa;
    match operation {
        aa::LOADING => write_i32(
            value,
            i32::from(context.archive.is_some_and(|a| a.is_loading()) || context.source_archive),
        ),
        aa::SAVING => write_i32(
            value,
            i32::from(context.archive.is_some_and(|a| a.is_saving())),
        ),
        aa::TRANS => write_i32(
            value,
            i32::from(context.archive.is_some_and(|a| a.is_trans())),
        ),
        aa::VERSION => {
            if context.source_archive {
                write_i32(
                    value,
                    i32::from(context.binding.ok_or_else(no_binding)?.version),
                )
            } else {
                write_i32(
                    value,
                    i32::from(context.archive.ok_or_else(no_archive)?.flags().version),
                )
            }
        }
        aa::TELL => {
            let position = if context.source_archive {
                let binding = context.binding.ok_or_else(no_binding)?;
                unsafe {
                    crate::core_loader::lazy_archive_position(
                        context.access,
                        binding.linker,
                        binding.row,
                    )?
                }
            } else {
                let archive = context.archive.ok_or_else(no_archive)?;
                archive.tell_file()?
            };
            write_i32(
                value,
                i32::try_from(position)
                    .map_err(|_| RuntimeError::new("Lazy archive position exceeds INT"))?,
            )
        }
        aa::SEEK => {
            if context.source_archive {
                if argument < 0 {
                    return Err(RuntimeError::new("Negative lazy archive position"));
                }
                let binding = context.binding.ok_or_else(no_binding)?;
                unsafe {
                    crate::core_loader::set_lazy_archive_position(
                        context.access,
                        binding.linker,
                        binding.row,
                        argument as usize,
                    )
                }
            } else {
                let archive = context.archive.ok_or_else(no_archive)?;
                if argument < 0 {
                    return Err(RuntimeError::new("Negative lazy archive position"));
                }
                let position = argument as usize;
                archive.seek_file(position)?;
                if let Some(binding) = context.binding {
                    if !archive.is_trans() && archive.linker_identity() == Some(binding.linker) {
                        unsafe {
                            crate::core_loader::set_lazy_archive_position(
                                context.access,
                                binding.linker,
                                binding.row,
                                position,
                            )?;
                        }
                    }
                }
                Ok(())
            }
        }
        aa::COMPACT => {
            if value.is_null() {
                return Err(RuntimeError::new("Missing compact index slot"));
            }
            let archive = context.archive.ok_or_else(no_archive)?;
            let mut integer = unsafe { ptr::read_unaligned(value.cast::<i32>()) };
            archive.serialize_compact_index(&mut integer)?;
            unsafe {
                ptr::write_unaligned(value.cast::<i32>(), integer);
            }
            Ok(())
        }
        aa::INTEGER => {
            if value.is_null() {
                return Err(RuntimeError::new("Missing integer slot"));
            }
            let archive = context.archive.ok_or_else(no_archive)?;
            let mut integer = unsafe { ptr::read_unaligned(value.cast::<i32>()) };
            archive.serialize_i32(&mut integer)?;
            unsafe {
                ptr::write_unaligned(value.cast::<i32>(), integer);
            }
            Ok(())
        }
        aa::LEGACY_CHECK => {
            if argument < 0 {
                return Err(RuntimeError::new("Negative legacy lazy-array count"));
            }
            Ok(())
        }
        aa::CHECKED_SIZE => {
            if value.is_null() {
                return Err(RuntimeError::new("Missing checked lazy-array size"));
            }
            let mut size = unsafe { ptr::read_unaligned(value.cast::<aa::CheckedSize>()) };
            size.result = i32::try_from(size.size)
                .map_err(|_| RuntimeError::new("Lazy-array size exceeds INT"))?;
            unsafe {
                ptr::write_unaligned(value.cast::<aa::CheckedSize>(), size);
            }
            Ok(())
        }
        aa::SAVED_ARCHIVE => {
            if value.is_null() {
                return Err(RuntimeError::new("Missing saved archive output"));
            }
            let saved = unsafe { ptr::read_unaligned(context.slots.saved_archive) };
            unsafe {
                ptr::write_unaligned(value.cast::<*mut c_void>(), saved);
            }
            Ok(())
        }
        aa::SAVED_POS => {
            let position = unsafe { ptr::read_unaligned(context.slots.saved_position) };
            write_i32(value, position)
        }
        aa::SET_SAVED_POS => {
            unsafe {
                ptr::write_unaligned(context.slots.saved_position, argument);
            }
            Ok(())
        }
        aa::ATTACH => {
            let archive = context.archive.ok_or_else(no_archive)?;
            if archive.is_trans() || archive.linker_identity().is_none() {
                // FArchive's default AttachLazyLoader is empty; do not invent a source.
                return Ok(());
            }
            unsafe {
                crate::core_loader::attach_lazy_array(
                    context.access,
                    context.owner,
                    context.slots,
                    archive,
                    context.element_width,
                    context.body,
                )?;
            }
            context.binding =
                unsafe { crate::core_loader::lazy_array_binding(context.access, context.slots)? };
            Ok(())
        }
        aa::DETACH => unsafe {
            crate::core_loader::detach_lazy_array(context.access, context.slots)
        },
        aa::BASE_SERIALIZE => {
            if context.source_archive {
                let binding = context.binding.ok_or_else(no_binding)?;
                let position = unsafe {
                    crate::core_loader::lazy_archive_position(
                        context.access,
                        binding.linker,
                        binding.row,
                    )?
                };
                unsafe {
                    crate::core_loader::serialize_lazy_array_source(
                        context.access,
                        binding.linker,
                        binding.row,
                        position,
                        context.slots.array.cast(),
                        context.body,
                    )
                }
            } else {
                let archive = context.archive.ok_or_else(no_archive)?;
                unsafe { (context.body)(context.access, context.slots.array.cast(), archive) }
            }
        }
        aa::LAZY_ENABLED => write_i32(value, i32::from(context.access.lazy_load())),
        aa::LOAD => {
            let saved_position = unsafe { ptr::read_unaligned(context.slots.saved_position) };
            if saved_position <= 0 {
                return Ok(());
            }
            let binding =
                unsafe { crate::core_loader::lazy_array_binding(context.access, context.slots)? }
                    .ok_or_else(no_binding)?;
            let mut nested = PolicyContext {
                access: context.access,
                owner: ptr::null_mut(),
                slots: context.slots,
                archive: None,
                element_width: binding.element_width,
                body: binding.body,
                binding: Some(binding),
                source_archive: true,
                element_destroy: context.element_destroy,
                error: None,
                policy_handle: ptr::null_mut(),
            };
            run_policy(&mut nested, aa::action::LAZY_LOAD, 0)
        }
        aa::EMPTY => {
            // SAFETY: the public unsafe entrypoint requires this live array's
            // exact element width and matching destructor, if nontrivial.
            unsafe {
                crate::core_array::destroy_array_with(
                    context.slots.array,
                    context.element_width,
                    context.element_destroy,
                )
            }
        }
        _ => Err(RuntimeError::new("Unsupported lazy-array policy callback")),
    }
}

fn write_i32(value: *mut c_void, integer: i32) -> RuntimeResult<()> {
    if value.is_null() {
        return Err(RuntimeError::new("Missing lazy-array integer output"));
    }
    unsafe {
        ptr::write_unaligned(value.cast::<i32>(), integer);
    }
    Ok(())
}

fn no_archive() -> RuntimeError {
    RuntimeError::new("Lazy-array action has no current archive")
}
fn no_binding() -> RuntimeError {
    RuntimeError::new("Lazy array is not attached to a canonical linker")
}
