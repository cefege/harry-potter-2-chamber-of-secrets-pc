use hp2_abi::FVector;
use hp2_abi::engine::generated::{UClass, UObject};
use hp2_core::{
    ansi::{Hp2AnsiServices, Tchar},
    math::FRotator,
    vm_convert::*,
};
use std::{cell::RefCell, ffi::c_void, ptr};
type Object = *mut c_void;
thread_local! { static ANSI_STATE: RefCell<([u8;512],usize,usize)> = const { RefCell::new(([0;512],0,0)) }; }
unsafe extern "C" fn fail_assert(_: *const c_void, _: *const c_void, _: i32) -> i32 {
    1
}
unsafe extern "C" fn size_error() -> i32 {
    1
}
unsafe extern "C" fn to_ansi(text: *const c_void, out: *mut *const c_void) -> i32 {
    ANSI_STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.1 += 1;
        if state.2 == state.1 {
            return 1;
        }
        unsafe {
            let mut p = text.cast::<Tchar>();
            let mut i = 0;
            while p.read() != 0 && i < 511 {
                state.0[i] = p.read() as u8;
                i += 1;
                p = p.add(1);
            }
            state.0[i] = 0;
            out.write(state.0.as_ptr().cast());
        }
        0
    })
}
static ANSI: Hp2AnsiServices = Hp2AnsiServices {
    fail_assert,
    to_ansi,
    size_exceeds_int_contract: size_error,
};
fn wide(text: &str) -> Vec<Tchar> {
    text.chars().map(|c| c as Tchar).chain([0]).collect()
}
// `UObject` is exactly 72 bytes (ending at `class`); `UClass::super_field`
// sits at that same offset 72, so a struct embedding `UObject` directly
// followed by a `super_field` pointer reproduces the real layout with no
// extra padding, letting one fixture serve both the `IsA` (reads `.object`)
// and `IsChildOf` (reads `.super_field`, since every `UClass` is also a
// `UObject`) walks that `cast()` now runs directly, no FFI leaf involved.
const _: () = assert!(size_of::<UObject>() == 72);
const _: () = assert!(std::mem::offset_of!(UClass, super_field) == 72);
#[repr(C)]
struct FakeClass {
    object: UObject,
    super_field: *mut c_void,
}
fn new_fake() -> FakeClass {
    FakeClass {
        object: unsafe { std::mem::zeroed() },
        super_field: ptr::null_mut(),
    }
}
thread_local! { static IS_CLASS_STATE: std::cell::Cell<(bool, i32)> = const { std::cell::Cell::new((false, 0)) }; }
unsafe extern "C" fn is_class(_: Object, out: *mut i32) -> i32 {
    let (matches, fail) = IS_CLASS_STATE.with(std::cell::Cell::get);
    if fail != 0 {
        return fail;
    }
    unsafe {
        out.write(i32::from(matches));
    }
    0
}
fn services() -> Hp2VmConvertServices {
    Hp2VmConvertServices { is_class }
}
#[test]
fn object_and_metaclass_casts_follow_hierarchy_and_do_not_publish_after_errors() {
    unsafe {
        let s = services();
        let mut root = new_fake();
        let mut derived = new_fake();
        derived.super_field = (&raw mut root).cast();
        let mut node = new_fake();
        node.object.class = (&raw mut derived).cast();
        node.super_field = (&raw mut derived).cast();
        let object: Object = (&raw mut node).cast();
        let root_class: Object = (&raw mut root).cast();
        let mut result = ptr::null_mut();
        cast(object, root_class, false, &mut result, &s).unwrap();
        assert_eq!(result, object);
        cast(object, root_class, true, &mut result, &s).unwrap();
        assert!(result.is_null());
        IS_CLASS_STATE.with(|c| c.set((true, 0)));
        cast(object, root_class, true, &mut result, &s).unwrap();
        assert_eq!(result, object);
        IS_CLASS_STATE.with(|c| c.set((false, 77)));
        assert_eq!(cast(object, root_class, true, &mut result, &s), Err(77));
        assert_eq!(result, object);
        cast(ptr::null_mut(), ptr::null_mut(), true, &mut result, &s).unwrap();
        assert!(result.is_null());
    }
}

mod dynamic_and_meta_cast {
    use super::*;
    use hp2_abi::engine::generated::FFrame;
    use hp2_core::object_storage::{self, Hp2ObjectStorageServices, ObjectStorage};
    use hp2_core::vm_registry::{Hp2VmNativeHandler, NATIVE_COUNT};

    unsafe extern "C" fn storage_check(kind: i32, condition: i32) -> i32 {
        if condition != 0 { 0 } else { 90 + kind }
    }

    thread_local! {
        static CAST_EVENTS: RefCell<Vec<(&'static str, usize, usize)>> = RefCell::new(Vec::new());
    }

    unsafe extern "C" fn log_is_class(object: Object, out: *mut i32) -> i32 {
        unsafe {
            CAST_EVENTS.with(|e| e.borrow_mut().push(("is_class", object as usize, 0)));
            out.write(1);
        }
        0
    }
    fn log_services() -> Hp2VmConvertServices {
        Hp2VmConvertServices {
            is_class: log_is_class,
        }
    }

    // The literal class operand is a four-byte object-table index (EX_ObjectConst),
    // never a raw pointer; reading eight bytes here would both resolve the wrong
    // class and desync the cursor before the castee operand is stepped.
    #[repr(C)]
    struct CastInvocation {
        frame: FFrame,
        code: [u8; 5],
        table: [Hp2VmNativeHandler; NATIVE_COUNT],
        castee: Object,
        events: Vec<usize>,
    }

    impl CastInvocation {
        fn new(class_index: i32, castee: Object) -> Self {
            let mut code = [0u8; 5];
            code[..4].copy_from_slice(&class_index.to_ne_bytes());
            code[4] = 0x80;
            Self {
                frame: FFrame {
                    node: ptr::null_mut(),
                    object: ptr::null_mut(),
                    code: ptr::null_mut(),
                    locals: ptr::null_mut(),
                    parent: ptr::null_mut(),
                },
                code,
                table: [cast_unexpected; NATIVE_COUNT],
                castee,
                events: Vec::new(),
            }
        }

        fn prepare(&mut self) {
            self.frame.code = self.code.as_mut_ptr();
            self.table[0x80] = cast_operand;
        }
    }

    unsafe extern "C" fn cast_unexpected(_: Object, _: Object, _: Object) -> i32 {
        99
    }
    // Records how many bytecode bytes are live-consumed by the time the castee
    // operand steps: four for the object-table index plus one for this opcode,
    // proving the class is decoded before step_argument ever runs.
    unsafe extern "C" fn cast_operand(_: Object, frame: Object, result: Object) -> i32 {
        unsafe {
            let p = frame.cast::<CastInvocation>();
            let consumed = (*p).frame.code.offset_from((*p).code.as_mut_ptr());
            (*p).events.push(consumed as usize);
            result.cast::<Object>().write((*p).castee);
            0
        }
    }

    #[test]
    fn execute_dynamic_cast_resolves_the_class_through_the_object_table_then_steps_the_castee() {
        unsafe {
            let mut storage = ObjectStorage::new();
            let services = Hp2ObjectStorageServices {
                check: storage_check,
            };
            let mut first: UObject = std::mem::zeroed();
            let mut second: UObject = std::mem::zeroed();
            let first = (&raw mut first).cast::<c_void>();
            let second = (&raw mut second).cast::<c_void>();
            object_storage::add(&raw mut storage, first, -1, &services).unwrap();
            object_storage::add(&raw mut storage, second, -1, &services).unwrap();

            CAST_EVENTS.with(|e| e.borrow_mut().clear());
            // The castee's class chain reaches `second` (object-table slot 1)
            // directly and never touches `first` (slot 0); index 1 (not 0)
            // proves the live operand bytes select the slot, not a hardcoded
            // first entry.
            let mut castee_fake = new_fake();
            castee_fake.object.class = second;
            let castee: Object = (&raw mut castee_fake).cast();
            let mut invocation = CastInvocation::new(1, castee);
            invocation.prepare();
            let p = &raw mut invocation;
            let mut result: Object = 0x1234_5678usize as Object;
            let convert = log_services();
            let status = execute_dynamic_cast(
                ptr::addr_of_mut!((*p).frame),
                (&raw mut result).cast(),
                ptr::addr_of!((*p).table).cast(),
                &raw const storage,
                &convert,
            );
            assert_eq!(status, Ok(()));
            assert_eq!(invocation.events, [5]);
            assert_eq!(result, castee);
            // `is_a` no longer crosses into services at all; confirm no leaf fired.
            CAST_EVENTS.with(|e| assert!(e.borrow().is_empty()));
        }
    }

    #[test]
    fn execute_meta_cast_resolves_the_class_through_the_object_table_then_steps_the_castee() {
        unsafe {
            let mut storage = ObjectStorage::new();
            let services = Hp2ObjectStorageServices {
                check: storage_check,
            };
            let mut first: UObject = std::mem::zeroed();
            let mut second: UObject = std::mem::zeroed();
            let first = (&raw mut first).cast::<c_void>();
            let second = (&raw mut second).cast::<c_void>();
            object_storage::add(&raw mut storage, first, -1, &services).unwrap();
            object_storage::add(&raw mut storage, second, -1, &services).unwrap();

            CAST_EVENTS.with(|e| e.borrow_mut().clear());
            // The castee's super-field chain reaches `first` (object-table
            // slot 0) directly and never touches `second` (slot 1); index 0
            // here (versus index 1 above) again proves the operand bytes
            // drive resolution, and that dynamic/meta cast differ only in
            // the meta bool.
            let mut castee_fake = new_fake();
            castee_fake.super_field = first;
            let castee: Object = (&raw mut castee_fake).cast();
            let mut invocation = CastInvocation::new(0, castee);
            invocation.prepare();
            let p = &raw mut invocation;
            let mut result: Object = 0x1234_5678usize as Object;
            let convert = log_services();
            let status = execute_meta_cast(
                ptr::addr_of_mut!((*p).frame),
                (&raw mut result).cast(),
                ptr::addr_of!((*p).table).cast(),
                &raw const storage,
                &convert,
            );
            assert_eq!(status, Ok(()));
            assert_eq!(invocation.events, [5]);
            assert_eq!(result, castee);
            CAST_EVENTS.with(|e| {
                assert_eq!(e.borrow().as_slice(), [("is_class", castee as usize, 0)]);
            });
        }
    }
}

mod checked_expressions {
    use super::*;
    use hp2_abi::engine::generated::FFrame;
    use hp2_core::vm_registry::{Hp2VmNativeHandler, NATIVE_COUNT};

    type Handler = unsafe fn(*mut FFrame, Object, *const Hp2VmNativeHandler) -> Result<(), i32>;

    const HANDLERS: [(Handler, usize); 18] = [
        (execute_byte_to_int, 4),
        (execute_byte_to_bool, 4),
        (execute_byte_to_float, 4),
        (execute_int_to_byte, 1),
        (execute_int_to_bool, 4),
        (execute_int_to_float, 4),
        (execute_bool_to_byte, 1),
        (execute_bool_to_int, 4),
        (execute_bool_to_float, 4),
        (execute_float_to_byte, 1),
        (execute_float_to_int, 4),
        (execute_float_to_bool, 4),
        (execute_object_to_bool, 4),
        (execute_name_to_bool, 4),
        (execute_vector_to_bool, 4),
        (execute_vector_to_rotator, 12),
        (execute_rotator_to_bool, 4),
        (execute_rotator_to_vector, 12),
    ];

    #[repr(C)]
    struct Invocation {
        frame: FFrame,
        code: [u8; 4],
        redirected: [u8; 4],
        table: [Hp2VmNativeHandler; NATIVE_COUNT],
        events: Vec<usize>,
        bytes: [u8; 4],
        offset: usize,
        count: usize,
        status: i32,
        outer_slot: *mut i32,
        nested_result: u32,
    }

    impl Invocation {
        fn new(opcode: u8) -> Self {
            Self {
                frame: FFrame {
                    node: ptr::null_mut(),
                    object: 1usize as Object,
                    code: ptr::null_mut(),
                    locals: ptr::null_mut(),
                    parent: ptr::null_mut(),
                },
                code: [opcode, 0x16, 0x38, 0x42],
                redirected: [0x80, 0x16, 0x38, 0x42],
                table: [unexpected; NATIVE_COUNT],
                events: Vec::new(),
                bytes: [0; 4],
                offset: 0,
                count: 0,
                status: 0,
                outer_slot: ptr::null_mut(),
                nested_result: 0xdead_beef,
            }
        }

        fn prepare(&mut self) {
            self.frame.code = self.code.as_mut_ptr();
            self.table[0x16] = omitted;
            self.table[0x80] = partial;
        }
    }

    unsafe extern "C" fn unexpected(_: Object, _: Object, _: Object) -> i32 {
        99
    }

    unsafe extern "C" fn omitted(object: Object, frame: Object, _: Object) -> i32 {
        unsafe {
            let p = frame.cast::<Invocation>();
            (*p).events.push(object as usize);
            (*p).frame.code = (*p).frame.code.sub(1);
        }
        0
    }

    unsafe extern "C" fn partial(object: Object, frame: Object, result: Object) -> i32 {
        unsafe {
            let p = frame.cast::<Invocation>();
            (*p).events.push(object as usize);
            ptr::copy_nonoverlapping(
                ptr::addr_of!((*p).bytes).cast::<u8>(),
                result.cast::<u8>().add((*p).offset),
                (*p).count,
            );
            (*p).status
        }
    }

    unsafe extern "C" fn reenter(object: Object, frame: Object, result: Object) -> i32 {
        unsafe {
            let p = frame.cast::<Invocation>();
            (*p).events.push(object as usize);
            (*p).outer_slot = result.cast();
            (*p).outer_slot.write(0);
            (*p).frame.object = 2usize as Object;
            (*p).frame.code = ptr::addr_of_mut!((*p).redirected).cast();
            ptr::addr_of_mut!((*p).table)
                .cast::<Hp2VmNativeHandler>()
                .add(0x80)
                .write(nested);
            execute_int_to_float(
                frame.cast(),
                ptr::addr_of_mut!((*p).nested_result).cast(),
                ptr::addr_of!((*p).table).cast(),
            )
            .err()
            .unwrap_or(0)
        }
    }

    unsafe extern "C" fn nested(object: Object, frame: Object, result: Object) -> i32 {
        unsafe {
            let p = frame.cast::<Invocation>();
            (*p).events.push(object as usize);
            result.cast::<i32>().write(16_777_217);
            (*p).outer_slot.write(-257);
            (*p).frame.object = 3usize as Object;
            (*p).status
        }
    }

    unsafe fn word(handler: Handler, bytes: &[u8]) -> u32 {
        unsafe {
            let mut invocation = Invocation::new(0x80);
            invocation.bytes[..bytes.len()].copy_from_slice(bytes);
            invocation.count = bytes.len();
            invocation.prepare();
            let p = &raw mut invocation;
            let mut output = 0xa5a5_a5a5_u32;
            handler(
                ptr::addr_of_mut!((*p).frame),
                (&raw mut output).cast(),
                ptr::addr_of!((*p).table).cast(),
            )
            .unwrap();
            output
        }
    }

    #[test]
    fn scalar_narrowing_preserves_destination_width_and_boolean_low_bit() {
        unsafe {
            assert_eq!(
                word(execute_int_to_byte, &(-257i32).to_ne_bytes()),
                0xa5a5a5ff
            );
            assert_eq!(word(execute_bool_to_byte, &2u32.to_ne_bytes()), 0xa5a5a500);
            assert_eq!(word(execute_bool_to_int, &2u32.to_ne_bytes()), 0);
            assert_eq!(
                word(execute_bool_to_float, &3u32.to_ne_bytes()),
                1.0f32.to_bits()
            );
            assert_eq!(word(execute_int_to_bool, &i32::MIN.to_ne_bytes()), 1);
            assert_eq!(word(execute_byte_to_int, &[255u8]), 255);
            assert_eq!(
                word(execute_int_to_float, &16_777_217i32.to_ne_bytes()),
                16_777_216f32.to_bits()
            );
        }
    }

    #[test]
    fn arm64_float_narrowing_and_nonfinite_boolean_boundaries() {
        unsafe {
            for (value, expected) in [
                (-1.0f32, 255),
                (255.9, 255),
                (256.0, 0),
                (511.0, 255),
                (2147483648.0, 255),
                (f32::INFINITY, 255),
                (f32::NEG_INFINITY, 0),
                (f32::NAN, 0),
            ] {
                assert_eq!(
                    word(execute_float_to_byte, &value.to_ne_bytes()) & 255,
                    expected
                );
            }
            assert_eq!(
                word(execute_float_to_int, &f32::INFINITY.to_ne_bytes()),
                i32::MAX as u32
            );
            assert_eq!(
                word(execute_float_to_int, &f32::NEG_INFINITY.to_ne_bytes()),
                i32::MIN as u32
            );
            assert_eq!(word(execute_float_to_int, &f32::NAN.to_ne_bytes()), 0);
            assert_eq!(word(execute_float_to_bool, &(-0.0f32).to_ne_bytes()), 0);
            assert_eq!(word(execute_float_to_bool, &f32::NAN.to_ne_bytes()), 1);
        }
    }

    #[test]
    fn rotations_use_low_words_for_boolean_but_vector_to_rotator_matches_real_atan2() {
        unsafe extern "C" fn rotator_operand(_: Object, _: Object, result: Object) -> i32 {
            unsafe {
                result.cast::<FRotator>().write(FRotator {
                    pitch: 65536,
                    yaw: -65536,
                    roll: 0,
                });
            }
            0
        }
        unsafe extern "C" fn vector_bool_zero(_: Object, _: Object, result: Object) -> i32 {
            unsafe {
                result.cast::<FVector>().write(FVector {
                    x: -0.0,
                    y: 0.0,
                    z: 0.0,
                });
            }
            0
        }
        unsafe extern "C" fn vector_bool_nan(_: Object, _: Object, result: Object) -> i32 {
            unsafe {
                result.cast::<FVector>().write(FVector {
                    x: 0.0,
                    y: f32::NAN,
                    z: 0.0,
                });
            }
            0
        }
        unsafe extern "C" fn vector_to_rotator_operand(
            _: Object,
            _: Object,
            result: Object,
        ) -> i32 {
            unsafe {
                result.cast::<FVector>().write(FVector {
                    x: 0.0,
                    y: 1.0,
                    z: 0.0,
                });
            }
            0
        }
        unsafe fn call(handler: Handler, operand: Hp2VmNativeHandler) -> [u32; 3] {
            unsafe {
                let mut code = [0x80u8, 0x16, 0x38, 0x42];
                let mut table = [unexpected as Hp2VmNativeHandler; NATIVE_COUNT];
                table[0x80] = operand;
                let mut frame = FFrame {
                    node: ptr::null_mut(),
                    object: ptr::null_mut(),
                    code: code.as_mut_ptr(),
                    locals: ptr::null_mut(),
                    parent: ptr::null_mut(),
                };
                let mut output = [0xa5a5_a5a5_u32; 3];
                handler(&raw mut frame, output.as_mut_ptr().cast(), table.as_ptr()).unwrap();
                output
            }
        }
        unsafe {
            assert_eq!(call(execute_rotator_to_bool, rotator_operand)[0], 0);
            assert_eq!(call(execute_vector_to_bool, vector_bool_zero)[0], 0);
            assert_eq!(call(execute_vector_to_bool, vector_bool_nan)[0], 1);
            let rotator = call(execute_vector_to_rotator, vector_to_rotator_operand);
            assert_eq!(
                [rotator[0] as i32, rotator[1] as i32, rotator[2] as i32],
                [0, 16383, 0]
            );
        }
    }

    #[test]
    fn omitted_expressions_preserve_end_marker_and_publish_only_the_result_width() {
        for (handler, width) in HANDLERS {
            let mut invocation = Invocation::new(0x16);
            invocation.prepare();
            let p = &raw mut invocation;
            let mut output = [0xa5a5_a5a5_u32; 5];
            assert_eq!(
                unsafe {
                    handler(
                        ptr::addr_of_mut!((*p).frame),
                        output.as_mut_ptr().add(1).cast(),
                        ptr::addr_of!((*p).table).cast(),
                    )
                },
                Ok(())
            );
            let bytes: Vec<_> = output.into_iter().flat_map(u32::to_ne_bytes).collect();
            assert_eq!(&bytes[..4], &[0xa5; 4]);
            // A null rotation is the identity, so RotatorToVector alone publishes
            // the unit X axis rather than an all-zero vector for an omitted operand.
            let mut expected = vec![0u8; width];
            if std::ptr::fn_addr_eq(handler, execute_rotator_to_vector as Handler) {
                expected[..4].copy_from_slice(&1.0_f32.to_ne_bytes());
            }
            assert_eq!(&bytes[4..4 + width], expected.as_slice());
            assert!(bytes[4 + width..].iter().all(|byte| *byte == 0xa5));
            assert_eq!(invocation.events, [1]);
            assert_eq!(invocation.frame.code, invocation.code.as_mut_ptr());
        }
    }

    #[test]
    fn partial_operands_keep_untouched_zero_components_without_consuming_finish_or_debug() {
        let cases: [(Handler, usize, [u8; 4], usize, [u32; 3]); 5] = [
            (
                execute_int_to_bool,
                0,
                [1, 0, 0, 0],
                1,
                [1, 0xa5a5_a5a5, 0xa5a5_a5a5],
            ),
            (
                execute_float_to_bool,
                0,
                (-0.0_f32).to_ne_bytes(),
                4,
                [0, 0xa5a5_a5a5, 0xa5a5_a5a5],
            ),
            (
                execute_vector_to_bool,
                8,
                1_u32.to_ne_bytes(),
                4,
                [1, 0xa5a5_a5a5, 0xa5a5_a5a5],
            ),
            (
                execute_vector_to_rotator,
                4,
                1.0_f32.to_ne_bytes(),
                4,
                [0, 16383, 0],
            ),
            (
                execute_rotator_to_bool,
                0,
                65536_i32.to_ne_bytes(),
                4,
                [0, 0xa5a5_a5a5, 0xa5a5_a5a5],
            ),
        ];
        for (handler, offset, bytes, count, expected) in cases {
            let mut invocation = Invocation::new(0x80);
            invocation.offset = offset;
            invocation.bytes = bytes;
            invocation.count = count;
            invocation.prepare();
            let p = &raw mut invocation;
            let mut output = [0xa5a5_a5a5_u32; 5];
            assert_eq!(
                unsafe {
                    handler(
                        ptr::addr_of_mut!((*p).frame),
                        output.as_mut_ptr().add(1).cast(),
                        ptr::addr_of!((*p).table).cast(),
                    )
                },
                Ok(())
            );
            assert_eq!(
                output,
                [
                    0xa5a5_a5a5,
                    expected[0],
                    expected[1],
                    expected[2],
                    0xa5a5_a5a5
                ]
            );
            assert_eq!(invocation.events, [1]);
            assert_eq!(invocation.frame.code, unsafe {
                invocation.code.as_mut_ptr().add(1)
            });
        }
    }

    #[test]
    fn live_cursor_object_and_table_reentry_keep_the_outer_operand_slot_stable() {
        for status in [0, -73, 91] {
            let mut invocation = Invocation::new(0x80);
            invocation.prepare();
            invocation.table[0x80] = reenter;
            invocation.status = status;
            let p = &raw mut invocation;
            let mut output = [0xa5_u8; 3];
            assert_eq!(
                unsafe {
                    execute_int_to_byte(
                        ptr::addr_of_mut!((*p).frame),
                        output.as_mut_ptr().add(1).cast(),
                        ptr::addr_of!((*p).table).cast(),
                    )
                },
                if status == 0 { Ok(()) } else { Err(status) }
            );
            assert_eq!(output, [0xa5, if status == 0 { 255 } else { 0xa5 }, 0xa5]);
            assert_eq!(
                invocation.nested_result,
                if status == 0 {
                    16_777_216_f32.to_bits()
                } else {
                    0xdead_beef
                }
            );
            assert_eq!(invocation.events, [1, 2]);
            assert_eq!(invocation.frame.object as usize, 3);
            assert_eq!(invocation.frame.code, unsafe {
                invocation.redirected.as_mut_ptr().add(1)
            });
        }
    }

    #[test]
    fn first_operand_errors_leave_all_conversion_results_unpublished() {
        for (handler, _) in HANDLERS {
            for status in [-37, 83] {
                let mut invocation = Invocation::new(0x80);
                invocation.bytes[0] = 0xff;
                invocation.count = 1;
                invocation.status = status;
                invocation.prepare();
                let p = &raw mut invocation;
                let mut output = [0xdead_beef_u32; 5];
                assert_eq!(
                    unsafe {
                        handler(
                            ptr::addr_of_mut!((*p).frame),
                            output.as_mut_ptr().add(1).cast(),
                            ptr::addr_of!((*p).table).cast(),
                        )
                    },
                    Err(status)
                );
                assert_eq!(output, [0xdead_beef; 5]);
                assert_eq!(invocation.events, [1]);
                assert_eq!(invocation.frame.code, unsafe {
                    invocation.code.as_mut_ptr().add(1)
                });
            }
        }
    }

    #[test]
    fn rotator_to_vector_matches_the_canonical_math_conversion() {
        unsafe extern "C" fn operand(_: Object, _: Object, result: Object) -> i32 {
            unsafe {
                result.cast::<FRotator>().write(FRotator {
                    pitch: 1000,
                    yaw: 20000,
                    roll: -5000,
                });
            }
            0
        }
        let mut code = [0x80, 0x16];
        let mut table = [unexpected as Hp2VmNativeHandler; NATIVE_COUNT];
        table[0x80] = operand;
        let mut frame = FFrame {
            node: ptr::null_mut(),
            object: ptr::null_mut(),
            code: code.as_mut_ptr(),
            locals: ptr::null_mut(),
            parent: ptr::null_mut(),
        };
        let mut result = FVector {
            x: 9.0,
            y: 9.0,
            z: 9.0,
        };
        unsafe {
            execute_rotator_to_vector(&raw mut frame, (&raw mut result).cast(), table.as_ptr())
                .unwrap();
        }
        let expected = hp2_core::math::rotator_vector(
            FRotator {
                pitch: 1000,
                yaw: 20000,
                roll: -5000,
            },
            hp2_core::math::trig_table(),
        );
        assert_eq!(
            [result.x, result.y, result.z],
            [expected.x, expected.y, expected.z]
        );
        assert_eq!(frame.code, unsafe { code.as_mut_ptr().add(1) });
    }
}

mod checked_text {
    use super::*;
    use hp2_abi::engine::generated::{FFrame, TArrayRaw};
    use hp2_core::{
        ansi::{self, PublishedAnsi},
        array_storage::{self, Hp2ArrayServices, Hp2ArraySlots},
        string_storage,
        vm_registry::{Hp2VmNativeHandler, NATIVE_COUNT},
    };

    // Localization is actual mutable process state, so tests using it serialize.
    static LOCALE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    unsafe extern "C" {
        fn realloc(data: Object, bytes: usize) -> Object;
        fn free(data: Object);
    }

    struct State {
        text: Vec<Tchar>,
        operand_status: i32,
        cleanup_status: i32,
        frees: usize,
        allocations: usize,
        fail_allocation: usize,
        parse_calls: usize,
        parse_status: i32,
        outer: *mut TArrayRaw,
        reenter: bool,
        nested: u32,
        mutate_locale: bool,
        scalar: u32,
        vector: FVector,
        rotator: FRotator,
    }

    impl State {
        fn new(text: &str) -> Self {
            Self {
                text: wide(text),
                operand_status: 0,
                cleanup_status: 0,
                frees: 0,
                allocations: 0,
                parse_calls: 0,
                parse_status: 0,
                fail_allocation: 0,
                outer: ptr::null_mut(),
                reenter: false,
                nested: 0,
                mutate_locale: false,
                scalar: 0,
                vector: FVector {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                rotator: FRotator {
                    pitch: 0,
                    yaw: 0,
                    roll: 0,
                },
            }
        }
    }

    unsafe fn slots(value: *mut TArrayRaw) -> Hp2ArraySlots {
        unsafe {
            Hp2ArraySlots {
                data: ptr::addr_of_mut!((*value).data),
                num: ptr::addr_of_mut!((*value).num),
                max: ptr::addr_of_mut!((*value).max),
            }
        }
    }

    unsafe extern "C" fn allocate(
        context: Object,
        data: Object,
        bytes: usize,
        _: *const i32,
        _: i32,
        _: *const c_void,
        out: *mut Object,
    ) -> i32 {
        unsafe {
            (*context.cast::<State>()).allocations += 1;
            if (*context.cast::<State>()).allocations == (*context.cast::<State>()).fail_allocation
            {
                return -53;
            }
            let next = if bytes == 0 {
                free(data);
                ptr::null_mut()
            } else {
                realloc(data, bytes)
            };
            if bytes != 0 && next.is_null() {
                return 81;
            }
            out.write(next);
        }
        0
    }

    unsafe extern "C" fn checked(_: Object, bytes: usize, out: *mut i32) -> i32 {
        match i32::try_from(bytes) {
            Ok(value) => {
                unsafe {
                    out.write(value);
                }
                0
            }
            Err(_) => 82,
        }
    }

    unsafe extern "C" fn release(context: Object, data: Object) -> i32 {
        unsafe {
            let state = context.cast::<State>();
            free(data);
            (*state).frees += 1;
            (*state).cleanup_status
        }
    }

    fn arrays(state: *mut State) -> Hp2ArrayServices {
        Hp2ArrayServices {
            context: state.cast(),
            realloc: allocate,
            checked_int_size: checked,
            free: release,
        }
    }

    unsafe extern "C" fn operand(object: Object, _: Object, result: Object) -> i32 {
        unsafe {
            let state = object.cast::<State>();
            (*state).outer = result.cast();
            let status = string_storage::assign_with_allocator(
                slots(result.cast()),
                None,
                (*state).text.as_ptr(),
                false,
                arrays(state),
                ptr::null(),
            )
            .err()
            .unwrap_or(0);
            if (*state).mutate_locale {
                (&raw mut GTrue).write({
                    let mut text = [0; 64];
                    text[..6].copy_from_slice(&[70, 65, 76, 83, 69, 0]);
                    text
                });
            }
            if status != 0 {
                status
            } else {
                (*state).operand_status
            }
        }
    }

    unsafe extern "C" fn scalar_operand(object: Object, _: Object, result: Object) -> i32 {
        unsafe {
            result.cast::<u32>().write((*object.cast::<State>()).scalar);
        }
        0
    }

    unsafe extern "C" fn vector_operand(object: Object, _: Object, result: Object) -> i32 {
        unsafe {
            result
                .cast::<FVector>()
                .write((*object.cast::<State>()).vector);
        }
        0
    }

    unsafe extern "C" fn rotator_operand(object: Object, _: Object, result: Object) -> i32 {
        unsafe {
            result
                .cast::<FRotator>()
                .write((*object.cast::<State>()).rotator);
        }
        0
    }

    unsafe extern "C" fn unexpected(_: Object, _: Object, _: Object) -> i32 {
        99
    }

    fn publication() -> PublishedAnsi {
        PublishedAnsi {
            services: ANSI,
            int_parse_file: c"conversion-fixture".as_ptr().cast(),
            int_parse_line: 1,
        }
    }

    type Handler = unsafe fn(
        *mut FFrame,
        Object,
        *const Hp2VmNativeHandler,
        Hp2ArrayServices,
        PublishedAnsi,
    ) -> Result<(), i32>;

    unsafe fn run(
        state: *mut State,
        handler: Handler,
        output: Object,
        ansi: PublishedAnsi,
    ) -> Result<(), i32> {
        let mut code = [0x80, 0x16, 0x38];
        let mut table = [unexpected as Hp2VmNativeHandler; NATIVE_COUNT];
        table[0x80] = operand;
        let mut frame = FFrame {
            node: ptr::null_mut(),
            object: state.cast(),
            code: code.as_mut_ptr(),
            locals: ptr::null_mut(),
            parent: ptr::null_mut(),
        };
        let status =
            unsafe { handler(&raw mut frame, output, table.as_ptr(), arrays(state), ansi) };
        assert_eq!(frame.code, unsafe { code.as_mut_ptr().add(1) });
        status
    }

    #[test]
    fn integer_prefix_width_range_and_owning_cleanup_match_the_vm_contract() {
        for (text, expected) in [("  -257suffix", -257), ("+17e7", 17), ("0xFF", 0)] {
            let mut state = State::new(text);
            let mut output = 0;
            unsafe {
                run(
                    &raw mut state,
                    execute_string_to_int,
                    (&raw mut output).cast(),
                    publication(),
                )
                .unwrap();
            }
            assert_eq!(output, expected);
            assert_eq!((state.allocations, state.frees), (1, 1));
        }
        let mut state = State::new("-257");
        let mut output = [0xa5; 3];
        unsafe {
            run(
                &raw mut state,
                execute_string_to_byte,
                output.as_mut_ptr().add(1).cast(),
                publication(),
            )
            .unwrap();
        }
        assert_eq!(output, [0xa5, 255, 0xa5]);
        let mut state = State::new("2147483648");
        let mut output = 73_i32;
        assert_eq!(
            unsafe {
                run(
                    &raw mut state,
                    execute_string_to_int,
                    (&raw mut output).cast(),
                    publication(),
                )
            },
            Err(1)
        );
        assert_eq!((output, state.frees), (73, 1));
    }

    #[test]
    fn localized_true_mutated_during_operand_precedes_english_false() {
        let _lock = LOCALE_LOCK.lock().unwrap();
        unsafe {
            let saved = (&raw const GTrue).read();
            let mut state = State::new("false");
            state.mutate_locale = true;
            let mut output = -1_i32;
            let status = run(
                &raw mut state,
                execute_string_to_bool,
                (&raw mut output).cast(),
                publication(),
            );
            (&raw mut GTrue).write(saved);
            assert_eq!(status, Ok(()));
            assert_eq!(output, 1);

            let mut code = [0x80, 0x16, 0x38];
            let mut table = [unexpected as Hp2VmNativeHandler; NATIVE_COUNT];
            table[0x80] = scalar_operand;
            state.scalar = 2; // Nonzero, unlike bool-to-int's low-bit policy.
            let mut frame = FFrame {
                node: ptr::null_mut(),
                object: (&raw mut state).cast(),
                code: code.as_mut_ptr(),
                locals: ptr::null_mut(),
                parent: ptr::null_mut(),
            };
            let mut text = TArrayRaw {
                data: ptr::null_mut(),
                num: 0,
                max: 0,
            };
            execute_bool_to_string(
                &raw mut frame,
                (&raw mut text).cast(),
                table.as_ptr(),
                arrays(&raw mut state),
            )
            .unwrap();
            assert_eq!(ansi::strcmp(text.data.cast(), (&raw const GTrue).cast()), 0);
            assert_eq!(frame.code, code.as_mut_ptr().add(1));
            array_storage::release(slots(&raw mut text), arrays(&raw mut state)).unwrap();
        }
    }

    thread_local! { static ACTIVE: std::cell::Cell<*mut State> = const { std::cell::Cell::new(ptr::null_mut()) }; }

    unsafe extern "C" fn parse_ansi(input: *const c_void, output: *mut *const c_void) -> i32 {
        unsafe {
            let state = ACTIVE.with(|active| active.get());
            (*state).parse_calls += 1;
            if (*state).reenter {
                (*state).reenter = false;
                let mut code = [0x80, 0x16];
                let mut table = [unexpected as Hp2VmNativeHandler; NATIVE_COUNT];
                table[0x80] = nested_operand;
                let mut frame = FFrame {
                    node: ptr::null_mut(),
                    object: state.cast(),
                    code: code.as_mut_ptr(),
                    locals: ptr::null_mut(),
                    parent: ptr::null_mut(),
                };
                let status = execute_int_to_float(
                    &raw mut frame,
                    ptr::addr_of_mut!((*state).nested).cast(),
                    table.as_ptr(),
                );
                if let Err(status) = status {
                    return status;
                }
            }
            if (*state).parse_status != 0 {
                return (*state).parse_status;
            }
            output.write(ansi::to_ansi_string(input.cast()).cast());
        }
        0
    }

    unsafe extern "C" fn nested_operand(object: Object, _: Object, result: Object) -> i32 {
        unsafe {
            let state = object.cast::<State>();
            // Reentry changes the outer operand's live owned buffer.
            let text = (*(*state).outer).data.cast::<Tchar>();
            ptr::copy_nonoverlapping([50, 46, 53, 0].as_ptr(), text, 4);
            result.cast::<i32>().write(16_777_217);
        }
        0
    }

    #[test]
    fn parsing_reentry_observes_live_owned_operand_and_preserves_nested_result() {
        let _lock = LOCALE_LOCK.lock().unwrap();
        let mut state = State::new("1.0");
        state.reenter = true;
        let mut ansi = publication();
        ansi.services.to_ansi = parse_ansi;
        ACTIVE.with(|active| active.set(&raw mut state));
        let mut output = 0_u32;
        let status = unsafe {
            run(
                &raw mut state,
                execute_string_to_float,
                (&raw mut output).cast(),
                ansi,
            )
        };
        ACTIVE.with(|active| active.set(ptr::null_mut()));
        assert_eq!(status, Ok(()));
        assert_eq!(output, 2.5_f32.to_bits());
        assert_eq!(state.nested, 16_777_216_f32.to_bits());
        assert_eq!((state.parse_calls, state.frees), (1, 1));
    }

    #[test]
    fn operand_and_parser_failures_cleanup_without_masking_the_first_error() {
        let _lock = LOCALE_LOCK.lock().unwrap();
        for (operand_status, parse_status, cleanup_status, expected, calls) in
            [(-37, 0, 91, -37, 0), (0, 1, 91, 1, 1), (0, 0, 91, 91, 1)]
        {
            let mut state = State::new("2.5");
            state.operand_status = operand_status;
            state.parse_status = parse_status;
            state.cleanup_status = cleanup_status;
            let mut ansi = publication();
            ansi.services.to_ansi = parse_ansi;
            ACTIVE.with(|active| active.set(&raw mut state));
            let mut output = 0xdead_beef_u32;
            let status = unsafe {
                run(
                    &raw mut state,
                    execute_string_to_float,
                    (&raw mut output).cast(),
                    ansi,
                )
            };
            ACTIVE.with(|active| active.set(ptr::null_mut()));
            assert_eq!(status, Err(expected));
            assert_eq!((state.parse_calls, state.frees), (calls, 1));
            assert_eq!(
                output,
                if operand_status == 0 && parse_status == 0 {
                    2.5_f32.to_bits()
                } else {
                    0xdead_beef
                }
            );
        }
    }

    #[test]
    fn formatted_temporary_cleanup_preserves_assignment_error_or_published_result() {
        for fail_allocation in [0, 2] {
            unsafe {
                let mut state = State::new("");
                state.scalar = i32::MIN as u32;
                state.fail_allocation = fail_allocation;
                state.cleanup_status = 91;
                let mut code = [0x80, 0x16, 0x38];
                let mut table = [unexpected as Hp2VmNativeHandler; NATIVE_COUNT];
                table[0x80] = scalar_operand;
                let mut frame = FFrame {
                    node: ptr::null_mut(),
                    object: (&raw mut state).cast(),
                    code: code.as_mut_ptr(),
                    locals: ptr::null_mut(),
                    parent: ptr::null_mut(),
                };
                let mut output = TArrayRaw {
                    data: ptr::null_mut(),
                    num: 0,
                    max: 0,
                };
                let status = execute_int_to_string(
                    &raw mut frame,
                    (&raw mut output).cast(),
                    table.as_ptr(),
                    arrays(&raw mut state),
                );
                assert_eq!(status, Err(if fail_allocation == 0 { 91 } else { -53 }));
                assert_eq!((state.allocations, state.frees), (2, 1));
                assert_eq!(frame.code, code.as_mut_ptr().add(1));
                if fail_allocation == 0 {
                    assert_eq!(
                        ansi::strcmp(output.data.cast(), wide("-2147483648").as_ptr()),
                        0
                    );
                } else {
                    assert!(output.data.is_null());
                }
                state.cleanup_status = 0;
                array_storage::release(slots(&raw mut output), arrays(&raw mut state)).unwrap();
            }
        }
    }

    #[test]
    fn string_to_vector_uses_comma_prefixes_and_zeroes_missing_fields() {
        for (text, expected) in [
            ("12.5", [12.5, 0.0, 0.0]),
            ("1,,3,99", [1.0, 0.0, 3.0]),
            ("1junk, -2.25,3tail", [1.0, -2.25, 3.0]),
        ] {
            let mut state = State::new(text);
            let mut output = FVector {
                x: 9.0,
                y: 9.0,
                z: 9.0,
            };
            unsafe {
                run(
                    &raw mut state,
                    execute_string_to_vector,
                    (&raw mut output).cast(),
                    publication(),
                )
                .unwrap();
            }
            assert_eq!([output.x, output.y, output.z], expected);
            assert_eq!((state.allocations, state.frees), (1, 1));
        }
    }

    #[test]
    fn string_to_rotator_uses_comma_prefixes_and_stops_after_three_components() {
        let mut state = State::new("-1,,65537,4");
        let mut output = FRotator {
            pitch: 9,
            yaw: 9,
            roll: 9,
        };
        unsafe {
            run(
                &raw mut state,
                execute_string_to_rotator,
                (&raw mut output).cast(),
                publication(),
            )
            .unwrap();
        }
        assert_eq!([output.pitch, output.yaw, output.roll], [-1, 0, 65537]);
        assert_eq!((state.allocations, state.frees), (1, 1));
    }

    #[test]
    fn string_to_vector_component_failure_neither_publishes_nor_parses_the_suffix() {
        let mut state = State::new("10,20,30");
        ANSI_STATE.with(|cell| cell.borrow_mut().2 = 2);
        let mut output = FVector {
            x: 4.0,
            y: 5.0,
            z: 6.0,
        };
        let status = unsafe {
            run(
                &raw mut state,
                execute_string_to_vector,
                (&raw mut output).cast(),
                publication(),
            )
        };
        assert_eq!(status, Err(1));
        assert_eq!([output.x, output.y, output.z], [4.0, 5.0, 6.0]);
        ANSI_STATE.with(|cell| assert_eq!(cell.borrow().1, 2));
    }

    #[test]
    fn string_to_rotator_atoi_overflow_leaves_result_unpublished() {
        let mut state = State::new("10,2147483648,30");
        let mut output = FRotator {
            pitch: 4,
            yaw: 5,
            roll: 6,
        };
        let status = unsafe {
            run(
                &raw mut state,
                execute_string_to_rotator,
                (&raw mut output).cast(),
                publication(),
            )
        };
        assert_eq!(status, Err(1));
        assert_eq!([output.pitch, output.yaw, output.roll], [4, 5, 6]);
    }

    #[test]
    fn vector_to_string_formats_each_component_with_format_float() {
        unsafe {
            let mut state = State::new("");
            state.vector = FVector {
                x: -1.0,
                y: 0.0,
                z: 65537.5,
            };
            let mut code = [0x80, 0x16, 0x38];
            let mut table = [unexpected as Hp2VmNativeHandler; NATIVE_COUNT];
            table[0x80] = vector_operand;
            let mut frame = FFrame {
                node: ptr::null_mut(),
                object: (&raw mut state).cast(),
                code: code.as_mut_ptr(),
                locals: ptr::null_mut(),
                parent: ptr::null_mut(),
            };
            let mut output = TArrayRaw {
                data: ptr::null_mut(),
                num: 0,
                max: 0,
            };
            execute_vector_to_string(
                &raw mut frame,
                (&raw mut output).cast(),
                table.as_ptr(),
                arrays(&raw mut state),
            )
            .unwrap();
            assert_eq!(
                ansi::strcmp(
                    output.data.cast(),
                    wide("-1.000000,0.000000,65537.500000").as_ptr()
                ),
                0
            );
            assert_eq!(frame.code, code.as_mut_ptr().add(1));
            array_storage::release(slots(&raw mut output), arrays(&raw mut state)).unwrap();
        }
    }

    #[test]
    fn rotator_to_string_normalizes_to_low_words_before_formatting() {
        unsafe {
            let mut state = State::new("");
            state.rotator = FRotator {
                pitch: -1,
                yaw: 65537,
                roll: i32::MIN,
            };
            let mut code = [0x80, 0x16, 0x38];
            let mut table = [unexpected as Hp2VmNativeHandler; NATIVE_COUNT];
            table[0x80] = rotator_operand;
            let mut frame = FFrame {
                node: ptr::null_mut(),
                object: (&raw mut state).cast(),
                code: code.as_mut_ptr(),
                locals: ptr::null_mut(),
                parent: ptr::null_mut(),
            };
            let mut output = TArrayRaw {
                data: ptr::null_mut(),
                num: 0,
                max: 0,
            };
            execute_rotator_to_string(
                &raw mut frame,
                (&raw mut output).cast(),
                table.as_ptr(),
                arrays(&raw mut state),
            )
            .unwrap();
            assert_eq!(
                ansi::strcmp(output.data.cast(), wide("65535,1,0").as_ptr()),
                0
            );
            assert_eq!(frame.code, code.as_mut_ptr().add(1));
            array_storage::release(slots(&raw mut output), arrays(&raw mut state)).unwrap();
        }
    }
}
