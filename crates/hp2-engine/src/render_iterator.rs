//! URenderIterator cursor and observer policy over checked native member leaves.
use core::ffi::c_void;

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2RenderIteratorOperation {
    Init,
    UnInit,
    First,
    Next,
    IsDone,
    CurrentItem,
}

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2RenderIteratorLeaf {
    SetFrame,
    CameraActor,
    SetObserver,
    ReadIndex,
    WriteIndex,
    ReadMaxItems,
    Outer,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2RenderIteratorIo {
    pub pointer: *mut c_void,
    pub value: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2RenderIteratorServices {
    pub context: *mut c_void,
    pub invoke: Option<
        unsafe extern "C" fn(*mut c_void, Hp2RenderIteratorLeaf, *mut Hp2RenderIteratorIo) -> i32,
    >,
}

/// # Safety
/// The copied service table must call live native storage and contain native
/// faults. No reference to the native owner is made or retained across calls.
pub unsafe fn run(
    services: Hp2RenderIteratorServices,
    operation: Hp2RenderIteratorOperation,
    camera: *mut c_void,
) -> Result<Hp2RenderIteratorIo, i32> {
    let invoke = services.invoke.ok_or(1)?;
    let call = |leaf, io: &mut Hp2RenderIteratorIo| {
        let status = unsafe { invoke(services.context, leaf, io) };
        if status == 0 { Ok(()) } else { Err(status) }
    };
    let mut io = Hp2RenderIteratorIo {
        pointer: core::ptr::null_mut(),
        value: 0,
    };
    use Hp2RenderIteratorLeaf as Leaf;
    match operation {
        Hp2RenderIteratorOperation::Init => {
            io.pointer = camera;
            call(Leaf::SetFrame, &mut io)?;
            if !camera.is_null() {
                call(Leaf::CameraActor, &mut io)?;
                call(Leaf::SetObserver, &mut io)?;
            }
        }
        Hp2RenderIteratorOperation::UnInit => call(Leaf::SetFrame, &mut io)?,
        Hp2RenderIteratorOperation::First => call(Leaf::WriteIndex, &mut io)?,
        Hp2RenderIteratorOperation::Next => {
            call(Leaf::ReadIndex, &mut io)?;
            // Original signed increment is defined through INT_MAX-1 only.
            // Wrapping avoids a Rust panic without adding a native check/fault.
            io.value = io.value.wrapping_add(1);
            call(Leaf::WriteIndex, &mut io)?;
        }
        Hp2RenderIteratorOperation::IsDone => {
            call(Leaf::ReadIndex, &mut io)?;
            let index = io.value;
            call(Leaf::ReadMaxItems, &mut io)?;
            io.value = i32::from(index >= io.value);
        }
        Hp2RenderIteratorOperation::CurrentItem => call(Leaf::Outer, &mut io)?,
    }
    Ok(io)
}
