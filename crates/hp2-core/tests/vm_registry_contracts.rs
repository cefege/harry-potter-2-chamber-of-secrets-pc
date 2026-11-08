use hp2_abi::engine::generated::FFrame;
use hp2_core::vm_registry::{
    Hp2VmNativeHandler, INDEX_NONE, NATIVE_COUNT, execute_high_native, high_index, register_entries,
};
use std::ffi::c_void;

unsafe extern "C" fn undefined(_: *mut c_void, _: *mut c_void, _: *mut c_void) -> i32 {
    61
}
unsafe extern "C" fn first(context: *mut c_void, _: *mut c_void, result: *mut c_void) -> i32 {
    unsafe { result.cast::<u32>().write(context.cast::<u32>().read() + 1) };
    0
}
unsafe extern "C" fn second(context: *mut c_void, _: *mut c_void, result: *mut c_void) -> i32 {
    unsafe { result.cast::<u32>().write(context.cast::<u32>().read() + 2) };
    73
}
const UNDEFINED: Hp2VmNativeHandler = undefined;
const FIRST: Hp2VmNativeHandler = first;
const SECOND: Hp2VmNativeHandler = second;

fn is_handler(entry: Option<Hp2VmNativeHandler>, handler: Hp2VmNativeHandler) -> bool {
    entry.is_some_and(|entry| std::ptr::fn_addr_eq(entry, handler))
}

// Every test owns the complete state. No test reads or mutates process-global
// registration, so the normal concurrent Rust test runner cannot race it.
struct Registry {
    table: [Option<Hp2VmNativeHandler>; NATIVE_COUNT],
    initialized: bool,
    duplicate: i32,
}

impl Registry {
    fn new() -> Self {
        Self {
            table: [None; NATIVE_COUNT],
            initialized: false,
            duplicate: 0,
        }
    }

    fn register(&mut self, index: i32, function: Hp2VmNativeHandler) {
        unsafe {
            register_entries(
                self.table.as_mut_ptr(),
                &raw mut self.initialized,
                &raw mut self.duplicate,
                index,
                function,
                UNDEFINED,
            )
            .unwrap();
        }
    }
}

#[test]
fn first_sentinel_initializes_every_slot_without_registering() {
    let mut registry = Registry::new();
    registry.register(INDEX_NONE, FIRST);
    assert!(registry.initialized);
    assert_eq!(registry.duplicate, 0);
    assert!(registry.table.iter().all(|entry| is_handler(*entry, UNDEFINED)));
}

#[test]
fn first_real_registration_initializes_all_other_slots() {
    let mut registry = Registry::new();
    registry.register(237, FIRST);
    assert!(registry.initialized);
    assert_eq!(registry.duplicate, 0);
    for (index, entry) in registry.table.iter().enumerate() {
        assert!(is_handler(*entry, if index == 237 { FIRST } else { UNDEFINED }));
    }
}

#[test]
fn later_sentinel_preserves_entries_and_duplicate_marker() {
    let mut registry = Registry::new();
    registry.register(57, FIRST);
    registry.register(57, SECOND);
    registry.register(INDEX_NONE, UNDEFINED);
    assert!(is_handler(registry.table[57], SECOND));
    assert!(is_handler(registry.table[58], UNDEFINED));
    assert_eq!(registry.duplicate, 57);
}

#[test]
fn duplicate_records_index_and_replaces_handler() {
    let mut registry = Registry::new();
    registry.register(217, FIRST);
    registry.register(217, SECOND);
    assert_eq!(registry.duplicate, 217);
    assert!(is_handler(registry.table[217], SECOND));
}

#[test]
fn repeated_same_function_is_still_a_duplicate() {
    let mut registry = Registry::new();
    registry.register(109, FIRST);
    registry.register(109, FIRST);
    assert_eq!(registry.duplicate, 109);
    assert!(is_handler(registry.table[109], FIRST));
}

#[test]
fn latest_duplicate_survives_subsequent_unique_registration() {
    let mut registry = Registry::new();
    registry.register(7, FIRST);
    registry.register(7, SECOND);
    registry.register(4081, FIRST);
    registry.register(4081, SECOND);
    registry.register(11, FIRST);
    assert_eq!(registry.duplicate, 4081);
    assert!(is_handler(registry.table[7], SECOND));
    assert!(is_handler(registry.table[4081], SECOND));
    assert!(is_handler(registry.table[11], FIRST));
}

#[test]
fn duplicate_at_zero_keeps_legacy_zero_marker_and_overwrites() {
    let mut registry = Registry::new();
    registry.register(9, FIRST);
    registry.register(9, SECOND);
    registry.register(0, FIRST);
    registry.register(0, SECOND);
    assert_eq!(registry.duplicate, 0);
    assert!(is_handler(registry.table[0], SECOND));
}

#[test]
fn registering_undefined_does_not_occupy_a_slot() {
    let mut registry = Registry::new();
    registry.register(41, UNDEFINED);
    registry.register(41, FIRST);
    assert_eq!(registry.duplicate, 0);
    assert!(is_handler(registry.table[41], FIRST));
}

#[test]
fn restoring_undefined_reopens_slot_without_resetting_duplicate_marker() {
    let mut registry = Registry::new();
    registry.register(41, FIRST);
    registry.register(41, UNDEFINED);
    registry.register(72, FIRST);
    registry.register(72, SECOND);
    registry.register(41, SECOND);
    assert_eq!(registry.duplicate, 72);
    assert!(is_handler(registry.table[41], SECOND));
}

#[test]
fn last_table_entry_is_valid_and_never_aliases_its_neighbor() {
    let mut registry = Registry::new();
    registry.register((NATIVE_COUNT - 1) as i32, FIRST);
    assert_eq!(registry.duplicate, 0);
    registry.register((NATIVE_COUNT - 1) as i32, SECOND);
    assert_eq!(registry.duplicate, (NATIVE_COUNT - 1) as i32);
    assert!(is_handler(registry.table[NATIVE_COUNT - 1], SECOND));
    assert!(is_handler(registry.table[NATIVE_COUNT - 2], UNDEFINED));
    assert!(is_handler(registry.table[0], UNDEFINED));
}

#[test]
fn registered_handlers_execute_and_preserve_status_and_output() {
    let mut registry = Registry::new();
    let mut context = 40u32;
    let mut result = 0u32;
    for (handler, status, output) in [(FIRST, 0, 41), (SECOND, 73, 42)] {
        registry.register(23, handler);
        let selected = registry.table[23].unwrap();
        assert_eq!(
            unsafe {
                selected(
                    (&raw mut context).cast(),
                    std::ptr::null_mut(),
                    (&raw mut result).cast(),
                )
            },
            status
        );
        assert_eq!(result, output);
    }
    registry.register(23, UNDEFINED);
    assert_eq!(
        unsafe {
            registry.table[23].unwrap()(
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        },
        61
    );
}

#[test]
fn invalid_indices_initialize_once_without_changing_entries_or_duplicate() {
    let mut registry = Registry::new();
    for index in [-2, NATIVE_COUNT as i32, i32::MAX] {
        assert_eq!(
            unsafe {
                register_entries(
                    registry.table.as_mut_ptr(),
                    &raw mut registry.initialized,
                    &raw mut registry.duplicate,
                    index,
                    FIRST,
                    UNDEFINED,
                )
            },
            Err(1)
        );
        assert!(registry.initialized);
        assert!(registry.table.iter().all(|entry| is_handler(*entry, UNDEFINED)));
        assert_eq!(registry.duplicate, 0);
    }
}

#[test]
fn every_high_prefix_and_byte_boundary_maps_inside_native_table() {
    for prefix in 0..=15 {
        for low in [0, 1, 0x7f, 0xfe, 0xff] {
            let mut code = [low, 0x5a];
            let mut cursor = code.as_mut_ptr();
            let index = unsafe { high_index(prefix, &raw mut cursor) };
            assert_eq!(index, u32::from(prefix) * 256 + u32::from(low));
            assert!((index as usize) < NATIVE_COUNT);
            assert_eq!(cursor, unsafe { code.as_mut_ptr().add(1) });
            assert_eq!(unsafe { cursor.read() }, 0x5a);
        }
    }
}

#[test]
fn consecutive_high_operands_consume_exactly_one_byte_each() {
    let mut code = [0x00, 0xff, 0x7f];
    let mut cursor = code.as_mut_ptr();
    unsafe {
        assert_eq!(high_index(0, &raw mut cursor), 0);
        assert_eq!(cursor, code.as_mut_ptr().add(1));
        assert_eq!(high_index(15, &raw mut cursor), 0xfff);
        assert_eq!(cursor, code.as_mut_ptr().add(2));
        assert_eq!(high_index(8, &raw mut cursor), 0x87f);
        assert_eq!(cursor, code.as_mut_ptr().add(3));
    }
}

#[test]
fn high_decoding_uses_the_current_live_cursor_after_redirection() {
    let mut first = [0x41, 0x99];
    let mut second = [0x80, 0x00];
    let mut cursor = first.as_mut_ptr();
    unsafe {
        assert_eq!(high_index(1, &raw mut cursor), 0x141);
        cursor = second.as_mut_ptr();
        assert_eq!(high_index(14, &raw mut cursor), 0xe80);
        assert_eq!(cursor, second.as_mut_ptr().add(1));
        assert_eq!(cursor.read(), 0x00);
    }
}

struct DispatchProbe {
    calls: u32,
    observed_frame: *mut c_void,
    observed_result: *mut c_void,
    observed_code: *mut u8,
    outer_code: *mut u8,
    table: *mut Hp2VmNativeHandler,
    redirect: *mut u8,
    status: i32,
}

impl DispatchProbe {
    fn new(status: i32) -> Self {
        Self {
            calls: 0,
            observed_frame: std::ptr::null_mut(),
            observed_result: std::ptr::null_mut(),
            observed_code: std::ptr::null_mut(),
            outer_code: std::ptr::null_mut(),
            table: std::ptr::null_mut(),
            redirect: std::ptr::null_mut(),
            status,
        }
    }
}

unsafe extern "C" fn observe_dispatch(
    object: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    let probe = object.cast::<DispatchProbe>();
    unsafe {
        (*probe).calls += 1;
        (*probe).observed_frame = frame;
        (*probe).observed_result = result;
        (*probe).observed_code = (*frame.cast::<FFrame>()).code;
        (*probe).status
    }
}

unsafe extern "C" fn nested_dispatch(
    object: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    let probe = object.cast::<DispatchProbe>();
    let frame = frame.cast::<FFrame>();
    unsafe {
        (*probe).calls += 1;
        (*probe).outer_code = (*frame).code;
        // A callback replaces the selected entry before the nested load.
        (*probe).table.add(0xfff).write(observe_dispatch);
        if !(*probe).redirect.is_null() {
            (*frame).code = (*probe).redirect;
        }
        execute_high_native::<15>(object, frame, result, (*probe).table)
    }
}

fn dispatch_frame(object: *mut c_void, code: *mut u8) -> FFrame {
    FFrame {
        node: std::ptr::null_mut(),
        object,
        code,
        locals: std::ptr::null_mut(),
        parent: std::ptr::null_mut(),
    }
}

fn check_dispatch_prefix<const PREFIX: u8>() {
    let mut registry = Registry::new();
    let mut incoming = DispatchProbe::new(0);
    let mut frame_object = DispatchProbe::new(79);
    let incoming_ptr = (&raw mut incoming).cast();
    let frame_object_ptr = (&raw mut frame_object).cast();
    for low in [0, 1, 0x7f, 0x80, 0xfe, 0xff] {
        let mut code = [low, 0x5a];
        let mut frame = dispatch_frame(frame_object_ptr, code.as_mut_ptr());
        let mut result = [0x1122_3344u32, 0x5566_7788, 0x99aa_bbcc];
        let result_ptr = (&raw mut result[1]).cast();
        registry.register(i32::from(PREFIX) * 256 + i32::from(low), observe_dispatch);
        let before = incoming.calls;
        assert_eq!(
            unsafe {
                execute_high_native::<PREFIX>(
                    incoming_ptr,
                    &raw mut frame,
                    result_ptr,
                    registry.table.as_ptr().cast(),
                )
            },
            0
        );
        assert_eq!(incoming.calls, before + 1);
        assert_eq!(frame_object.calls, 0);
        assert_eq!(incoming.observed_frame, (&raw mut frame).cast());
        assert_eq!(incoming.observed_result, result_ptr);
        assert_eq!(incoming.observed_code, unsafe { code.as_mut_ptr().add(1) });
        assert_eq!(frame.code, incoming.observed_code);
        assert_eq!(frame.object, frame_object_ptr);
        assert!(frame.node.is_null());
        assert!(frame.locals.is_null());
        assert!(frame.parent.is_null());
        assert_eq!(code, [low, 0x5a]);
        assert_eq!(result, [0x1122_3344, 0x5566_7788, 0x99aa_bbcc]);
    }
}

#[test]
fn every_high_dispatch_prefix_preserves_incoming_context_and_pointer_identity() {
    check_dispatch_prefix::<0>();
    check_dispatch_prefix::<1>();
    check_dispatch_prefix::<2>();
    check_dispatch_prefix::<3>();
    check_dispatch_prefix::<4>();
    check_dispatch_prefix::<5>();
    check_dispatch_prefix::<6>();
    check_dispatch_prefix::<7>();
    check_dispatch_prefix::<8>();
    check_dispatch_prefix::<9>();
    check_dispatch_prefix::<10>();
    check_dispatch_prefix::<11>();
    check_dispatch_prefix::<12>();
    check_dispatch_prefix::<13>();
    check_dispatch_prefix::<14>();
    check_dispatch_prefix::<15>();
}

#[test]
fn nested_high_dispatch_reads_live_table_and_cursor_and_returns_first_status() {
    for status in [0, 73, -37] {
        for redirect in [false, true] {
            let mut registry = Registry::new();
            registry.register(0x60, nested_dispatch);
            let mut incoming = DispatchProbe::new(status);
            incoming.table = registry.table.as_mut_ptr().cast();
            let mut frame_object = DispatchProbe::new(79);
            let frame_object_ptr = (&raw mut frame_object).cast();
            let mut code = [0x60, 0xff, 0x5a];
            let mut redirected = [0xff, 0x7c];
            if redirect {
                incoming.redirect = redirected.as_mut_ptr();
            }
            let mut frame = dispatch_frame(frame_object_ptr, code.as_mut_ptr());
            let mut result = [0x1122_3344u32, 0x5566_7788, 0x99aa_bbcc];
            let result_ptr = (&raw mut result[1]).cast();
            assert_eq!(
                unsafe {
                    execute_high_native::<0>(
                        (&raw mut incoming).cast(),
                        &raw mut frame,
                        result_ptr,
                        registry.table.as_ptr().cast(),
                    )
                },
                status
            );
            let expected_code = unsafe {
                if redirect {
                    redirected.as_mut_ptr().add(1)
                } else {
                    code.as_mut_ptr().add(2)
                }
            };
            assert_eq!(incoming.calls, 2);
            assert_eq!(frame_object.calls, 0);
            assert_eq!(incoming.outer_code, unsafe { code.as_mut_ptr().add(1) });
            assert_eq!(incoming.observed_frame, (&raw mut frame).cast());
            assert_eq!(incoming.observed_result, result_ptr);
            assert_eq!(incoming.observed_code, expected_code);
            assert_eq!(frame.code, expected_code);
            assert_eq!(frame.object, frame_object_ptr);
            assert_eq!(code, [0x60, 0xff, 0x5a]);
            assert_eq!(redirected, [0xff, 0x7c]);
            assert_eq!(result, [0x1122_3344, 0x5566_7788, 0x99aa_bbcc]);
            assert!(is_handler(registry.table[0xfff], observe_dispatch));
            assert!(is_handler(registry.table[0xffe], UNDEFINED));
        }
    }
}
