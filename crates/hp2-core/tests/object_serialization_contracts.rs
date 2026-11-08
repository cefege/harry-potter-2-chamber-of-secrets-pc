use hp2_core::object_serialization::{
    serialize, Hp2ObjectSerializationLeaf as Leaf, Hp2ObjectSerializationServices,
};
use hp2_core::vm_frame;
use std::ffi::c_void;

// A service-level Rust state model, not a fabricated native object image.
// Both scripts share one allocation so the old script is deterministically below
// the decoded node's script, independently of allocator ordering or ASLR.
struct State {
    scripts: Box<[u8; 256]>,
    frame: *mut c_void,
    loading: bool,
    node: usize,
    code: usize,
    wire_offset: i32,
    flags: usize,
}

impl State {
    fn new(loading: bool, wire_offset: i32) -> Self {
        let scripts = Box::new([0; 256]);
        let code = scripts.as_ptr() as usize + 3;
        Self {
            scripts,
            frame: vm_frame::allocate_state(std::ptr::null_mut(), std::ptr::null_mut()).cast(),
            loading,
            node: 1,
            code,
            wire_offset,
            flags: 1,
        }
    }

    fn script_base(&self, node: usize) -> usize {
        self.scripts.as_ptr() as usize + if node == 2 { 128 } else { 0 }
    }

    fn run(&mut self) -> (Result<(), i32>, i32) {
        let services = Hp2ObjectSerializationServices { call };
        let mut guard = -1;
        let result = unsafe {
            serialize(
                (self as *mut Self).cast(),
                &services,
                2,
                1,
                true,
                &mut guard,
            )
        };
        (result, guard)
    }
}

impl Drop for State {
    fn drop(&mut self) {
        unsafe { vm_frame::free_state(self.frame.cast()) };
    }
}

unsafe extern "C" fn call(
    context: *mut c_void,
    leaf: Leaf,
    a: usize,
    b: usize,
    result: *mut usize,
) -> i32 {
    let state = unsafe { &mut *context.cast::<State>() };
    let value = match leaf {
        Leaf::SetFlags => {
            state.flags |= a;
            0
        }
        Leaf::Class | Leaf::StaticClass => 0,
        Leaf::IsLoading => state.loading as usize,
        Leaf::IsSaving => (!state.loading) as usize,
        Leaf::Flags => state.flags,
        Leaf::Frame => state.frame as usize,
        Leaf::Node => {
            if state.loading {
                state.node = 2;
            }
            0
        }
        Leaf::FrameNode => state.node,
        Leaf::Code => state.code,
        Leaf::PublishCode => {
            state.code = a;
            0
        }
        Leaf::ScriptAt => state.script_base(a) + b,
        Leaf::ScriptCount => 128,
        Leaf::CheckSaveCode | Leaf::CheckNonnegativeOffset => {
            if a == 0 {
                return 73;
            }
            0
        }
        Leaf::CheckedOffset => match i32::try_from(a) {
            Ok(offset) => offset as usize,
            Err(_) => return 74,
        },
        Leaf::Offset => {
            let offset = a as *mut i32;
            if state.loading {
                unsafe { offset.write(state.wire_offset) };
            } else {
                state.wire_offset = unsafe { offset.read() };
            }
            0
        }
        Leaf::InvalidOffset => return 79,
        Leaf::StateNode
        | Leaf::ProbeMask
        | Leaf::LatentAction
        | Leaf::Preload
        | Leaf::IsTrans
        | Leaf::PropertiesSize
        | Leaf::CountBytes => 0,
        _ => return 91,
    };
    unsafe { result.write(value) };
    0
}

#[test]
fn cross_node_load_reconstructs_code_from_wire_not_the_previous_script() {
    for offset in [0, 7, 127, -1] {
        let mut state = State::new(true, offset);
        assert!(state.code < state.script_base(2));
        assert_eq!(state.run(), (Ok(()), 0));
        assert_eq!(state.node, 2);
        assert_eq!(
            state.code,
            if offset == -1 {
                0
            } else {
                state.script_base(2) + offset as usize
            }
        );
        assert_eq!(state.flags, 3);
    }
}

#[test]
fn invalid_loaded_offset_preserves_the_old_code_and_committed_new_node() {
    for offset in [-2, 128] {
        let mut state = State::new(true, offset);
        let old_code = state.code;
        assert_eq!(state.run(), (Err(79), 1));
        assert_eq!(state.node, 2);
        assert_eq!(state.code, old_code);
    }
}

#[test]
fn save_rejects_code_outside_the_live_node_script() {
    let mut state = State::new(false, -1);
    state.node = 2;
    let old_code = state.code;
    assert_eq!(state.run(), (Err(73), 1));
    assert_eq!(state.code, old_code);
    assert_eq!(state.wire_offset, -1);
}

#[test]
fn save_encodes_the_live_script_offset() {
    let mut state = State::new(false, -1);
    state.node = 2;
    state.code = state.script_base(2) + 11;
    assert_eq!(state.run(), (Ok(()), 0));
    assert_eq!(state.wire_offset, 11);
    assert_eq!(state.code, state.script_base(2) + 11);
}
