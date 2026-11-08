use hp2_core::ansi::Tchar;
use hp2_abi::engine::generated::{FFrame, TArrayRaw};
use hp2_core::array_storage::{self, Hp2ArrayServices, Hp2ArraySlots};
use hp2_core::vm_registry::{Hp2VmNativeHandler, NATIVE_COUNT};
use hp2_core::vm_string::{self, Hp2VmStringView};
use hp2_core::vm_flow;
use std::{collections::BTreeMap, ffi::c_void, ptr};

static EMPTY: [Tchar; 1] = [0];

fn empty() -> Hp2VmStringView {
    Hp2VmStringView {
        data: EMPTY.as_ptr().cast(),
        num: 0,
    }
}

fn view(chars: &[Tchar]) -> Hp2VmStringView {
    assert_eq!(chars.last(), Some(&0));
    Hp2VmStringView {
        data: chars.as_ptr().cast(),
        num: chars.len() as i32,
    }
}

fn wide(text: &str) -> Vec<Tchar> {
    text.chars().map(|c| c as Tchar).chain([0]).collect()
}

#[derive(Default)]
struct Heap {
    buffers: BTreeMap<usize, Vec<Tchar>>,
    allocations: Vec<i32>,
    releases: Vec<usize>,
    fail_allocate: i32,
    fail_check: i32,
    fail_free: i32,
    arguments: Vec<*mut TArrayRaw>,
    argument_data: Vec<usize>,
    fail_argument: usize,
    debug_status: i32,
    debug_character: Option<Tchar>,
    contexts: Vec<*mut c_void>,
}

fn arrays(heap: *mut Heap) -> Hp2ArrayServices {
    Hp2ArrayServices {
        context: heap.cast(),
        realloc: reallocate,
        checked_int_size: checked,
        free: release,
    }
}

unsafe extern "C" fn reallocate(
    context: *mut c_void,
    old: *mut c_void,
    bytes: usize,
    _: *const i32,
    width: i32,
    _: *const c_void,
    out: *mut *mut c_void,
) -> i32 {
    unsafe {
        let heap = &mut *context.cast::<Heap>();
        assert_eq!(width as usize, size_of::<Tchar>());
        let count = bytes / size_of::<Tchar>();
        heap.allocations.push(count as i32);
        if heap.fail_allocate != 0 {
            return heap.fail_allocate;
        }
        let mut next = vec![0x7777 as Tchar; count];
        if !old.is_null() {
            let previous = heap.buffers.remove(&(old as usize)).unwrap();
            let copy = count.min(previous.len());
            next[..copy].copy_from_slice(&previous[..copy]);
        }
        let data = if count == 0 {
            ptr::null_mut()
        } else {
            let data = next.as_mut_ptr().cast();
            heap.buffers.insert(data as usize, next);
            data
        };
        out.write(data);
        0
    }
}

unsafe extern "C" fn checked(context: *mut c_void, bytes: usize, out: *mut i32) -> i32 {
    unsafe {
        let heap = &*context.cast::<Heap>();
        if heap.fail_check != 0 {
            return heap.fail_check;
        }
        match i32::try_from(bytes) {
            Ok(bytes) => { out.write(bytes); 0 }
            Err(_) => 97,
        }
    }
}

unsafe extern "C" fn release(context: *mut c_void, data: *mut c_void) -> i32 {
    unsafe {
        let heap = &mut *context.cast::<Heap>();
        heap.releases.push(data as usize);
        if heap.fail_free != 0 {
            return heap.fail_free;
        }
        assert!(heap.buffers.remove(&(data as usize)).is_some());
        0
    }
}

fn raw_empty() -> TArrayRaw {
    TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }
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

unsafe fn contents(value: *const TArrayRaw) -> Vec<Tchar> {
    unsafe {
        let num = ptr::addr_of!((*value).num).read();
        if num == 0 {
            Vec::new()
        } else {
            let data = ptr::addr_of!((*value).data).read_unaligned();
            std::slice::from_raw_parts(data.cast(), num as usize).to_vec()
        }
    }
}

struct Storage {
    chars: Vec<Tchar>,
}

fn run(op: u32, a: Hp2VmStringView, b: Hp2VmStringView, index: i32, count: i32) -> Storage {
    let mut heap = Heap::default();
    let mut result = raw_empty();
    unsafe {
        vm_string::string(op, a, b, index, count, &raw mut result, arrays(&raw mut heap)).unwrap();
        let chars = contents(&raw const result);
        array_storage::release(slots(&raw mut result), arrays(&raw mut heap)).unwrap();
        assert!(heap.buffers.is_empty());
        Storage { chars }
    }
}

fn scalar(op: u32, a: Hp2VmStringView, b: Hp2VmStringView) -> i32 {
    let mut result = -99;
    unsafe { vm_string::scalar(op, a, b, &mut result) };
    result
}

#[test]
fn concatenation_uses_left_array_count_but_right_terminated_prefix() {
    let a = [b'a' as Tchar, 0, b'z' as Tchar, 0];
    let b = [b'b' as Tchar, 0, b'x' as Tchar, 0];
    assert_eq!(
        run(112, view(&a), view(&b), 0, 0).chars,
        [b'a' as Tchar, 0, b'z' as Tchar, b'b' as Tchar, 0]
    );
    assert_eq!(
        run(168, view(&a), view(&b), 0, 0).chars,
        [
            b'a' as Tchar,
            0,
            b'z' as Tchar,
            b' ' as Tchar,
            b'b' as Tchar,
            0
        ]
    );
    assert!(run(112, empty(), empty(), 0, 0).chars.is_empty());
    assert_eq!(run(168, empty(), empty(), 0, 0).chars, wide(" "));
    // Num==1 is a distinct empty storage representation; copying the left
    // operand preserves it whereas appending such a right operand does not.
    assert_eq!(run(112, view(&EMPTY), empty(), 0, 0).chars, [0]);
    assert!(run(112, empty(), view(&EMPTY), 0, 0).chars.is_empty());
}

#[test]
fn mid_calculates_end_before_unsigned_start_and_end_clamps() {
    let input = wide("abcdef");
    let a = view(&input);
    let cases = [
        (1, 3, "bcd"),
        (-1, 3, ""),
        (i32::MIN, i32::MAX, ""),
        (2, -1, ""),
        (2, -3, "cdef"),
        (0, -1, "abcdef"),
        (1, i32::MAX, "bcdef"),
        (6, 65535, ""),
        (99, 2, ""),
        (2, 65535, "cdef"),
    ];
    for (start, count, expected) in cases {
        let result = run(127, a, empty(), start, count);
        let expected = if expected.is_empty() {
            Vec::new()
        } else {
            wide(expected)
        };
        assert_eq!(result.chars, expected, "start={start}, count={count}");
    }
}

#[test]
fn counted_substrings_pad_nuls_while_right_scans_from_clamped_offset() {
    let input = [b'a' as Tchar, 0, b'c' as Tchar, b'd' as Tchar, 0];
    let a = view(&input);
    assert_eq!(
        run(128, a, empty(), 0, 4).chars,
        [b'a' as Tchar, 0, 0, 0, 0]
    );
    assert_eq!(run(127, a, empty(), 1, 3).chars, [0, 0, 0, 0]);
    assert_eq!(run(234, a, empty(), 0, 4).chars, wide("a"));
    assert!(run(234, a, empty(), 0, 3).chars.is_empty());
    assert_eq!(run(234, a, empty(), 0, 2).chars, wide("cd"));
    for count in [i32::MIN, -1, 0] {
        assert!(run(128, a, empty(), 0, count).chars.is_empty());
        assert!(run(234, a, empty(), 0, count).chars.is_empty());
    }
    let normal = wide("abcdef");
    assert_eq!(run(128, view(&normal), empty(), 0, i32::MAX).chars, normal);
    assert_eq!(run(234, view(&normal), empty(), 0, i32::MAX).chars, normal);
}

#[test]
fn caps_is_ascii_only_and_chr_asc_preserve_all_32_bits() {
    let input = wide("azAZéß𐐨");
    assert_eq!(run(235, view(&input), empty(), 0, 0).chars, wide("AZAZéß𐐨"));
    assert!(run(235, empty(), empty(), 0, 0).chars.is_empty());
    for code in [
        0,
        65,
        0xe9,
        0x1f600,
        0xd800,
        0x110000,
        -1,
        i32::MIN,
        i32::MAX,
    ] {
        let output = run(236, empty(), empty(), code, 0);
        if code == 0 {
            assert!(output.chars.is_empty());
            assert_eq!(scalar(237, empty(), empty()), 0);
        } else {
            assert_eq!(output.chars, [code as Tchar, 0]);
            assert_eq!(scalar(237, view(&output.chars), empty()), code);
        }
    }
}

#[test]
fn length_uses_array_count_search_uses_utf32_offsets_and_nul_termination() {
    let input = wide("a😀é😀");
    let needle = wide("é😀");
    let absent = wide("😀a");
    assert_eq!(scalar(125, view(&input), empty()), 4);
    assert_eq!(scalar(126, view(&input), view(&needle)), 2);
    assert_eq!(scalar(126, view(&input), view(&absent)), -1);
    assert_eq!(scalar(126, view(&input), empty()), 0);
    assert_eq!(scalar(126, empty(), empty()), 0);
    assert_eq!(scalar(126, empty(), view(&needle)), -1);
    let embedded = [b'a' as Tchar, 0, b'b' as Tchar, 0];
    let b = wide("b");
    assert_eq!(scalar(125, view(&embedded), empty()), 3);
    assert_eq!(scalar(126, view(&embedded), view(&b)), -1);
    assert_eq!(scalar(125, empty(), empty()), 0);
    assert_eq!(scalar(125, view(&EMPTY), empty()), 0);
}

#[test]
fn comparisons_apply_sign_and_case_rules_before_publishing_aliased_result() {
    let a = wide("alpha");
    let b = wide("beta");
    for (op, less, same, greater) in [
        (115, 1, 0, 0),
        (116, 0, 0, 1),
        (120, 1, 1, 0),
        (121, 0, 1, 1),
        (122, 0, 1, 0),
        (123, 1, 0, 1),
    ] {
        assert_eq!(scalar(op, view(&a), view(&b)), less);
        assert_eq!(scalar(op, view(&a), view(&a)), same);
        assert_eq!(scalar(op, view(&b), view(&a)), greater);
    }
    let upper = wide("ALPHA");
    assert_eq!(scalar(122, view(&a), view(&upper)), 0);
    assert_eq!(scalar(124, view(&a), view(&upper)), 1);
    let mut alias = wide("alpha");
    unsafe { vm_string::scalar(122, view(&alias), view(&a), alias.as_mut_ptr().cast()) };
    assert_eq!(alias[0], 1);
}

#[test]
fn non_ascii_case_equality_tracks_the_current_host_locale() {
    unsafe extern "C" {
        fn wcscasecmp(a: *const Tchar, b: *const Tchar) -> i32;
    }
    // Do not mutate the process locale while other contracts are running. The
    // operator must use the host's current locale, not Unicode/Rust lowercasing.
    for (a, b) in [("é", "É"), ("ı", "I"), ("ß", "SS"), ("𐐨", "𐐀")] {
        let a = wide(a);
        let b = wide(b);
        let expected = i32::from(unsafe { wcscasecmp(a.as_ptr(), b.as_ptr()) } == 0);
        assert_eq!(scalar(124, view(&a), view(&b)), expected);
    }
}

#[test]
fn names_and_objects_compare_full_identity_including_null() {
    assert_eq!(vm_string::identity(254, 0, 0), 1);
    assert_eq!(vm_string::identity(255, 7, 9), 1);
    assert_eq!(vm_string::identity(254, 7, 9), 0);
    assert_eq!(vm_string::identity(255, 9, 9), 0);
    let low = 0x1234usize;
    let high = low | (1usize << 40);
    assert_eq!(vm_string::identity(114, low, high), 0);
    assert_eq!(vm_string::identity(119, low, high), 1);
    assert_eq!(vm_string::identity(114, 0, 0), 1);
    assert_eq!(vm_string::identity(119, high, high), 0);
}

#[test]
fn first_resize_error_preserves_partial_metadata_without_publishing_characters() {
    let a = wide("abc");
    let b = wide("def");
    for (op, index, count, requested) in [
        (112, 0, 0, 7),
        (168, 0, 0, 8),
        (127, 1, 1, 2),
        (128, 0, 2, 3),
        (234, 0, 1, 2),
        (235, 0, 0, 4),
        (236, 65, 0, 2),
        (236, 0, 0, 0),
    ] {
        let mut heap = Heap::default();
        let mut storage = raw_empty();
        let original = [0x7777 as Tchar; 15].into_iter().chain([0]).collect::<Vec<_>>();
        unsafe {
            vm_string::string(112, view(&original), empty(), 0, 0, &raw mut storage, arrays(&raw mut heap)).unwrap();
        }
        heap.allocations.clear();
        heap.fail_allocate = 73;
        let status = unsafe {
            vm_string::string(
                op,
                view(&a),
                view(&b),
                index,
                count,
                &raw mut storage,
                arrays(&raw mut heap),
            )
        };
        assert_eq!(status, Err(73));
        assert_eq!(heap.allocations, [requested]);
        let num = storage.num;
        let max = storage.max;
        assert_eq!(num, requested);
        assert_eq!(max, requested);
        let data = storage.data;
        assert_eq!(heap.buffers[&(data as usize)], original);
        unsafe { array_storage::release(slots(&raw mut storage), arrays(&raw mut heap)).unwrap() };
    }
}

#[test]
fn decoded_value_copies_allow_destination_to_be_the_original_argument() {
    let mut heap = Heap::default();
    let mut destination = raw_empty();
    let initial = wide("same");
    unsafe {
        vm_string::string(112, view(&initial), empty(), 0, 0, &raw mut destination, arrays(&raw mut heap)).unwrap();
    }
    // The kernel receives the owning value copies retained by checked execute.
    let a = unsafe { contents(&raw const destination) };
    let b = a.clone();
    unsafe {
        vm_string::string(
            112,
            view(&a),
            view(&b),
            0,
            0,
            &raw mut destination,
            arrays(&raw mut heap),
        )
        .unwrap();
    }
    assert_eq!(unsafe { contents(&raw const destination) }, wide("samesame"));
    unsafe { array_storage::release(slots(&raw mut destination), arrays(&raw mut heap)).unwrap() };
    assert_eq!(a, wide("same"));
    assert_eq!(b, wide("same"));
}

unsafe extern "C" fn unexpected(_: *mut c_void, _: *mut c_void, _: *mut c_void) -> i32 {
    99
}

unsafe extern "C" fn literal(
    object: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        let frame = frame.cast::<FFrame>();
        let heap = ptr::addr_of!((*frame).locals).read().cast::<Heap>();
        (*heap).contexts.push(object);
        (*heap).arguments.push(result.cast());
        let status = vm_flow::execute_string_constant(
            false,
            ptr::addr_of_mut!((*frame).code),
            result.cast(),
            arrays(heap),
        );
        if let Err(status) = status {
            return status;
        }
        let data = ptr::addr_of!((*result.cast::<TArrayRaw>()).data).read_unaligned();
        (*heap).argument_data.push(data as usize);
        if (*heap).fail_argument == (*heap).arguments.len() { 41 } else { 0 }
    }
}

unsafe extern "C" fn debug(
    _: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        assert!(result.is_null());
        let frame = frame.cast::<FFrame>();
        let heap = ptr::addr_of!((*frame).locals).read().cast::<Heap>();
        assert!((*heap).releases.is_empty());
        if let Some(character) = (*heap).debug_character {
            let argument = (&(*heap).arguments)[0];
            let data = ptr::addr_of!((*argument).data).read_unaligned();
            data.cast::<Tchar>().write(character);
        }
        // As with the native debug expression, this consumes its own operand.
        let code = ptr::addr_of_mut!((*frame).code);
        code.write(code.read().add(1));
        (*heap).debug_status
    }
}

fn table() -> [Hp2VmNativeHandler; NATIVE_COUNT] {
    let mut table = [unexpected as Hp2VmNativeHandler; NATIVE_COUNT];
    table[0x1f] = literal;
    table[0x38] = debug;
    table
}

fn frame(code: &mut [u8], heap: *mut Heap) -> FFrame {
    FFrame {
        node: ptr::null_mut(),
        object: 1usize as *mut c_void,
        code: code.as_mut_ptr(),
        locals: heap.cast(),
        parent: ptr::null_mut(),
    }
}

#[test]
fn checked_strings_destroy_every_initialized_argument_in_reverse_on_each_error_path() {
    // Failure after first/second argument, during debug, and during cleanup.
    // Failed frees retain their buffer in Heap so later cleanup remains visible.
    for (fail_argument, debug_status, fail_free, expected, arguments, cursor) in [
        (1, 0, 67, 41, 1, 3),
        (2, 0, 67, 41, 2, 6),
        (0, 53, 67, 53, 2, 9),
        (0, 0, 67, 67, 2, 9),
    ] {
        let mut heap = Heap { fail_argument, debug_status, fail_free, ..Heap::default() };
        let mut code = [0x1f, b'a', 0, 0x1f, b'b', 0, 0x16, 0x38, 0, 0x42];
        let mut frame = frame(&mut code, &raw mut heap);
        let mut result = -1_i32;
        let status = unsafe {
            vm_string::execute::<115>(
                &raw mut frame,
                (&raw mut result).cast(),
                table().as_ptr(),
                arrays(&raw mut heap),
            )
        };
        assert_eq!(status, Err(expected));
        assert_eq!(heap.arguments.len(), arguments);
        assert_eq!(heap.releases, heap.argument_data.iter().rev().copied().collect::<Vec<_>>());
        assert_eq!(result, if fail_argument == 0 && debug_status == 0 { 1 } else { -1 });
        let actual_cursor = frame.code;
        assert_eq!(actual_cursor, unsafe { code.as_mut_ptr().add(cursor) });
    }
}

#[test]
fn checked_strings_read_live_owning_arguments_after_debug_and_cleanup_after_resize_failure() {
    for fail_output in [false, true] {
        let mut heap = Heap { debug_character: Some(b'z' as Tchar), ..Heap::default() };
        let mut code = [0x1f, b'a', 0, 0x1f, b'b', 0, 0x16, 0x38, 0, 0x42];
        let mut frame = frame(&mut code, &raw mut heap);
        let mut result = raw_empty();
        let mut handlers = table();
        if fail_output {
            handlers[0x38] = debug_fail_output;
        }
        let status = unsafe {
            vm_string::execute::<112>(
                &raw mut frame,
                (&raw mut result).cast(),
                handlers.as_ptr(),
                arrays(&raw mut heap),
            )
        };
        assert_eq!(status, if fail_output { Err(73) } else { Ok(()) });
        assert_eq!(heap.releases, heap.argument_data.iter().rev().copied().collect::<Vec<_>>());
        if fail_output {
            let data = result.data;
            let num = result.num;
            assert!(data.is_null());
            assert_eq!(num, 3);
        } else {
            assert_eq!(unsafe { contents(&raw const result) }, wide("zb"));
        }
        unsafe { array_storage::release(slots(&raw mut result), arrays(&raw mut heap)).unwrap() };
        assert!(heap.buffers.is_empty());
    }
}

unsafe extern "C" fn debug_fail_output(
    object: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        let status = debug(object, frame, result);
        let heap = ptr::addr_of!((*frame.cast::<FFrame>()).locals).read().cast::<Heap>();
        (*heap).fail_allocate = 73;
        status
    }
}

#[test]
fn eat_string_uses_explicit_context_keeps_following_debug_unconsumed_and_propagates_cleanup() {
    for (fail_argument, fail_free, expected) in [(0, 0, Ok(())), (0, 67, Err(67)), (1, 67, Err(41))] {
        let mut heap = Heap { fail_argument, fail_free, ..Heap::default() };
        let mut code = [0x1f, b'a', 0, 0x38, 0];
        let mut frame = frame(&mut code, &raw mut heap);
        let object = 2usize as *mut c_void;
        let status = unsafe {
            vm_string::eat(object, &raw mut frame, table().as_ptr(), arrays(&raw mut heap))
        };
        assert_eq!(status, expected);
        assert_eq!(heap.contexts, [object]);
        assert_eq!(heap.releases, heap.argument_data);
        let cursor = frame.code;
        assert_eq!(cursor, unsafe { code.as_mut_ptr().add(3) });
    }
}

#[derive(Default)]
struct IdentityOperands {
    objects: bool,
    values: [usize; 2],
    slots: Vec<*mut c_void>,
    contexts: Vec<*mut c_void>,
    omit: bool,
    fail_argument: usize,
    debug_status: i32,
    debug_equal: bool,
}

unsafe extern "C" fn identity_operand(
    object: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        let frame = frame.cast::<FFrame>();
        let state = ptr::addr_of!((*frame).locals).read().cast::<IdentityOperands>();
        let index = (*state).slots.len();
        (*state).slots.push(result);
        (*state).contexts.push(object);
        if (*state).objects {
            assert!(result.cast::<*mut c_void>().read().is_null());
            if !(*state).omit {
                result.cast::<*mut c_void>().write((*state).values[index] as *mut c_void);
            }
        } else {
            assert_eq!(result.cast::<i32>().read(), 0);
            if !(*state).omit {
                result.cast::<i32>().write((*state).values[index] as i32);
            }
        }
        // Subsequent operands and debug must re-read Stack.Object.
        ptr::addr_of_mut!((*frame).object).write(2usize as *mut c_void);
        if (*state).fail_argument == index + 1 { 41 } else { 0 }
    }
}

unsafe extern "C" fn identity_debug(
    object: *mut c_void,
    frame: *mut c_void,
    result: *mut c_void,
) -> i32 {
    unsafe {
        assert!(result.is_null());
        let frame = frame.cast::<FFrame>();
        let state = ptr::addr_of!((*frame).locals).read().cast::<IdentityOperands>();
        (*state).contexts.push(object);
        if (*state).debug_equal {
            let a = (&(*state).slots)[0];
            let b = (&(*state).slots)[1];
            if (*state).objects {
                b.cast::<*mut c_void>().write(a.cast::<*mut c_void>().read());
            } else {
                b.cast::<i32>().write(a.cast::<i32>().read());
            }
        }
        let code = ptr::addr_of_mut!((*frame).code);
        code.write(code.read().add(1));
        (*state).debug_status
    }
}

fn run_identity<const OP: u32>(
    state: &mut IdentityOperands,
) -> (Result<(), i32>, [u32; 3], usize) {
    let mut table = [unexpected as Hp2VmNativeHandler; NATIVE_COUNT];
    table[1] = identity_operand;
    table[0x38] = identity_debug;
    let mut code = [1, 1, 0x16, 0x38, 0, 0x42];
    let mut frame = FFrame {
        node: ptr::null_mut(),
        object: 1usize as *mut c_void,
        code: code.as_mut_ptr(),
        locals: (state as *mut IdentityOperands).cast(),
        parent: ptr::null_mut(),
    };
    let mut result = [0xface_cafe, u32::MAX, 0xfeed_beef];
    let status = unsafe {
        vm_string::execute_identity::<OP>(&raw mut frame, result.as_mut_ptr().add(1), table.as_ptr())
    };
    let code_end = frame.code;
    (status, result, unsafe { code_end.offset_from(code.as_ptr()) as usize })
}

#[test]
fn checked_identity_preserves_name_bits_pointer_width_and_dword_result_width() {
    for (objects, values) in [
        (false, [0x8000_0000, 0x7fff_ffff]),
        (false, [u32::MAX as usize, u32::MAX as usize]),
        (true, [0x1234, 0x1234 | (1usize << 40)]),
        (true, [0, 0]),
    ] {
        for unequal in [false, true] {
            let mut state = IdentityOperands { objects, values, ..Default::default() };
            let (status, result, cursor) = match (objects, unequal) {
                (false, false) => run_identity::<254>(&mut state),
                (false, true) => run_identity::<255>(&mut state),
                (true, false) => run_identity::<114>(&mut state),
                (true, true) => run_identity::<119>(&mut state),
            };
            assert_eq!(status, Ok(()));
            assert_eq!(result, [0xface_cafe, u32::from((values[0] != values[1]) == unequal), 0xfeed_beef]);
            assert_eq!(cursor, 5);
            assert_eq!(state.contexts, [1usize as *mut c_void, 2usize as *mut c_void, 2usize as *mut c_void]);
        }
    }
}

#[test]
fn checked_identity_keeps_optional_defaults_and_reads_debug_mutated_operands() {
    for objects in [false, true] {
        for omit in [false, true] {
            let mut state = IdentityOperands {
                objects, values: [7, 9], omit, debug_equal: !omit, ..Default::default()
            };
            let (status, result, cursor) = if objects {
                run_identity::<114>(&mut state)
            } else {
                run_identity::<254>(&mut state)
            };
            assert_eq!(status, Ok(()));
            assert_eq!(result, [0xface_cafe, 1, 0xfeed_beef]);
            assert_eq!(cursor, 5);
        }
    }
}

#[test]
fn checked_identity_stops_on_first_argument_or_debug_error_without_publishing() {
    for objects in [false, true] {
        for (fail_argument, debug_status, expected, calls, cursor) in [
            (1, 53, 41, 1, 1),
            (2, 53, 41, 2, 2),
            (0, 53, 53, 3, 5),
        ] {
            let mut state = IdentityOperands {
                objects, values: [7, 9], fail_argument, debug_status, ..Default::default()
            };
            let (status, result, actual_cursor) = if objects {
                run_identity::<119>(&mut state)
            } else {
                run_identity::<255>(&mut state)
            };
            assert_eq!(status, Err(expected));
            assert_eq!(result, [0xface_cafe, u32::MAX, 0xfeed_beef]);
            assert_eq!(state.contexts.len(), calls);
            assert_eq!(actual_cursor, cursor);
        }
    }
}
