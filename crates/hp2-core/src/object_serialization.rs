//! UObject::Serialize orchestration over live SDK fields and exception-contained leaves.
use crate::vm_frame;
use std::ffi::c_void;

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Hp2ObjectSerializationLeaf {
    SetFlags,
    Class,
    Object,
    StaticClass,
    Preload,
    IsLoading,
    IsSaving,
    IsTrans,
    Name,
    Outer,
    SerializeClass,
    Linker,
    Flags,
    Frame,
    PublishFrame,
    Node,
    StateNode,
    ProbeMask,
    LatentAction,
    FrameNode,
    Code,
    PublishCode,
    ScriptCount,
    ScriptAt,
    CheckSaveCode,
    CheckNonnegativeOffset,
    CheckedOffset,
    Offset,
    InvalidOffset,
    TaggedProperties,
    BinaryProperties,
    PropertiesSize,
    CountBytes,
}

#[repr(C)]
pub struct Hp2ObjectSerializationServices {
    pub call: unsafe extern "C" fn(
        *mut c_void,
        Hp2ObjectSerializationLeaf,
        usize,
        usize,
        *mut usize,
    ) -> i32,
}

/// Runs UObject serialization through a typed operation closure.
///
/// The closure receives one native-policy leaf at a time. The algorithm never
/// retains an object-field value between calls, so each leaf may safely reenter
/// and mutate canonical storage.
fn serialize_with<E>(
    debug_flag: u32,
    stack_flag: u32,
    checks: bool,
    stack_guard: &mut i32,
    mut call: impl FnMut(Hp2ObjectSerializationLeaf, usize, usize) -> Result<usize, E>,
) -> Result<(), E> {
    use Hp2ObjectSerializationLeaf::*;
    *stack_guard = 0;
    call(SetFlags, debug_flag as usize, 0)?;
    if call(Class, 0, 0)? != call(StaticClass, 0, 0)? {
        let class = call(Class, 0, 0)?;
        call(Preload, class, 0)?;
    }
    if (call(IsLoading, 0, 0)? == 0 && call(IsSaving, 0, 0)? == 0)
        || call(IsTrans, 0, 0)? != 0
    {
        let archive = call(Name, 0, 0)?;
        let archive = call(Outer, archive, 0)?;
        call(SerializeClass, archive, 0)?;
    }
    if call(IsLoading, 0, 0)? == 0 && call(IsSaving, 0, 0)? == 0 {
        call(Linker, 0, 0)?;
    }
    *stack_guard = 1;
    if call(IsTrans, 0, 0)? == 0 {
        if call(Flags, 0, 0)? & stack_flag as usize != 0 {
            if call(Frame, 0, 0)? == 0 {
                let object = call(Object, 0, 0)?;
                let class = call(Class, 0, 0)?;
                let frame = vm_frame::allocate_state(object as *mut c_void, class as *mut c_void);
                call(PublishFrame, frame as usize, 0)?;
            }
            let archive = call(Node, 0, 0)?;
            call(StateNode, archive, 0)?;
            call(ProbeMask, 0, 0)?;
            call(LatentAction, 0, 0)?;
            if call(FrameNode, 0, 0)? != 0 {
                let preload_node = call(FrameNode, 0, 0)?;
                call(Preload, preload_node, 0)?;
                if call(IsSaving, 0, 0)? != 0 && call(Code, 0, 0)? != 0 && checks {
                    let node = call(FrameNode, 0, 0)?;
                    let lower = call(ScriptAt, node, 0)?;
                    let valid_lower = call(Code, 0, 0)? >= lower;
                    let valid = valid_lower && {
                        let node = call(FrameNode, 0, 0)?;
                        let count = call(ScriptCount, node, 0)?;
                        let upper = call(ScriptAt, node, count)?;
                        call(Code, 0, 0)? < upper
                    };
                    call(CheckSaveCode, valid as usize, 0)?;
                }
                let mut offset = -1_i32;
                // Loading replaces Node before Code is rebased.
                if call(IsLoading, 0, 0)? == 0 && call(Code, 0, 0)? != 0 {
                    let code = call(Code, 0, 0)?;
                    let node = call(FrameNode, 0, 0)?;
                    let base = call(ScriptAt, node, 0)?;
                    let distance = code.wrapping_sub(base) as isize;
                    call(CheckNonnegativeOffset, (distance >= 0) as usize, 0)?;
                    offset = call(CheckedOffset, distance as usize, 0)? as i32;
                }
                call(Offset, (&mut offset as *mut i32) as usize, 0)?;
                if offset != -1
                    && (offset < 0
                        || {
                            let node = call(FrameNode, 0, 0)?;
                            offset >= call(ScriptCount, node, 0)? as i32
                        })
                {
                    call(InvalidOffset, offset as usize, 0)?;
                }
                let code = if offset != -1 {
                    let node = call(FrameNode, 0, 0)?;
                    call(ScriptAt, node, offset as usize)?
                } else {
                    0
                };
                call(PublishCode, code, 0)?;
            } else {
                call(PublishCode, 0, 0)?;
            }
        } else {
            let frame = call(Frame, 0, 0)?;
            if frame != 0 {
                unsafe { vm_frame::free_state(frame as *mut _) };
                call(PublishFrame, 0, 0)?;
            }
        }
    }
    *stack_guard = 0;
    if call(Class, 0, 0)? != call(StaticClass, 0, 0)? {
        if (call(IsLoading, 0, 0)? != 0 || call(IsSaving, 0, 0)? != 0)
            && call(IsTrans, 0, 0)? == 0
        {
            call(TaggedProperties, 0, 0)?;
        } else {
            call(BinaryProperties, 0, 0)?;
        }
    }
    let class = call(Class, 0, 0)?;
    let size = call(PropertiesSize, class, 0)?;
    call(CountBytes, size, 0)?;
    Ok(())
}

/// Runs the shared UObject serialization policy with the Rust-owned runtime.
///
/// # Errors
/// Returns the first runtime error without translating it through a native status.
pub(crate) fn serialize_with_runtime(
    debug_flag: u32,
    stack_flag: u32,
    checks: bool,
    stack_guard: &mut i32,
    call: impl FnMut(Hp2ObjectSerializationLeaf, usize, usize) -> crate::core_access::RuntimeResult<usize>,
) -> crate::core_access::RuntimeResult<()> {
    serialize_with(debug_flag, stack_flag, checks, stack_guard, call)
}

/// # Safety
/// Context and services are live for the entire call. Leaves capture native
/// exceptions and return their first failure. No SDK field is borrowed across a
/// callback. `stack_guard` remains set on a stack failure for the native shell.
/// Published persistent frames are null or uniquely owned allocate_state results.
/// Object/Class getters and frame publication access the live object's fields.
pub unsafe fn serialize(
    context: *mut c_void,
    services: &Hp2ObjectSerializationServices,
    debug_flag: u32,
    stack_flag: u32,
    checks: bool,
    stack_guard: *mut i32,
) -> Result<(), i32> {
    let stack_guard = unsafe { &mut *stack_guard };
    serialize_with(debug_flag, stack_flag, checks, stack_guard, |op, a, b| {
        let mut result = 0;
        let status = unsafe { (services.call)(context, op, a, b, &mut result) };
        if status == 0 { Ok(result) } else { Err(status) }
    })
}
