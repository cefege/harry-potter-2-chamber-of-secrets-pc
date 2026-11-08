//! Opcode-level function-call dispatch (VirtualFunction/FinalFunction/
//! GlobalFunction). Name/operand reads, function resolution, the native
//! RandTrace pre-begin-relevance scope and the virtual `UObject::CallFunction`
//! dispatch all stay inside one C++ leaf per opcode; Rust only selects and
//! invokes the published leaf for the live frame.
use std::ffi::c_void;
type Object = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2VmFunctionCallServices {
    pub call_virtual: unsafe extern "C" fn(Object, Object, Object) -> i32,
    pub call_final: unsafe extern "C" fn(Object, Object, Object) -> i32,
    pub call_global: unsafe extern "C" fn(Object, Object, Object) -> i32,
}

static mut FUNCTION_CALL_SERVICES: Option<Hp2VmFunctionCallServices> = None;

/// # Safety
/// Engine-thread serialized. Callbacks remain live until all calls using this
/// publication finish. Publication invokes no callbacks.
pub unsafe fn publish_services(value: Hp2VmFunctionCallServices) {
    unsafe {
        (&raw mut FUNCTION_CALL_SERVICES).write(Some(value));
    }
}

/// # Safety
/// Serialized with publication.
pub unsafe fn published_services() -> Result<Hp2VmFunctionCallServices, i32> {
    unsafe { (&raw const FUNCTION_CALL_SERVICES).read().ok_or(1) }
}
