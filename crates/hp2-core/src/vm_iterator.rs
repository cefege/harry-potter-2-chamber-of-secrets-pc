//! Source `PRE_ITERATOR`/`POST_ITERATOR` control, shared by native and canonical hosts.

use std::ffi::c_void;
use hp2_abi::engine::generated::FFrame;

/// Runs a native iterator after its argument terminator. Arguments are evaluated once;
/// the following word is the absolute script offset of `IteratorPop`.
///
/// `get_next` owns selection and exhaustion writes. Breaking preserves its last output.
/// Return/Stop remain unconsumed for the enclosing VM, including through nested iterators,
/// so a function return writes to the caller's result rather than this body's scratch.
///
/// # Safety
/// The frame, bytecode and callbacks remain live. `script_code` resolves the supplied
/// absolute script offset, and `step` executes one ordinary opcode using the scratch result.
/// Callbacks may reenter the VM but must not retain the frame or scratch pointer.
pub unsafe fn run<E>(
    frame: *mut FFrame,
    mut script_code: impl FnMut(i32) -> Result<*mut u8, E>,
    mut step: impl FnMut(*mut c_void) -> Result<(), E>,
    mut get_next: impl FnMut() -> Result<bool, E>,
) -> Result<(), E> {
    unsafe {
        let end_offset = crate::vm_bytecode::read::<u16>(&raw mut (*frame).code) as i32;
        let start = (*frame).code;
        // MAX_CONST_SIZE, with alignment sufficient for native result types.
        let mut scratch = [0_u128; std::mem::size_of::<crate::ansi::Tchar>()];
        loop {
            if !get_next()? {
                (*frame).code = script_code(end_offset + 1)?;
                return Ok(());
            }
            loop {
                let code = (*frame).code;
                if code.is_null() { return Ok(()); }
                match code.read() {
                    0x04 | 0x08 => return Ok(()), // Return / Stop: enclosing VM owns control.
                    0x30 => { // IteratorPop / break.
                        (*frame).code = code.add(1);
                        return Ok(());
                    }
                    0x31 => { // IteratorNext.
                        (*frame).code = start;
                        break;
                    }
                    _ => step(scratch.as_mut_ptr().cast())?,
                }
            }
        }
    }
}
