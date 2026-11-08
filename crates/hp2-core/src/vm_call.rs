//! Function-call policy with Rust-owned frames and reusable local storage.

use crate::{
    vm_bytecode,
    vm_frame::{self, Hp2VmFrameInitializationServices},
    vm_locals,
};
use hp2_abi::engine::generated::FFrame;
use std::ffi::c_void;
use std::mem::MaybeUninit;

type Object = *mut c_void;
const MAX_OUT_PARMS: usize = 16;

#[repr(C)]
pub struct Hp2VmCallServices {
    pub out_parm: u32,
    pub native: u32,
    pub end_parms: u8,
    pub debug_info: u8,
    pub native_index: unsafe extern "C" fn(Object) -> i32,
    pub function_flags: unsafe extern "C" fn(Object) -> u32,
    pub invoke_native: unsafe extern "C" fn(Object) -> i32,
    pub remote: unsafe extern "C" fn(Object, *mut i32) -> i32,
    pub locals_size: unsafe extern "C" fn(Object) -> i32,
    pub code: unsafe extern "C" fn(Object) -> *mut *mut u8,
    pub prop_addr: unsafe extern "C" fn(Object) -> *mut *mut u8,
    pub first_property: unsafe extern "C" fn(Object) -> Object,
    pub next_property: unsafe extern "C" fn(Object) -> Object,
    pub property_flags: unsafe extern "C" fn(Object) -> u32,
    pub property_offset: unsafe extern "C" fn(Object) -> i32,
    pub step: unsafe extern "C" fn(Object, *mut u8) -> i32,
    pub capture_observer: unsafe extern "C" fn() -> Object,
    pub observer_enter: unsafe extern "C" fn(Object, Object) -> i32,
    pub process_internal: unsafe extern "C" fn(Object) -> i32,
    pub observer_exit: unsafe extern "C" fn(Object, Object) -> i32,
    pub copy: unsafe extern "C" fn(Object, *mut u8, *mut u8) -> i32,
    pub first_constructor: unsafe extern "C" fn(Object) -> Object,
    pub next_constructor: unsafe extern "C" fn(Object) -> Object,
    pub destroy: unsafe extern "C" fn(Object, *mut u8) -> i32,
    pub frame_object: unsafe extern "C" fn(Object) -> Object,
    pub frame_node: unsafe extern "C" fn(Object) -> Object,
    pub frame_parent: unsafe extern "C" fn(Object) -> *mut FFrame,
    pub publish_frame: unsafe extern "C" fn(Object, *mut FFrame),
    pub frame_initialization: *const Hp2VmFrameInitializationServices,
}

fn status(value: i32) -> Result<(), i32> {
    if value == 0 { Ok(()) } else { Err(value) }
}

/// # Safety
/// Context/services and valid function metadata remain live through reentry.
/// All callbacks catch native exceptions and use the published frame only
/// during the call.
pub unsafe fn call(context: Object, s: &Hp2VmCallServices) -> Result<(), i32> {
    unsafe {
        if (s.native_index)(context) != 0 {
            return status((s.invoke_native)(context));
        }
        let mut skip = 0;
        if (s.function_flags)(context) & s.native != 0 {
            status((s.remote)(context, &mut skip))?;
            if skip == 0 {
                return status((s.invoke_native)(context));
            }
        }
        let size = (s.locals_size)(context) as usize;
        vm_locals::with_locals(size, |locals| {
            std::ptr::write_bytes(locals, 0, size);
            frame(context, skip, locals, s)
        })
    }
}

#[derive(Clone, Copy)]
struct OutParm {
    property: Object,
    address: *mut u8,
}

/// # Safety
/// Called inside `vm_locals::with_locals` while the zeroed locals remain live.
/// Initialization services and frame metadata are valid; the published frame
/// pointer is used only during this continuation, including callback reentry.
/// Valid function metadata describes at most 16 out parameters; bytecode ends
/// its argument expressions with the compiled end-parms token. Metadata, code
/// and globals may change at callbacks and are read at their native sequence
/// points. No callback may unwind over Rust.
unsafe fn frame(
    context: Object,
    skip: i32,
    locals: *mut u8,
    s: &Hp2VmCallServices,
) -> Result<(), i32> {
    unsafe {
        let mut storage = MaybeUninit::<FFrame>::uninit();
        let node = (s.frame_node)(context);
        let object = (s.frame_object)(context);
        let parent = (s.frame_parent)(context);
        vm_frame::initialize(
            storage.as_mut_ptr(),
            object,
            node,
            0,
            locals,
            parent,
            &*s.frame_initialization,
        )?;
        (s.publish_frame)(context, storage.as_mut_ptr());
        let code = (s.code)(context);
        let prop_addr = (s.prop_addr)(context);
        let mut outs: [MaybeUninit<OutParm>; MAX_OUT_PARMS] =
            [MaybeUninit::uninit(); MAX_OUT_PARMS];
        let mut count = 0;
        let mut property = (s.first_property)(context);
        while vm_bytecode::peek::<u8>(code) != s.end_parms {
            prop_addr.write(std::ptr::null_mut());
            let destination = std::ptr::addr_of!((*storage.as_ptr()).locals)
                .read()
                .offset((s.property_offset)(property) as isize);
            status((s.step)(context, destination))?;
            if (s.property_flags)(property) & s.out_parm != 0 && !prop_addr.read().is_null() {
                outs.get_unchecked_mut(count).write(OutParm {
                    property,
                    address: prop_addr.read(),
                });
                count += 1;
            }
            property = (s.next_property)(property);
        }
        vm_bytecode::advance(code, 1);
        if vm_bytecode::peek::<u8>(code) == s.debug_info {
            status((s.step)(context, std::ptr::null_mut()))?;
        }
        if skip == 0 {
            let observer = (s.capture_observer)();
            if !observer.is_null() {
                status((s.observer_enter)(context, observer))?;
            }
            status((s.process_internal)(context))?;
            if !observer.is_null() {
                status((s.observer_exit)(context, observer))?;
            }
        }
        while count != 0 {
            count -= 1;
            let out = outs.get_unchecked(count).assume_init_read();
            let source = std::ptr::addr_of!((*storage.as_ptr()).locals)
                .read()
                .offset((s.property_offset)(out.property) as isize);
            status((s.copy)(out.property, out.address, source))?;
        }
        let mut property = (s.first_constructor)(context);
        while !property.is_null() {
            let data = std::ptr::addr_of!((*storage.as_ptr()).locals)
                .read()
                .offset((s.property_offset)(property) as isize);
            status((s.destroy)(property, data))?;
            property = (s.next_constructor)(property);
        }
        Ok(())
    }
}
