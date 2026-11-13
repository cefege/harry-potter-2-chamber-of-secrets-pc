//! Default snapshot forwarding for legacy and result-aware VmHost implementations.
use hp2_abi::engine::generated::{FFrame, TArrayRaw, UFunction};
use hp2_core::{
    canonical_vm_native::CanonicalVmNative,
    core_access::{RuntimeError, RuntimeResult},
    object_owner::CanonicalNativeBinding,
    vm_owner::{EventBoundary, StepControl, VmCallOp, VmExecutor, VmFlowOp, VmHost, VmPropertyOp, VmStringOp, VmSystemOp},
};
use std::{alloc::Layout, cell::Cell, ffi::c_void, ptr::NonNull};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Seen {
    object: *mut c_void,
    function: *mut UFunction,
    frame: *mut FFrame,
    observed: *mut c_void,
    body: Option<*mut c_void>,
    from_script: bool,
}

struct LegacyHost(Cell<Option<Seen>>);
struct ResultHost(Cell<Option<Seen>>);

// These leaves are not part of this forwarding test. A call to any of them
// must fail visibly, rather than silently simulating an engine operation.
macro_rules! unused_host_leaves {
    () => {
        fn object_at(&self, _: i32) -> RuntimeResult<*mut c_void> { panic!("unexpected object_at") }
        fn native_binding(&self, _: *mut UFunction) -> RuntimeResult<CanonicalNativeBinding> { panic!("unexpected native_binding") }
        fn external_native(&self, _: u32) -> RuntimeResult<Option<CanonicalVmNative>> { panic!("unexpected external_native") }
        unsafe fn execute_external_native(&self, _: CanonicalVmNative, _: *mut c_void, _: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> { panic!("unexpected execute_external_native") }
        fn name_text(&self, _: i32) -> RuntimeResult<Vec<u32>> { panic!("unexpected name_text") }
        fn find_name(&self, _: &[u32]) -> RuntimeResult<i32> { panic!("unexpected find_name") }
        fn none_name(&self) -> RuntimeResult<i32> { panic!("unexpected none_name") }
        fn log(&self, _: i32, _: &[u32]) -> RuntimeResult<()> { panic!("unexpected log") }
        fn config_get(&self, _: &[u32], _: &[u32]) -> RuntimeResult<Option<Vec<u32>>> { panic!("unexpected config_get") }
        fn config_set(&self, _: &[u32], _: &[u32], _: &[u32]) -> RuntimeResult<()> { panic!("unexpected config_set") }
        fn archive_object_reference(&self, _: *mut c_void) -> RuntimeResult<*mut c_void> { panic!("unexpected archive_object_reference") }
        fn property_address(&self, _: *mut c_void, _: *mut c_void) -> RuntimeResult<*mut u8> { panic!("unexpected property_address") }
        fn reference_argument(&self, _: &VmExecutor<'_, Self>, _: *mut FFrame, _: *mut c_void, _: usize) -> RuntimeResult<*mut c_void> { panic!("unexpected reference_argument") }
        fn assign_string_constant(&self, _: &VmExecutor<'_, Self>, _: bool, _: *mut FFrame, _: *mut c_void) -> RuntimeResult<()> { panic!("unexpected assign_string_constant") }
        fn allocate_locals(&self, _: Layout) -> RuntimeResult<NonNull<u8>> { panic!("unexpected allocate_locals") }
        fn free_locals(&self, _: NonNull<u8>, _: Layout) { panic!("unexpected free_locals") }
        fn construct_locals(&self, _: *mut UFunction, _: *mut u8) -> RuntimeResult<()> { panic!("unexpected construct_locals") }
        fn destroy_locals(&self, _: *mut UFunction, _: *mut u8) -> RuntimeResult<()> { panic!("unexpected destroy_locals") }
        fn string_argument(&self, _: &VmExecutor<'_, Self>, _: *mut FFrame, _: *mut TArrayRaw) -> RuntimeResult<()> { panic!("unexpected string_argument") }
        fn release_string(&self, _: *mut TArrayRaw) -> RuntimeResult<()> { panic!("unexpected release_string") }
        fn execute_property(&self, _: &VmExecutor<'_, Self>, _: VmPropertyOp, _: *mut c_void, _: *mut FFrame, _: *mut c_void) -> RuntimeResult<StepControl> { panic!("unexpected execute_property") }
        fn execute_call(&self, _: &VmExecutor<'_, Self>, _: VmCallOp, _: *mut c_void, _: *mut FFrame, _: *mut c_void) -> RuntimeResult<StepControl> { panic!("unexpected execute_call") }
        fn execute_system(&self, _: &VmExecutor<'_, Self>, _: VmSystemOp, _: u32, _: *mut c_void, _: *mut FFrame, _: *mut c_void) -> RuntimeResult<StepControl> { panic!("unexpected execute_system") }
        fn execute_string(&self, _: &VmExecutor<'_, Self>, _: VmStringOp, _: u32, _: *mut c_void, _: *mut FFrame, _: *mut c_void) -> RuntimeResult<StepControl> { panic!("unexpected execute_string") }
        fn execute_flow(&self, _: &VmExecutor<'_, Self>, _: VmFlowOp, _: *mut c_void, _: *mut FFrame, _: *mut c_void) -> RuntimeResult<StepControl> { panic!("unexpected execute_flow") }
        fn event_boundary(&self, _: EventBoundary, _: *mut c_void, _: *mut UFunction, _: *mut FFrame) -> RuntimeResult<()> { panic!("unexpected event_boundary") }
        fn check_runaway(&self, _: *mut c_void, _: *mut UFunction, _: *mut FFrame, _: u64) -> RuntimeResult<()> { panic!("unexpected check_runaway") }
    };
}

impl VmHost for LegacyHost {
    unused_host_leaves!();

    fn interpreted_call(&self, object: *mut c_void, function: *mut UFunction, frame: *mut FFrame, observed: *mut c_void, from_script: bool, execute: impl FnOnce() -> RuntimeResult<()>) -> RuntimeResult<()> {
        assert!(self.0.replace(Some(Seen { object, function, frame, observed, body: None, from_script })).is_none(), "host must be entered exactly once");
        execute()
    }
}

impl VmHost for ResultHost {
    unused_host_leaves!();

    fn interpreted_call_with_result(&self, object: *mut c_void, function: *mut UFunction, frame: *mut FFrame, observed: *mut c_void, body: *mut c_void, from_script: bool, execute: impl FnOnce() -> RuntimeResult<()>) -> RuntimeResult<()> {
        assert!(self.0.replace(Some(Seen { object, function, frame, observed, body: Some(body), from_script })).is_none(), "host must be entered exactly once");
        execute()
    }
}

fn exercise<H: VmHost>(host: &H, seen: &Cell<Option<Seen>>, result_aware: bool) {
    // Distinct live opaque addresses detect accidental replacement or swapping.
    // Their pointees are never dereferenced as UObject, UFunction, or FFrame.
    let mut object = 0_u8;
    let mut function = 0_u8;
    let mut frame = 0_u8;
    let mut observed = 0_u8;
    let mut body = 0_u8;
    let mut script = [0_u8; 17];
    let mut locals = [0_u8; 20];
    let object = (&raw mut object).cast::<c_void>();
    let function = (&raw mut function).cast::<UFunction>();
    let frame = (&raw mut frame).cast::<FFrame>();
    let observed = (&raw mut observed).cast::<c_void>();
    let body = (&raw mut body).cast::<c_void>();
    assert_ne!(observed, body);
    for from_script in [false, true] {
        for fail in [false, true] {
            for locals_hook in [false, true] {
                seen.set(None);
                let count = Cell::new(0);
                let execute = || {
                    count.set(count.get() + 1);
                    if fail { Err(RuntimeError::new("snapshot closure failure")) } else { Ok(()) }
                };
                let result = if locals_hook {
                    host.interpreted_call_with_locals_snapshot(
                        object, function, frame, observed, body, from_script,
                        script.as_mut_ptr(), script.len(), locals.as_mut_ptr(), locals.len(), 12, execute,
                    )
                } else {
                    host.interpreted_call_with_script_snapshot(
                        object, function, frame, observed, body, from_script,
                        script.as_mut_ptr(), script.len(), execute,
                    )
                };
                assert_eq!(count.get(), 1, "the forwarded closure must run exactly once");
                assert_eq!(seen.get(), Some(Seen { object, function, frame, observed, body: result_aware.then_some(body), from_script }));
                if fail {
                    let error = result.expect_err("closure failure must propagate");
                    assert_eq!(error.diagnostic(), "snapshot closure failure".chars().map(u32::from).collect::<Vec<_>>());
                    assert!(error.context().is_empty(), "default forwarding must not add context");
                } else {
                    result.expect("closure success must propagate");
                }
            }
        }
    }
}

#[test]
fn snapshot_default_forwards_legacy_and_result_hosts() {
    let legacy = LegacyHost(Cell::new(None));
    exercise(&legacy, &legacy.0, false);
    let result = ResultHost(Cell::new(None));
    exercise(&result, &result.0, true);
}

// This host overrides the OLD snapshot hook only. The new default must call
// this exact override rather than bypassing it for interpreted_call_with_result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ScriptSeen {
    call: Seen,
    script: *mut u8,
    script_length: usize,
}
struct ScriptSnapshotHost(Cell<Option<ScriptSeen>>);
impl VmHost for ScriptSnapshotHost {
    unused_host_leaves!();
    fn interpreted_call_with_script_snapshot(
        &self, object: *mut c_void, function: *mut UFunction, frame: *mut FFrame,
        observed: *mut c_void, body: *mut c_void, from_script: bool,
        script: *mut u8, script_length: usize, execute: impl FnOnce() -> RuntimeResult<()>,
    ) -> RuntimeResult<()> {
        assert!(self.0.replace(Some(ScriptSeen {
            call: Seen { object, function, frame, observed, body: Some(body), from_script },
            script, script_length,
        })).is_none(), "old snapshot override must be entered exactly once");
        execute()
    }
}

#[test]
fn locals_snapshot_default_forwards_existing_snapshot_override() {
    // These are distinct initialized opaque identities, never actual VM frames
    // or function objects; neither the host nor closure dereferences them.
    let mut object = 0_u8;
    let mut function = 0_u8;
    let mut frame = 0_u8;
    let mut observed = 0_u8;
    let mut body = 0_u8;
    let mut script = [0_u8; 17];
    let mut locals = [0_u8; 20];
    let object = (&raw mut object).cast::<c_void>();
    let function = (&raw mut function).cast::<UFunction>();
    let frame = (&raw mut frame).cast::<FFrame>();
    let observed = (&raw mut observed).cast::<c_void>();
    let body = (&raw mut body).cast::<c_void>();
    let host = ScriptSnapshotHost(Cell::new(None));
    for from_script in [false, true] {
        for fail in [false, true] {
            host.0.set(None);
            let calls = Cell::new(0);
            let result = host.interpreted_call_with_locals_snapshot(
                object, function, frame, observed, body, from_script,
                script.as_mut_ptr(), script.len(), locals.as_mut_ptr(), locals.len(), 12, || {
                    calls.set(calls.get() + 1);
                    if fail { Err(RuntimeError::new("locals forwarding closure failure")) } else { Ok(()) }
                },
            );
            assert_eq!(calls.get(), 1, "closure runs exactly once");
            assert_eq!(host.0.get(), Some(ScriptSeen {
                call: Seen { object, function, frame, observed, body: Some(body), from_script },
                script: script.as_mut_ptr(), script_length: script.len(),
            }), "old override receives the original arguments and script snapshot");
            if fail {
                let error = result.expect_err("closure error must propagate unchanged");
                assert_eq!(error.diagnostic(), "locals forwarding closure failure".chars().map(u32::from).collect::<Vec<_>>());
                assert!(error.context().is_empty(), "no context added by default forwarding");
            } else {
                result.expect("closure success must propagate");
            }
        }
    }
}
