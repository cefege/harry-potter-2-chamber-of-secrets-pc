use hp2_abi::engine::generated::TArrayRaw;
use hp2_core::{
    array_storage::{self, Hp2ArrayServices},
    runtime_archive,
    vm_flow::{Hp2VmFlowOp, Hp2VmFlowOp::*, Hp2VmFlowServices, execute, execute_string_constant},
};
use std::{cell::Cell, ffi::c_void, mem::size_of, ptr, sync::Mutex};

static LITERAL_RING: Mutex<()> = Mutex::new(());
type Object = *mut c_void;
#[repr(C)]
#[derive(Clone, Copy)]
struct Text {
    data: *const u8,
    len: usize,
}
#[derive(Debug, PartialEq)]
struct Event(&'static str, usize, Vec<i32>);
struct Frame {
    script: [u8; 256],
    code: *mut u8,
    events: Vec<Event>,
    fail: &'static str,
    fail_occurrence: usize,
    occurrences: usize,
    assign_redirect: Option<usize>,
    branch_redirect: Option<usize>,
    debugger_live: bool,
    no_node: bool,
    object: u64,
    label: i32,
}
thread_local! { static ACTIVE: Cell<*mut Frame> = const { Cell::new(ptr::null_mut()) }; }
impl Frame {
    fn new(bytes: &[u8]) -> Box<Self> {
        let mut f = Box::new(Self {
            script: [0; 256],
            code: ptr::null_mut(),
            events: Vec::new(),
            fail: "",
            fail_occurrence: 1,
            occurrences: 0,
            assign_redirect: None,
            branch_redirect: None,
            debugger_live: false,
            no_node: false,
            object: 0x1234,
            label: 7,
        });
        f.script[..bytes.len()].copy_from_slice(bytes);
        f.code = f.script.as_mut_ptr();
        f
    }
    fn position(&self) -> usize {
        self.code as usize - self.script.as_ptr() as usize
    }
    fn names(&self) -> Vec<&'static str> {
        self.events.iter().map(|e| e.0).collect()
    }
    unsafe fn run(
        &mut self,
        op: Hp2VmFlowOp,
        result: Object,
        first: Object,
        second: Object,
    ) -> Result<(), i32> {
        let p = self as *mut Self;
        let previous = ACTIVE.with(|a| a.replace(p));
        let answer = unsafe {
            execute(
                op,
                p.cast(),
                p.cast(),
                &raw mut (*p).code,
                result,
                first,
                second,
                &services(),
            )
        };
        ACTIVE.with(|a| a.set(previous));
        answer
    }
}
unsafe fn active() -> *mut Frame {
    ACTIVE.with(Cell::get)
}
unsafe fn event(name: &'static str, values: Vec<i32>) -> i32 {
    unsafe {
        let f = &mut *active();
        f.events.push(Event(name, f.position(), values));
        if f.fail == name {
            f.occurrences += 1;
            if f.occurrences == f.fail_occurrence {
                return 73;
            }
        }
        0
    }
}
unsafe extern "C" fn step(frame: Object, result: Object) -> i32 {
    unsafe {
        let f = frame.cast::<Frame>();
        let status = event("step", vec![]);
        if status != 0 {
            return status;
        }
        let token = (*f).code.read();
        (*f).code = (*f).code.add(1);
        // Switch/Case fixtures supply their own Text-valued expression service;
        // direct FString literal policy is exercised separately below.
        if token == 3 {
            let status = assign_string(result, (*f).code, 0);
            if status == 0 {
                while (*f).code.read() != 0 {
                    (*f).code = (*f).code.add(1);
                }
                (*f).code = (*f).code.add(1);
            }
            return status;
        }
        let op = match token {
            1 => VmFlowIntConst,
            2 => VmFlowByteConst,
            4 => VmFlowEndFunctionParms,
            _ => return 91,
        };
        execute(
            op,
            frame,
            frame,
            &raw mut (*f).code,
            result,
            ptr::null_mut(),
            ptr::null_mut(),
            &services(),
        )
        .err()
        .unwrap_or(0)
    }
}
unsafe extern "C" fn script_base(frame: Object, out: *mut *mut u8) -> i32 {
    unsafe {
        let f = frame.cast::<Frame>();
        let status = event("base", vec![]);
        if status == 0 {
            out.write(if (*f).no_node {
                ptr::null_mut()
            } else {
                (*f).script.as_mut_ptr()
            });
        }
        status
    }
}
unsafe extern "C" fn script_at(frame: Object, offset: u32, out: *mut *mut u8) -> i32 {
    unsafe {
        let status = event("jump", vec![offset as i32]);
        if status == 0 {
            out.write(
                (*frame.cast::<Frame>())
                    .script
                    .as_mut_ptr()
                    .add(offset as usize),
            );
        }
        status
    }
}
unsafe extern "C" fn bad_case() -> i32 {
    unsafe {
        event("bad_case", vec![]);
    }
    92
}
unsafe extern "C" fn branch(
    _: Object,
    _: Object,
    opcode: i32,
    target: i32,
    next: i32,
    value: i32,
) -> i32 {
    unsafe {
        let status = event("branch", vec![opcode, target, next, value]);
        let f = active();
        if let Some(offset) = (*f).branch_redirect {
            (*f).code = (*f).script.as_mut_ptr().add(offset);
        }
        status
    }
}
unsafe extern "C" fn assertion(_: Object, line: i32) -> i32 {
    unsafe { event("assert", vec![line]) }
}
unsafe extern "C" fn goto_label(_: Object, name: i32, out: *mut i32) -> i32 {
    unsafe {
        let status = event("label", vec![name]);
        if status == 0 {
            out.write(i32::from(name == (*active()).label));
        }
        status
    }
}
unsafe extern "C" fn missing_label(_: Object, name: i32) -> i32 {
    unsafe { event("missing", vec![name]) }
}
unsafe extern "C" fn assign_string(result: Object, text: *const u8, unicode: u32) -> i32 {
    unsafe {
        let status = event("assign", vec![unicode as i32]);
        if status != 0 {
            return status;
        }
        let width = if unicode == 0 { 1 } else { 2 };
        let mut len = 0;
        while if width == 1 {
            text.add(len).read() != 0
        } else {
            text.add(len).cast::<u16>().read_unaligned() != 0
        } {
            len += width;
        }
        result.cast::<Text>().write(Text { data: text, len });
        let f = active();
        if let Some(offset) = (*f).assign_redirect {
            (*f).code = (*f).script.as_mut_ptr().add(offset);
        }
        0
    }
}
unsafe extern "C" fn strings_equal(a: Object, b: Object, out: *mut i32) -> i32 {
    unsafe {
        let status = event("equal", vec![]);
        if status != 0 {
            return status;
        }
        let a = a.cast::<Text>().read();
        let b = b.cast::<Text>().read();
        let equal =
            a.len == b.len && (0..a.len).all(|i| a.data.add(i).read() == b.data.add(i).read());
        out.write(i32::from(equal));
        0
    }
}
unsafe extern "C" fn destroy_string(text: Object) -> i32 {
    unsafe {
        let status = event("destroy", vec![text.cast::<Text>().read().len as i32]);
        if status == 0 {
            text.cast::<Text>().write(Text {
                data: ptr::null(),
                len: 0,
            });
        }
        status
    }
}
unsafe extern "C" fn is_return(text: Object, out: *mut i32) -> i32 {
    unsafe {
        let status = event("return?", vec![]);
        if status != 0 {
            return status;
        }
        let text = text.cast::<Text>().read();
        out.write(i32::from(
            text.len == 6 && (0..6).all(|i| text.data.add(i).read() == b"RETURN"[i]),
        ));
        0
    }
}
unsafe extern "C" fn trace_return(_: Object, line: i32) -> i32 {
    unsafe { event("return", vec![line]) }
}
unsafe extern "C" fn debugger() -> Object {
    unsafe {
        if (*active()).debugger_live {
            active().cast()
        } else {
            ptr::null_mut()
        }
    }
}
unsafe extern "C" fn debug_info(
    _: Object,
    _: Object,
    _: Object,
    _: Object,
    line: i32,
    input: i32,
) -> i32 {
    unsafe { event("debugger", vec![line, input]) }
}
unsafe extern "C" fn runaway_suppressed(out: *mut i32) -> i32 {
    unsafe {
        out.write(0);
        event("runaway-query", vec![])
    }
}
unsafe extern "C" fn runaway_error(_: Object, limit: i32) -> i32 {
    unsafe { event("runaway-error", vec![limit]) }
}
fn services() -> Hp2VmFlowServices {
    Hp2VmFlowServices {
        string_size: size_of::<Text>(),
        slow_checks: 1,
        case_token: 0x0a,
        none_name: 0,
        guard_enabled: 0,
        runaway_suppressed,
        runaway_error,
        step,
        script_base,
        script_at,
        bad_case,
        branch,
        assertion,
        goto_label,
        missing_label,
        assign_string,
        strings_equal,
        destroy_string,
        is_return,
        trace_return,
        debugger,
        debug_info,
    }
}
unsafe fn no_result(f: &mut Frame, op: Hp2VmFlowOp) -> Result<(), i32> {
    unsafe { f.run(op, ptr::null_mut(), ptr::null_mut(), ptr::null_mut()) }
}
unsafe fn switch(
    f: &mut Frame,
    first: &mut [u64; 128],
    second: &mut [u64; 128],
) -> Result<(), i32> {
    unsafe {
        f.run(
            VmFlowSwitch,
            ptr::null_mut(),
            first.as_mut_ptr().cast(),
            second.as_mut_ptr().cast(),
        )
    }
}
fn debug_bytes(text: &[u8]) -> Vec<u8> {
    [
        100i32.to_ne_bytes().as_slice(),
        (-12i32).to_ne_bytes().as_slice(),
        i32::MAX.to_ne_bytes().as_slice(),
        text,
        b"\0",
    ]
    .concat()
}

#[test]
fn scalar_constants_keep_exact_width_and_unaligned_bits() {
    unsafe {
        for (op, bytes, expected, width) in [
            (
                VmFlowIntConst,
                vec![0xff, 0xff, 0xff, 0x80],
                vec![0xff, 0xff, 0xff, 0x80],
                4,
            ),
            (
                VmFlowFloatConst,
                0x7fc12345u32.to_ne_bytes().to_vec(),
                0x7fc12345u32.to_ne_bytes().to_vec(),
                4,
            ),
            (
                VmFlowNameConst,
                i32::MAX.to_ne_bytes().to_vec(),
                i32::MAX.to_ne_bytes().to_vec(),
                4,
            ),
            (VmFlowByteConst, vec![255], vec![255], 1),
            (
                VmFlowIntConstByte,
                vec![255],
                255i32.to_ne_bytes().to_vec(),
                1,
            ),
        ] {
            let mut f = Frame::new(&[vec![0x5a], bytes].concat());
            f.code = f.code.add(1);
            let mut out = [0xa5u8; 12];
            f.run(
                op,
                out.as_mut_ptr().add(1).cast(),
                ptr::null_mut(),
                ptr::null_mut(),
            )
            .unwrap();
            assert_eq!(&out[1..1 + expected.len()], expected);
            assert_eq!(out[0], 0xa5);
            assert!(out[1 + expected.len()..].iter().all(|&b| b == 0xa5));
            assert_eq!(f.position(), 1 + width);
        }
    }
}
#[test]
fn implicit_constants_do_not_consume_code_or_overwrite_adjacent_storage() {
    unsafe {
        for (op, value) in [
            (VmFlowIntZero, 0u32),
            (VmFlowFalse, 0),
            (VmFlowIntOne, 1),
            (VmFlowTrue, 1),
        ] {
            let mut f = Frame::new(&[255]);
            let mut out = [0xa5u8; 12];
            f.run(
                op,
                out.as_mut_ptr().add(1).cast(),
                ptr::null_mut(),
                ptr::null_mut(),
            )
            .unwrap();
            assert_eq!(&out[1..5], value.to_ne_bytes());
            assert_eq!(out[0], 0xa5);
            assert_eq!(out[5], 0xa5);
            assert_eq!(f.position(), 0);
        }
        let mut f = Frame::new(&[255]);
        let mut out = [0xa5u8; 32];
        f.run(
            VmFlowNoObject,
            out.as_mut_ptr().add(1).cast(),
            ptr::null_mut(),
            ptr::null_mut(),
        )
        .unwrap();
        assert!(out[1..1 + size_of::<Object>()].iter().all(|&b| b == 0));
        assert_eq!(out[1 + size_of::<Object>()], 0xa5);
        assert_eq!(f.position(), 0);
    }
}
#[test]
fn scalar_publication_may_alias_operand_bytes() {
    unsafe {
        let mut f = Frame::new(&[0xff, 0xaa, 0xbb, 0xcc, 0xdd]);
        let result = f.code;
        f.run(
            VmFlowIntConstByte,
            result.cast(),
            ptr::null_mut(),
            ptr::null_mut(),
        )
        .unwrap();
        assert_eq!(&f.script[..4], 255i32.to_ne_bytes());
        assert_eq!(f.script[4], 0xdd);
        assert_eq!(f.position(), 1);
    }
}
unsafe extern "C" fn object_storage_check(kind: i32, condition: i32) -> i32 {
    if condition != 0 { 0 } else { 90 + kind }
}

#[test]
fn object_literals_resolve_live_slots_and_publish_only_a_pointer() {
    use hp2_core::{object_storage::{self, Hp2ObjectStorageServices, ObjectStorage}, vm_flow::execute_object_constant};
    unsafe {
        let mut storage = ObjectStorage::new();
        let mut object: hp2_abi::engine::generated::UObject = std::mem::zeroed();
        let services = Hp2ObjectStorageServices { check: object_storage_check };
        let handle = (&raw mut object).cast();
        object_storage::add(&raw mut storage, handle, -1, &services).unwrap();
        for (index, expected) in [(0_i32, handle), (-1, ptr::null_mut()), (i32::MAX, ptr::null_mut())] {
            let mut bytes = index.to_ne_bytes();
            let mut cursor = bytes.as_mut_ptr();
            let mut guarded = [0xa5_u8; 24];
            execute_object_constant(&raw mut cursor, guarded.as_mut_ptr().add(8).cast(), &raw const storage);
            assert_eq!(cursor, bytes.as_mut_ptr().add(4));
            assert_eq!(guarded.as_ptr().add(8).cast::<Object>().read_unaligned(), expected);
            assert!(guarded[..8].iter().chain(guarded[8 + size_of::<Object>()..].iter()).all(|&byte| byte == 0xa5));
        }
        let bucket = object_storage::hash_bucket(0, 0);
        object_storage::unhash(&raw mut storage, handle, bucket, &services).unwrap();
        storage.remove(0);
        let mut bytes = 0_i32.to_ne_bytes();
        let mut cursor = bytes.as_mut_ptr();
        let mut value = handle;
        execute_object_constant(&raw mut cursor, (&raw mut value).cast(), &raw const storage);
        assert!(value.is_null());
    }
}

#[test]
fn object_literal_publication_can_alias_the_code_slot() {
    use hp2_core::{object_storage::{self, Hp2ObjectStorageServices, ObjectStorage}, vm_flow::execute_object_constant};
    unsafe {
        let mut storage = ObjectStorage::new();
        let mut object: hp2_abi::engine::generated::UObject = std::mem::zeroed();
        let services = Hp2ObjectStorageServices { check: object_storage_check };
        let handle = (&raw mut object).cast();
        object_storage::add(&raw mut storage, handle, -1, &services).unwrap();
        let mut bytes = 0_i32.to_ne_bytes();
        let mut cursor = bytes.as_mut_ptr();
        execute_object_constant(&raw mut cursor, (&raw mut cursor).cast(), &raw const storage);
        assert_eq!(cursor.cast::<c_void>(), handle);
    }
}
unsafe extern "C" fn literal_realloc(
    _context: Object, data: Object, bytes: usize, max: *const i32,
    width: i32, tag: *const c_void, out: *mut Object,
) -> i32 {
    unsafe {
        let status = event("realloc", vec![]);
        if status != 0 {
            return status;
        }
        let frame = active();
        if let Some(offset) = (*frame).assign_redirect {
            (*frame).code = (*frame).script.as_mut_ptr().add(offset);
        }
        let arrays = runtime_archive::array_services(ptr::null_mut());
        (arrays.realloc)(arrays.context, data, bytes, max, width, tag, out)
    }
}

unsafe fn run_literal(frame: *mut Frame, unicode: bool, result: *mut TArrayRaw) -> Result<(), i32> {
    let previous = ACTIVE.with(|a| a.replace(frame));
    let arrays = Hp2ArrayServices {
        realloc: literal_realloc,
        ..runtime_archive::array_services(ptr::null_mut())
    };
    let answer = unsafe { execute_string_constant(unicode, &raw mut (*frame).code, result, arrays) };
    ACTIVE.with(|a| a.set(previous));
    answer
}

#[test]
fn string_assignment_precedes_scan_of_reentered_live_cursor() {
    let _ring = LITERAL_RING.lock().unwrap();
    unsafe {
        for (unicode, bytes, replacement, expected, end) in [
            (false, b"abc\0".to_vec(), b"XY\0".to_vec(), vec![97, 98, 99, 0], 43),
            (
                true,
                [65u16, 0x100, 0].into_iter().flat_map(u16::to_ne_bytes).collect(),
                [0x100u16, 66, 0].into_iter().flat_map(u16::to_ne_bytes).collect(),
                vec![65, 0x100, 0],
                46,
            ),
        ] {
            let mut f = Frame::new(&bytes);
            f.script[40..40 + replacement.len()].copy_from_slice(&replacement);
            f.assign_redirect = Some(40);
            let arrays = runtime_archive::array_services(ptr::null_mut());
            let mut text = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
            let slots = runtime_archive::array_slots(&raw mut text);
            // Replace an actual initialized allocation, not uninitialized Text.
            array_storage::set_exact(slots, 2, 4, ptr::null(), arrays).unwrap();
            text.data.cast::<hp2_core::ansi::Tchar>().write(90);
            text.data.cast::<hp2_core::ansi::Tchar>().add(1).write(0);
            run_literal(&raw mut *f, unicode, &raw mut text).unwrap();
            let actual = std::slice::from_raw_parts(text.data.cast::<hp2_core::ansi::Tchar>(), text.num as usize);
            assert_eq!(actual, expected);
            assert_eq!(f.events[0].1, 0);
            assert_eq!(f.position(), end);
            array_storage::release(slots, arrays).unwrap();
        }
    }
}

#[test]
fn string_assignment_error_does_not_scan_or_replace_data() {
    let _ring = LITERAL_RING.lock().unwrap();
    unsafe {
        for unicode in [false, true] {
            let mut f = Frame::new(b"\0\0");
            f.fail = "realloc";
            let arrays = runtime_archive::array_services(ptr::null_mut());
            let mut text = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
            let slots = runtime_archive::array_slots(&raw mut text);
            array_storage::set_exact(slots, 2, 4, ptr::null(), arrays).unwrap();
            let before = text.data;
            assert_eq!(run_literal(&raw mut *f, unicode, &raw mut text), Err(73));
            assert_eq!(f.position(), 0);
            assert_eq!(text.data, before);
            // Canonical SetExact publishes counts before asking the allocator.
            assert_eq!((text.num, text.max), (0, 0));
            array_storage::release(slots, arrays).unwrap();
        }
    }
}

#[test]
fn string_literals_share_reentrant_conversion_ring_and_legacy_width_limits() {
    let _ring = LITERAL_RING.lock().unwrap();
    unsafe extern "C" fn wrap_ring(
        context: Object, data: Object, bytes: usize, max: *const i32,
        width: i32, tag: *const c_void, out: *mut Object,
    ) -> i32 {
        unsafe {
            let arrays = runtime_archive::array_services(ptr::null_mut());
            let status = (arrays.realloc)(context, data, bytes, max, width, tag, out);
            if status == 0 {
                for _ in 0..128 {
                    hp2_core::ansi::from_ansi(c"new".as_ptr().cast());
                }
            }
            status
        }
    }
    unsafe {
        let arrays = runtime_archive::array_services(ptr::null_mut());
        let mut text = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
        let slots = runtime_archive::array_slots(&raw mut text);
        let mut bytes = *b"old\0";
        let mut cursor = bytes.as_mut_ptr();
        execute_string_constant(false, &raw mut cursor, &raw mut text,
            Hp2ArrayServices { realloc: wrap_ring, ..arrays }).unwrap();
        assert_eq!(std::slice::from_raw_parts(text.data.cast::<hp2_core::ansi::Tchar>(), 4),
            &[110, 101, 119, 0]);
        assert_eq!(cursor, bytes.as_mut_ptr().add(4));

        let mut long = vec![0xffu8; 1100];
        long.push(0);
        cursor = long.as_mut_ptr();
        execute_string_constant(false, &raw mut cursor, &raw mut text, arrays).unwrap();
        assert_eq!(text.num, 1024);
        assert_eq!(text.data.cast::<hp2_core::ansi::Tchar>().read(), 255);
        assert_eq!(text.data.cast::<hp2_core::ansi::Tchar>().add(1023).read(), 0);
        assert_eq!(cursor, long.as_mut_ptr().add(1101));

        // WORD bytecode is allowed at an odd address. Decode pairs but replace
        // isolated surrogates; scan still consumes every original WORD.
        let mut words = vec![0xa5u8];
        words.extend([0xd83du16, 0xde00, 0xd800, 65, 0xdc00, 0]
            .into_iter().flat_map(u16::to_ne_bytes));
        cursor = words.as_mut_ptr().add(1);
        execute_string_constant(true, &raw mut cursor, &raw mut text, arrays).unwrap();
        assert_eq!(std::slice::from_raw_parts(text.data.cast::<hp2_core::ansi::Tchar>(), text.num as usize),
            &[0x1f600, 0xfffd, 65, 0xfffd, 0]);
        assert_eq!(cursor, words.as_mut_ptr().add(words.len()));
        array_storage::release(slots, arrays).unwrap();
    }
}
#[test]
fn optional_parameter_rewinds_and_stop_nulls_cursor() {
    unsafe {
        let mut f = Frame::new(&[4]);
        f.code = f.code.add(1);
        no_result(&mut f, VmFlowEndFunctionParms).unwrap();
        assert_eq!(f.position(), 0);
        no_result(&mut f, VmFlowStop).unwrap();
        assert!(f.code.is_null());
    }
}
#[test]
fn wrong_debug_version_rewinds_five_bytes_without_callbacks() {
    unsafe {
        for version in [0i32, 99, 101, i32::MIN] {
            let mut f = Frame::new(&[vec![0x55], version.to_ne_bytes().to_vec()].concat());
            f.code = f.code.add(1);
            no_result(&mut f, VmFlowDebugInfo).unwrap();
            assert_eq!(f.position(), 0);
            assert!(f.events.is_empty());
        }
    }
}
#[test]
fn debug_return_traces_before_live_debugger_lookup_and_preserves_signed_fields() {
    unsafe {
        let mut f = Frame::new(&debug_bytes(b"RETURN"));
        f.debugger_live = true;
        let mut text = Text {
            data: ptr::null(),
            len: 0,
        };
        f.run(
            VmFlowDebugInfo,
            ptr::null_mut(),
            (&raw mut text).cast(),
            ptr::null_mut(),
        )
        .unwrap();
        assert_eq!(f.names(), ["assign", "return?", "return", "debugger"]);
        assert_eq!(f.events[0].1, 12);
        assert!(f.events[1..].iter().all(|e| e.1 == 19));
        assert_eq!(f.events[3].2, [-12, i32::MAX]);
    }
}
#[test]
fn debug_nonreturn_and_absent_debugger_skip_only_their_callbacks() {
    unsafe {
        let mut f = Frame::new(&debug_bytes(b"return"));
        let mut text = Text {
            data: ptr::null(),
            len: 0,
        };
        f.run(
            VmFlowDebugInfo,
            ptr::null_mut(),
            (&raw mut text).cast(),
            ptr::null_mut(),
        )
        .unwrap();
        assert_eq!(f.names(), ["assign", "return?"]);
        assert_eq!(f.position(), 19);
    }
}
#[test]
fn debug_first_error_stops_trace_chain_at_each_boundary() {
    unsafe {
        let order = ["assign", "return?", "return", "debugger"];
        for (i, fail) in order.iter().enumerate() {
            let mut f = Frame::new(&debug_bytes(b"RETURN"));
            f.fail = fail;
            f.debugger_live = true;
            let mut text = Text {
                data: ptr::null(),
                len: 0,
            };
            assert_eq!(
                f.run(
                    VmFlowDebugInfo,
                    ptr::null_mut(),
                    (&raw mut text).cast(),
                    ptr::null_mut()
                ),
                Err(73)
            );
            assert_eq!(f.names(), order[..=i]);
            assert_eq!(f.position(), if i == 0 { 12 } else { 19 });
        }
    }
}
#[test]
fn unconditional_jump_publishes_target_only_after_lookup_success() {
    unsafe {
        for fail in [false, true] {
            let mut f = Frame::new(&31u16.to_ne_bytes());
            if fail {
                f.fail = "jump";
            }
            assert_eq!(
                no_result(&mut f, VmFlowJump),
                if fail { Err(73) } else { Ok(()) }
            );
            assert_eq!(f.position(), if fail { 2 } else { 31 });
            assert_eq!(f.events, [Event("jump", 2, vec![31])]);
        }
    }
}
#[test]
fn branch_traces_post_expression_before_jump_and_respects_true_reentry() {
    unsafe {
        for value in [0i32, 1, -1] {
            let mut f = Frame::new(
                &[
                    vec![0x77],
                    31u16.to_ne_bytes().to_vec(),
                    vec![1],
                    value.to_ne_bytes().to_vec(),
                ]
                .concat(),
            );
            f.code = f.code.add(1);
            f.branch_redirect = Some(50);
            no_result(&mut f, VmFlowJumpIfNot).unwrap();
            assert_eq!(f.events[3], Event("branch", 8, vec![0, 31, 8, value]));
            assert_eq!(f.position(), if value == 0 { 31 } else { 50 });
            assert_eq!(
                f.names(),
                if value == 0 {
                    vec!["base", "step", "base", "branch", "jump"]
                } else {
                    vec!["base", "step", "base", "branch"]
                }
            );
        }
    }
}
#[test]
fn branch_without_node_records_index_none_not_minus_two() {
    unsafe {
        let mut f = Frame::new(&[9, 0, 1, 1, 0, 0, 0]);
        f.no_node = true;
        no_result(&mut f, VmFlowJumpIfNot).unwrap();
        assert_eq!(f.events[3].2, [-1, 9, -1, 1]);
    }
}
#[test]
fn branch_error_preserves_callback_mutation_and_never_jumps() {
    unsafe {
        let mut f = Frame::new(&[31, 0, 1, 0, 0, 0, 0]);
        f.fail = "branch";
        f.branch_redirect = Some(50);
        assert_eq!(no_result(&mut f, VmFlowJumpIfNot), Err(73));
        assert_eq!(f.position(), 50);
        assert!(!f.names().contains(&"jump"));
    }
}
#[test]
fn branch_step_failure_stops_before_post_expression_trace() {
    unsafe {
        let mut f = Frame::new(&[31, 0, 1, 0, 0, 0, 0]);
        f.fail = "step";
        assert_eq!(no_result(&mut f, VmFlowJumpIfNot), Err(73));
        assert_eq!(f.position(), 2);
        assert_eq!(f.names(), ["base", "step"]);
    }
}
#[test]
fn assertion_uses_unsigned_line_and_full_word_truth() {
    unsafe {
        for value in [0u32, 0x80000000, 2] {
            let mut f = Frame::new(
                &[
                    u16::MAX.to_ne_bytes().to_vec(),
                    vec![1],
                    value.to_ne_bytes().to_vec(),
                ]
                .concat(),
            );
            no_result(&mut f, VmFlowAssert).unwrap();
            assert_eq!(f.position(), 7);
            if value == 0 {
                assert_eq!(f.events[1], Event("assert", 7, vec![65535]));
            } else {
                assert_eq!(f.names(), ["step"]);
            }
        }
    }
}
#[test]
fn goto_label_decodes_name_and_reports_only_missing_label() {
    unsafe {
        for name in [7i32, 8] {
            let mut f = Frame::new(&[vec![1], name.to_ne_bytes().to_vec()].concat());
            no_result(&mut f, VmFlowGotoLabel).unwrap();
            assert_eq!(f.events[1], Event("label", 5, vec![name]));
            assert_eq!(
                f.names(),
                if name == 7 {
                    vec!["step", "label"]
                } else {
                    vec!["step", "label", "missing"]
                }
            );
        }
        let mut f = Frame::new(&[4]);
        no_result(&mut f, VmFlowGotoLabel).unwrap();
        assert_eq!(f.events[1], Event("label", 0, vec![0]));
    }
}
#[test]
fn label_lookup_error_does_not_report_missing_label() {
    unsafe {
        let mut f = Frame::new(&[1, 8, 0, 0, 0]);
        f.fail = "label";
        assert_eq!(no_result(&mut f, VmFlowGotoLabel), Err(73));
        assert_eq!(f.names(), ["step", "label"]);
        assert_eq!(f.position(), 5);
    }
}
#[test]
fn case_default_skips_scratch_and_nondefault_evaluates_without_cleanup() {
    unsafe {
        for default in [false, true] {
            let mut f = Frame::new(
                &[
                    if default { u16::MAX } else { 12u16 }
                        .to_ne_bytes()
                        .to_vec(),
                    vec![3],
                    b"abc\0".to_vec(),
                ]
                .concat(),
            );
            let mut scratch = [0xa5a5a5a5a5a5a5a5u64; 128];
            f.run(
                VmFlowCase,
                ptr::null_mut(),
                scratch.as_mut_ptr().cast(),
                ptr::null_mut(),
            )
            .unwrap();
            assert_eq!(f.position(), if default { 2 } else { 7 });
            assert!(!f.names().contains(&"destroy"));
            if default {
                assert!(f.events.is_empty());
                assert_eq!(scratch[0], 0xa5a5a5a5a5a5a5a5);
            } else {
                assert_eq!(scratch.as_ptr().cast::<Text>().read().len, 3);
            }
        }
    }
}
#[test]
fn switch_mismatch_jumps_then_match_stays_after_case_expression() {
    unsafe {
        let mut f = Frame::new(&[1, 2, 7, 0x0a, 20, 0, 2, 8]);
        f.script[20..25].copy_from_slice(&[0x0a, 30, 0, 2, 7]);
        let (mut a, mut b) = ([0xa5u64; 128], [0xa5u64; 128]);
        switch(&mut f, &mut a, &mut b).unwrap();
        assert_eq!(f.position(), 25);
        assert_eq!(f.names(), ["step", "step", "jump", "step"]);
        assert_eq!(f.events[2], Event("jump", 8, vec![20]));
        assert_eq!(a[2], 0xa5);
        assert_eq!(b[2], 0xa5);
    }
}
#[test]
fn switch_default_initializes_only_string_prefix_and_destroys_both_strings() {
    unsafe {
        let mut f = Frame::new(&[vec![0, 3], b"abc\0".to_vec(), vec![0x0a, 255, 255]].concat());
        let (mut a, mut b) = ([0xa5u64; 128], [0xa5u64; 128]);
        switch(&mut f, &mut a, &mut b).unwrap();
        assert_eq!(f.position(), 9);
        assert_eq!(f.names(), ["step", "assign", "destroy", "destroy"]);
        assert_eq!(f.events[2].2, [3]);
        assert_eq!(f.events[3].2, [0]);
        assert_eq!(a[2], 0xa5);
        assert_eq!(b[2], 0xa5);
    }
}
#[test]
fn string_switch_compares_content_and_destroys_in_operand_order() {
    unsafe {
        let mut f = Frame::new(
            &[
                vec![0, 3],
                b"same\0".to_vec(),
                vec![0x0a, 50, 0, 3],
                b"same\0".to_vec(),
            ]
            .concat(),
        );
        let (mut a, mut b) = ([0u64; 128], [0u64; 128]);
        switch(&mut f, &mut a, &mut b).unwrap();
        assert_eq!(f.position(), 16);
        assert_eq!(
            f.names(),
            [
                "step", "assign", "step", "assign", "equal", "destroy", "destroy"
            ]
        );
        assert_eq!(a[0], 0);
        assert_eq!(b[0], 0);
    }
}
#[test]
fn switch_first_error_does_not_invent_string_cleanup() {
    unsafe {
        for (fail, occurrence, expected) in [
            ("step", 1, vec!["step"]),
            ("step", 2, vec!["step", "assign", "step"]),
            (
                "equal",
                1,
                vec!["step", "assign", "step", "assign", "equal"],
            ),
            (
                "destroy",
                1,
                vec!["step", "assign", "step", "assign", "equal", "destroy"],
            ),
            (
                "destroy",
                2,
                vec![
                    "step", "assign", "step", "assign", "equal", "destroy", "destroy",
                ],
            ),
        ] {
            let mut f = Frame::new(&[0, 3, b'a', 0, 0x0a, 50, 0, 3, b'a', 0]);
            f.fail = fail;
            f.fail_occurrence = occurrence;
            let (mut a, mut b) = ([0u64; 128], [0u64; 128]);
            assert_eq!(switch(&mut f, &mut a, &mut b), Err(73));
            assert_eq!(f.names(), expected);
        }
    }
}
#[test]
fn switch_bad_case_fails_before_consuming_token_or_cleaning_scratch() {
    unsafe {
        let mut f = Frame::new(&[0, 3, 0, 0x99, 255, 255]);
        let (mut a, mut b) = ([0u64; 128], [0u64; 128]);
        assert_eq!(switch(&mut f, &mut a, &mut b), Err(92));
        assert_eq!(f.position(), 3);
        assert_eq!(f.names(), ["step", "assign", "bad_case"]);
    }
}

#[test]
fn switch_compares_all_255_bytes_without_cleanup_for_scalar_cases() {
    unsafe {
        let mut f = Frame::new(&[255, 2, 7, 0x0a, 20, 0, 2, 7]);
        f.script[20..23].copy_from_slice(&[0x0a, 255, 255]);
        let (mut a, mut b) = ([0u64; 128], [0u64; 128]);
        b.as_mut_ptr().cast::<u8>().add(254).write(1);
        switch(&mut f, &mut a, &mut b).unwrap();
        assert_eq!(f.position(), 23);
        assert_eq!(f.names(), ["step", "step", "jump"]);
        assert_eq!(f.events[2], Event("jump", 8, vec![20]));
    }
}

#[test]
fn unchecked_switch_skips_case_token_without_assertion_callback() {
    unsafe {
        let mut f = Frame::new(&[1, 2, 7, 0x99, 255, 255]);
        let (mut a, mut b) = ([0u64; 128], [0u64; 128]);
        let mut s = services();
        s.slow_checks = 0;
        let p = &raw mut *f;
        ACTIVE.with(|active| active.set(p));
        let answer = execute(
            VmFlowSwitch,
            p.cast(),
            p.cast(),
            &raw mut (*p).code,
            ptr::null_mut(),
            a.as_mut_ptr().cast(),
            b.as_mut_ptr().cast(),
            &s,
        );
        ACTIVE.with(|active| active.set(ptr::null_mut()));
        assert_eq!(answer, Ok(()));
        assert_eq!(f.position(), 6);
        assert_eq!(f.names(), ["step"]);
    }
}

#[test]
fn debug_conversion_reentry_scans_live_code_but_retains_converted_info() {
    unsafe {
        let mut f = Frame::new(&debug_bytes(b"RETURN"));
        f.script[40..44].copy_from_slice(b"xyz\0");
        f.assign_redirect = Some(40);
        let mut text = Text {
            data: ptr::null(),
            len: 0,
        };
        f.run(
            VmFlowDebugInfo,
            ptr::null_mut(),
            (&raw mut text).cast(),
            ptr::null_mut(),
        )
        .unwrap();
        assert_eq!(text.data, f.script.as_ptr().add(12));
        assert_eq!(f.position(), 44);
        assert_eq!(f.events[2], Event("return", 44, vec![-12]));
        assert_eq!(f.names(), ["assign", "return?", "return"]);
    }
}

#[test]
fn assertion_and_missing_label_propagate_reporting_errors() {
    unsafe {
        let mut assertion = Frame::new(&[255, 255, 1, 0, 0, 0, 0]);
        assertion.fail = "assert";
        assert_eq!(no_result(&mut assertion, VmFlowAssert), Err(73));
        assert_eq!(assertion.position(), 7);
        let mut label = Frame::new(&[1, 8, 0, 0, 0]);
        label.fail = "missing";
        assert_eq!(no_result(&mut label, VmFlowGotoLabel), Err(73));
        assert_eq!(label.position(), 5);
        assert_eq!(label.names(), ["step", "label", "missing"]);
    }
}

// Only this test enables the process-wide guard in this integration-test binary.
// Other flow contracts use guard_enabled=0, including their nested executions.
// Ten million real jumps exercise the production threshold without seeding it.
#[test]
fn runaway_guard_keeps_live_state_and_cursor_across_errors_and_reentry() {
    use hp2_core::vm_guard::{HP2_VM_RUNAWAY_LIMIT, reset};
    struct GuardFrame {
        bytes: [u8; 8],
        code: *mut u8,
        queries: usize,
        reports: usize,
        jumps: usize,
        skip: i32,
        failure: i32,
        reenter: bool,
        reset_in_report: bool,
    }
    static mut ACTIVE_GUARD: *mut GuardFrame = ptr::null_mut();
    unsafe extern "C" fn query(out: *mut i32) -> i32 {
        unsafe {
            let f = ACTIVE_GUARD;
            (*f).queries += 1;
            out.write((*f).skip);
            0
        }
    }
    unsafe extern "C" fn report(p: Object, limit: i32) -> i32 {
        unsafe {
            let f = p.cast::<GuardFrame>();
            if limit != HP2_VM_RUNAWAY_LIMIT {
                return 99;
            }
            (*f).reports += 1;
            if (*f).reset_in_report {
                reset(1);
            }
            if (*f).reenter {
                (*f).reenter = false;
                if let Err(error) = run(f, VmFlowJump) {
                    return error;
                }
                // The outer jump must read the newly published operands.
                (*f).code = (*f).bytes.as_mut_ptr().add(4);
            }
            (*f).failure
        }
    }
    unsafe extern "C" fn at(p: Object, offset: u32, out: *mut *mut u8) -> i32 {
        unsafe {
            let f = p.cast::<GuardFrame>();
            (*f).jumps += 1;
            out.write((*f).bytes.as_mut_ptr().add(offset as usize));
            0
        }
    }
    unsafe fn run(f: *mut GuardFrame, op: Hp2VmFlowOp) -> Result<(), i32> {
        let mut s = services();
        s.guard_enabled = 1;
        s.runaway_suppressed = query;
        s.runaway_error = report;
        s.script_at = at;
        unsafe {
            execute(
                op,
                f.cast(),
                f.cast(),
                &raw mut (*f).code,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                &s,
            )
        }
    }
    let mut f = GuardFrame {
        bytes: [0, 0, 0, 0, 6, 0, 0, 0],
        code: ptr::null_mut(),
        queries: 0,
        reports: 0,
        jumps: 0,
        skip: 0,
        failure: 0,
        reenter: false,
        reset_in_report: false,
    };
    f.code = f.bytes.as_mut_ptr();
    let p = &raw mut f;
    unsafe {
        ACTIVE_GUARD = p;
        reset(1);
        // Disabled flow never consumes guard budget.
        let mut disabled = services();
        disabled.script_at = at;
        for _ in 0..3 {
            assert_eq!(
                execute(
                    VmFlowJump,
                    p.cast(),
                    p.cast(),
                    &raw mut (*p).code,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    &disabled
                ),
                Ok(())
            );
        }
        for _ in 0..HP2_VM_RUNAWAY_LIMIT {
            assert_eq!(run(p, VmFlowJump), Ok(()));
        }
        assert_eq!((f.queries, f.reports), (0, 0));
        f.failure = 73;
        let jumps = f.jumps;
        assert_eq!(run(p, VmFlowJumpIfNot), Err(73));
        assert_eq!(f.code, f.bytes.as_mut_ptr());
        assert_eq!(f.jumps, jumps);
        assert_eq!((f.queries, f.reports), (1, 1));
        reset(0); // Disabled reset must not clear the failed guard.
        assert_eq!(run(p, VmFlowJump), Err(73));
        assert_eq!((f.queries, f.reports), (2, 2));
        f.failure = 0;
        f.reenter = true;
        assert_eq!(run(p, VmFlowJump), Ok(()));
        assert_eq!((f.queries, f.reports), (4, 4));
        assert_eq!(f.code, f.bytes.as_mut_ptr().add(6));
        f.code = f.bytes.as_mut_ptr();
        assert_eq!(run(p, VmFlowJump), Ok(()));
        assert_eq!((f.queries, f.reports), (4, 4));
        reset(1);
        f.skip = 1;
        for _ in 0..=HP2_VM_RUNAWAY_LIMIT {
            assert_eq!(run(p, VmFlowJump), Ok(()));
        }
        assert_eq!((f.queries, f.reports), (5, 4));
        f.skip = 0;
        // Suppression reset the counter; changing the option is observed only
        // at the next threshold, never cached from an earlier query.
        for _ in 0..HP2_VM_RUNAWAY_LIMIT {
            assert_eq!(run(p, VmFlowJump), Ok(()));
        }
        assert_eq!((f.queries, f.reports), (5, 4));
        f.reset_in_report = true;
        f.reenter = true;
        assert_eq!(run(p, VmFlowJump), Ok(()));
        assert_eq!((f.queries, f.reports), (6, 5));
        reset(1);
        ACTIVE_GUARD = ptr::null_mut();
    }
}
