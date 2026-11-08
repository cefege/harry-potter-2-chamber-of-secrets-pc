//! Startup native registry and high-native dispatch.
//!
//! The engine serializes registration during bootstrap, before any dispatch.
//! There is deliberately no allocator, lazy runtime initializer, or lock here.
//! Rust stores and invokes checked C handlers. C++ adapters catch exceptions
//! before returning a status, so native handlers never unwind through Rust.

use crate::vm_bytecode;
use hp2_abi::engine::generated::FFrame;
use std::ffi::c_void;

pub const NATIVE_COUNT: usize = 0x1000;
pub const INDEX_NONE: i32 = -1;

/// Checked native invocation: zero succeeds; nonzero preserves the first error.
/// Handlers must catch native exceptions and must never unwind through Rust.
/// Arguments are the object, opaque native frame, and result storage.
pub type Hp2VmNativeHandler = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32;

// Keep the native symbols without emitting duplicate array declarations into
// hp2_rs.h. The native boundary asserts its public callback signature matches.
/// cbindgen:ignore
#[unsafe(export_name = "GNatives")]
static mut NATIVE_TABLE: [Option<Hp2VmNativeHandler>; NATIVE_COUNT] = [None; NATIVE_COUNT];

/// cbindgen:ignore
#[unsafe(export_name = "GNativeDuplicate")]
static mut NATIVE_DUPLICATE: i32 = 0;

static mut INITIALIZED: bool = false;
/// Returns the process-wide duplicate-native index recorded by registration.
///
/// Registration and reads are serialized on the engine application thread.
pub fn duplicate_index() -> i32 {
    unsafe { NATIVE_DUPLICATE }
}

/// Raw access to the existing GNatives backing, never a snapshot or second table.
/// Dereferencing requires initialized registration and serialized native access;
/// no reference to this storage may survive a handler invocation.
pub fn native_table() -> *const Hp2VmNativeHandler {
    (&raw const NATIVE_TABLE).cast()
}

/// Apply registration to independent storage, also used by isolated fixtures.
/// INDEX_NONE still initializes every entry, but never registers a function.
/// A duplicate records the index and replaces the old function. Registering
/// execUndefined itself does not make that slot count as occupied.
///
/// Invalid indices other than INDEX_NONE are outside the legacy registration
/// domain: the old implementation could write out of bounds. Return an error
/// instead; this is not a compatibility promise for that undefined behavior.
///
/// # Safety
/// `table` addresses NATIVE_COUNT initialized, writable nullable handlers.
/// `initialized` and `duplicate` address distinct writable state. Access is
/// exclusive for this call, with no concurrent native table reads or writes.
/// When `initialized` is true every entry must be Some. `undefined` is the
/// execUndefined handler. No references to this storage may be held across
/// native dispatch or later registration.
pub unsafe fn register_entries(
    table: *mut Option<Hp2VmNativeHandler>,
    initialized: *mut bool,
    duplicate: *mut i32,
    index: i32,
    function: Hp2VmNativeHandler,
    undefined: Hp2VmNativeHandler,
) -> Result<(), i32> {
    unsafe {
        if !initialized.read() {
            initialized.write(true);
            for slot in 0..NATIVE_COUNT {
                table.add(slot).write(Some(undefined));
            }
        }
        if index == INDEX_NONE {
            return Ok(());
        }
        if index < 0 || index as usize >= NATIVE_COUNT {
            return Err(1);
        }
        let entry = table.add(index as usize);
        if !std::ptr::fn_addr_eq(entry.read().unwrap_unchecked(), undefined) {
            duplicate.write(index);
        }
        entry.write(Some(function));
        Ok(())
    }
}

/// Register directly into the backing storage of the exported native symbols.
///
/// # Safety
/// Calls must be serialized with each other and every native table consumer.
/// Handlers satisfy register_entries; valid indices are INDEX_NONE or
/// 0..NATIVE_COUNT. This function is safe to call before engine initialization.
pub unsafe fn register(
    index: i32,
    function: Hp2VmNativeHandler,
    undefined: Hp2VmNativeHandler,
) -> Result<(), i32> {
    unsafe {
        register_entries(
            (&raw mut NATIVE_TABLE).cast(),
            &raw mut INITIALIZED,
            &raw mut NATIVE_DUPLICATE,
            index,
            function,
            undefined,
        )
    }
}

/// Consume the second opcode byte before native dispatch observes the frame.
///
/// # Safety
/// `cursor` satisfies vm_bytecode::read for one byte. The high-native prefix
/// must be 0..=15, as supplied by the sixteen compiled HIGH_NATIVE wrappers.
/// No reference into the frame is retained when native invocation resumes.
pub unsafe fn high_index(prefix: u8, cursor: *mut *mut u8) -> u32 {
    let low = unsafe { vm_bytecode::read::<u8>(cursor) };
    u32::from(prefix) * 0x100 + u32::from(low)
}

/// Execute a high-native prefix through the live handler table.
///
/// The incoming object is the original native `this`, not Frame.Object. Decode
/// and advance Code before selecting the handler, and return its exact status.
///
/// # Safety
/// PREFIX is 0..=15. The live frame Code slot has one readable operand byte.
/// Object, frame and result satisfy the selected handler's contract. The table
/// has NATIVE_COUNT initialized non-null handlers, none unwinding through Rust;
/// table access is serialized with registration. No references into the frame
/// or table may survive the invocation, including nested high-native dispatch.
#[inline]
pub unsafe fn execute_high_native<const PREFIX: u8>(
    object: *mut c_void,
    frame: *mut FFrame,
    result: *mut c_void,
    native_table: *const Hp2VmNativeHandler,
) -> i32 {
    unsafe {
        let index = high_index(PREFIX, std::ptr::addr_of_mut!((*frame).code));
        let handler = native_table.add(index as usize).read();
        handler(object, frame.cast(), result)
    }
}
