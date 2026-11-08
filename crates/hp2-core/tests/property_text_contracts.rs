use hp2_core::ansi::{Hp2AnsiServices, Tchar};
use hp2_core::property_text::*;
use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::null_mut;
type Object = *mut c_void;
fn obj<T>(p: &mut T) -> Object {
    (p as *mut T).cast()
}
fn wide(s: &str) -> Vec<Tchar> {
    s.chars().map(|c| c as Tchar).chain(Some(0)).collect()
}
unsafe fn text(mut p: *const Tchar) -> String {
    unsafe {
        let mut s = String::new();
        while *p != 0 {
            s.push(char::from_u32(*p as u32).unwrap());
            p = p.add(1);
        }
        s
    }
}
struct Node {
    kind: Hp2TextKind,
    dim: i32,
    size: i32,
    offset: i32,
    mask: i32,
    count: i32,
    flags: i32,
    category: i32,
    name: i32,
    inner: Object,
    structure: Object,
    children: Object,
    next: Object,
    parent: Object,
    class: Object,
    meta: Object,
    key: Object,
    value: Object,
    enumeration: Option<Vec<i32>>,
}
impl Node {
    fn new(kind: Hp2TextKind) -> Self {
        Self {
            kind,
            dim: 1,
            size: 4,
            offset: 0,
            mask: 4,
            count: 0,
            flags: 0,
            category: 0,
            name: 1,
            inner: null_mut(),
            structure: null_mut(),
            children: null_mut(),
            next: null_mut(),
            parent: null_mut(),
            class: null_mut(),
            meta: null_mut(),
            key: null_mut(),
            value: null_mut(),
            enumeration: None,
        }
    }
}
#[derive(Default)]
struct State {
    names: Vec<Box<[Tchar]>>,
    warnings: Vec<Hp2TextWarning>,
    calls: usize,
    fail_call: usize,
    fail_warning: bool,
    imports: usize,
    fail_import: usize,
    export_flags: Vec<i32>,
    classes: Vec<(String, Object)>,
    objects: Vec<(Object, String, Object)>,
    replace_next: Option<(Object, Object)>,
    shrink_array: Option<Object>,
    ansi: Vec<u8>,
    numeric_asserts: Vec<i32>,
}
thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}
fn reset() {
    STATE.with(|s| {
        *s.borrow_mut() = State {
            names: [
                "None", "First", "Second", "Third", "TrueName", "a b", "a,b", "a\tb", "Oui", "Non",
            ]
            .into_iter()
            .map(|n| wide(n).into_boxed_slice())
            .collect(),
            ..State::default()
        }
    });
}
fn name(index: i32) -> Object {
    STATE.with(|s| s.borrow().names[index as usize].as_ptr().cast_mut().cast())
}
unsafe extern "C" fn integer(p: Object, f: Hp2TextInteger) -> i32 {
    use Hp2TextInteger::*;
    unsafe {
        if matches!(f, TextArrayNum) {
            return (*p.cast::<Vec<i32>>()).len() as i32;
        }
        let n = &*p.cast::<Node>();
        match f {
            TextDim => n.dim,
            TextSize => n.size,
            TextOffset => n.offset,
            TextMask => n.mask,
            TextCount => n.count,
            TextHasEnum => n.enumeration.is_some() as i32,
            TextEnumCount => n.enumeration.as_ref().map_or(0, |e| e.len() as i32),
            TextNameId => n.name,
            TextCategory => n.category,
            TextFlags => n.flags,
            TextIsProperty => 1,
            TextIsBool => (n.kind == Hp2TextKind::TextBool) as i32,
            TextIsString => (n.kind == Hp2TextKind::TextString) as i32,
            TextArrayNum => unreachable!(),
        }
    }
}
unsafe extern "C" fn pointer(p: Object, f: Hp2TextPointer, index: i32, result: *mut Object) -> i32 {
    use Hp2TextPointer::*;
    unsafe {
        *result = match f {
            TextTrue => name(8),
            TextFalse => name(9),
            TextData => (*p.cast::<Vec<i32>>()).as_mut_ptr().cast(),
            TextStringData => (*p.cast::<Vec<Tchar>>()).as_mut_ptr().cast(),
            TextNameData => name(*p.cast::<i32>()),
            _ => {
                let n = &*p.cast::<Node>();
                match f {
                    TextInner => n.inner,
                    TextStructType => n.structure,
                    TextChildren => n.children,
                    TextNext => n.next,
                    TextSuper => n.parent,
                    TextPropertyName | TextCppName | TextClassName | TextObjectPath => name(n.name),
                    TextPropertyClass => n.class,
                    TextMetaClass => n.meta,
                    TextEnumName => name(n.enumeration.as_ref().unwrap()[index as usize]),
                    TextKey => n.key,
                    TextValue => n.value,
                    _ => unreachable!(),
                }
            }
        };
    }
    0
}
unsafe extern "C" fn call(
    p: Object,
    op: Hp2TextCall,
    a: Object,
    b: Object,
    c: Object,
    index: i32,
    flags: i32,
    result: *mut Object,
    number: *mut i32,
) -> i32 {
    use Hp2TextCall::*;
    let failed = STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls += 1;
        s.calls == s.fail_call
    });
    if failed {
        return 71;
    }
    unsafe {
        let recursive = match op {
            TextVirtualImport => {
                let fail = STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.imports += 1;
                    s.imports == s.fail_import
                });
                if fail {
                    return 73;
                }
                Some(Hp2TextOperation::TextImport)
            }
            TextVirtualExport => Some(Hp2TextOperation::TextExport),
            TextVirtualExportItem => {
                STATE.with(|s| s.borrow_mut().export_flags.push(flags));
                Some(Hp2TextOperation::TextExportItem)
            }
            TextVirtualCpp => Some(Hp2TextOperation::TextCppItem),
            TextVirtualPort => Some(Hp2TextOperation::TextPort),
            _ => None,
        };
        if let Some(operation) = recursive {
            let kind = (*p.cast::<Node>()).kind;
            match run(kind, operation, p, a, b, c, index, flags, &SERVICES) {
                Ok((r, n)) => {
                    *result = r;
                    *number = n;
                }
                Err(code) => return code,
            }
            if matches!(op, TextVirtualExport) {
                let replacement = STATE.with(|s| s.borrow_mut().replace_next.take());
                if let Some((node, next)) = replacement {
                    (*node.cast::<Node>()).next = next;
                }
            }
            if matches!(op, TextVirtualExportItem) {
                let array = STATE.with(|s| s.borrow_mut().shrink_array.take());
                if let Some(array) = array {
                    (*array.cast::<Vec<i32>>()).truncate(1);
                }
            }
            return 0;
        }
        match op {
            TextMatches => {
                let n = &*p.cast::<Node>();
                let offset = n.offset + index * n.size;
                let av = a.cast::<u8>().offset(offset as isize).cast::<i32>().read();
                let bv = if b.is_null() {
                    0
                } else {
                    b.cast::<u8>().offset(offset as isize).cast::<i32>().read()
                };
                *number = (av == bv) as i32;
            }
            TextFindName => {
                let needle = text(a.cast());
                *number = STATE.with(|s| {
                    s.borrow()
                        .names
                        .iter()
                        .position(|v| text(v.as_ptr()).eq_ignore_ascii_case(&needle))
                        .unwrap_or(0) as i32
                });
            }
            TextAssignName => {
                let needle = text(b.cast());
                let id = STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    if let Some(i) = s
                        .names
                        .iter()
                        .position(|v| text(v.as_ptr()).eq_ignore_ascii_case(&needle))
                    {
                        i
                    } else {
                        let i = s.names.len();
                        s.names.push(wide(&needle).into_boxed_slice());
                        i
                    }
                });
                *a.cast::<i32>() = id as i32;
            }
            TextAssignString => {
                *a.cast::<Vec<Tchar>>() = wide(&text(b.cast()));
            }
            TextFindEnum => {
                *number = (*p.cast::<Node>())
                    .enumeration
                    .as_ref()
                    .unwrap()
                    .iter()
                    .position(|v| *v == index)
                    .map_or(-1, |i| i as i32);
            }
            TextFindObject => {
                let needle = text(a.cast());
                *result = STATE.with(|s| {
                    s.borrow()
                        .objects
                        .iter()
                        .find(|(class, n, _)| *class == p && *n == needle)
                        .map_or(null_mut(), |v| v.2)
                });
            }
            TextFindClass => {
                let needle = text(a.cast());
                *result = STATE.with(|s| {
                    s.borrow()
                        .classes
                        .iter()
                        .find(|(n, _)| *n == needle)
                        .map_or(null_mut(), |v| v.1)
                });
            }
            TextIsClass => {
                *number = ((*p.cast::<Node>()).kind == Hp2TextKind::TextClass) as i32;
            }
            TextIsChild => {
                let mut class = p;
                *number = 0;
                while !class.is_null() {
                    if class == a {
                        *number = 1;
                        break;
                    }
                    class = (*class.cast::<Node>()).parent;
                }
            }
            TextArrayEmpty => (*a.cast::<Vec<i32>>()).clear(),
            TextArrayAdd => {
                let array = &mut *a.cast::<Vec<i32>>();
                *number = array.len() as i32;
                array.push(0x12345678);
            }
            TextCheckInner | TextCheckMap => {}
            _ => unreachable!(),
        }
    }
    0
}
unsafe extern "C" fn format(
    out: Object,
    dest: Object,
    kind: Hp2TextFormat,
    a: *const c_void,
    b: *const c_void,
    n: i32,
    f: f64,
    count: *mut i32,
) -> i32 {
    use Hp2TextFormat::*;
    unsafe {
        let av = if a.is_null() {
            String::new()
        } else {
            text(a.cast())
        };
        let bv = if b.is_null() {
            String::new()
        } else {
            text(b.cast())
        };
        let value = match kind {
            TextFormatInt => n.to_string(),
            TextFormatFloat => f.to_string(),
            TextFormatIndex => format!("[{n}]"),
            TextFormatObject => format!("{av}'{bv}'"),
            TextFormatField => format!("{av}="),
            TextFormatArrayField => format!("{av}[{n}]="),
            TextFormatCppObject => format!("class {av}*"),
            TextFormatCppBool => format!(" {av}{bv}:1"),
            TextFormatCppReference => format!("& {av}{bv}"),
            TextFormatCppValue => format!(" {av}{bv}"),
            TextFormatCppPointer => format!("* {av}"),
            TextFormatCppString => format!("NoInit {av}{bv}"),
        };
        if out.is_null() {
            let value = wide(&value);
            *count = value.len() as i32 - 1;
            std::ptr::copy_nonoverlapping(value.as_ptr(), dest.cast(), value.len());
        } else {
            (*out.cast::<String>()).push_str(&value);
        }
    }
    0
}
unsafe extern "C" fn log(out: Object, value: *const c_void) -> i32 {
    unsafe {
        (*out.cast::<String>()).push_str(&text(value.cast()));
    }
    0
}
unsafe extern "C" fn warning(_: Object, kind: Hp2TextWarning, _: *const c_void) -> i32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.warnings.push(kind);
        if s.fail_warning { 79 } else { 0 }
    })
}
unsafe extern "C" fn fail_assert(_: *const c_void, _: *const c_void, line: i32) -> i32 {
    STATE.with(|s| s.borrow_mut().numeric_asserts.push(line));
    1
}
unsafe extern "C" fn to_ansi(source: *const c_void, result: *mut *const c_void) -> i32 {
    unsafe {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.ansi = text(source.cast()).into_bytes();
            s.ansi.push(0);
            *result = s.ansi.as_ptr().cast();
        });
    }
    0
}
unsafe extern "C" fn size_error() -> i32 {
    1
}
static ANSI: Hp2AnsiServices = Hp2AnsiServices {
    fail_assert,
    to_ansi,
    size_exceeds_int_contract: size_error,
};
const SERVICES: Hp2PropertyTextServices = Hp2PropertyTextServices {
    none_name: 0,
    class_name: 99,
    delimited: 1,
    localized: 2,
    transient: 4,
    native: 8,
    out_parm: 16,
    ansi: &ANSI,
    atoi_file: b"UnAnsi.cpp\0".as_ptr().cast(),
    atoi_line: 2213,
    integer,
    pointer,
    call,
    format,
    log,
    warning,
};
unsafe fn import(
    p: &mut Node,
    source: &[Tchar],
    data: Object,
    flags: i32,
) -> Result<*const Tchar, i32> {
    unsafe {
        run(
            p.kind,
            Hp2TextOperation::TextImport,
            obj(p),
            source.as_ptr().cast_mut().cast(),
            data,
            null_mut(),
            0,
            flags,
            &SERVICES,
        )
        .map(|v| v.0.cast::<Tchar>().cast_const())
    }
}
unsafe fn export(p: &mut Node, data: Object, default: Object, flags: i32) -> Vec<Tchar> {
    let mut out = vec![0; 4096];
    unsafe {
        run(
            p.kind,
            Hp2TextOperation::TextExportItem,
            obj(p),
            out.as_mut_ptr().cast(),
            data,
            default,
            0,
            flags,
            &SERVICES,
        )
        .unwrap();
    }
    out
}

#[test]
fn tokenizer_preserves_punctuation_hex_and_exact_capacity_failure() {
    reset();
    unsafe {
        for input in [",rest", "_name", "-12", "éclair", ""] {
            let input = wide(input);
            let mut dest = [99; 32];
            let end = read_token(input.as_ptr(), dest.as_mut_ptr(), 32, false, &SERVICES).unwrap();
            assert_eq!(end, input.as_ptr());
            assert_eq!(dest[0], input[0]);
            assert_eq!(dest[1], 0);
        }
        let input = wide("\"a\\41\\qF\\\\z\",rest");
        let mut dest = [99; 32];
        let end = read_token(input.as_ptr(), dest.as_mut_ptr(), 32, false, &SERVICES).unwrap();
        assert_eq!(&dest[..6], &[97, 65, 15, 92, 122, 0]);
        assert_eq!(*end, 44);
        let input = wide("abc");
        let mut dest = [99; 4];
        assert!(
            read_token(input.as_ptr(), dest.as_mut_ptr(), 4, false, &SERVICES)
                .unwrap()
                .is_null()
        );
        assert_eq!(dest, [97, 98, 99, 99]);
        let input = wide("\"abc\"");
        let mut dest = [77; 4];
        assert!(
            read_token(input.as_ptr(), dest.as_mut_ptr(), 4, false, &SERVICES)
                .unwrap()
                .is_null()
        );
        assert_eq!(dest[3], 77);
        STATE.with(|s| {
            assert_eq!(
                s.borrow().warnings,
                [
                    Hp2TextWarning::TextTokenOverflow,
                    Hp2TextWarning::TextQuotedOverflow
                ]
            )
        });
        let input = wide("Ab_1-2.name!");
        let mut dest = [0; 32];
        let end = read_token(input.as_ptr(), dest.as_mut_ptr(), 32, true, &SERVICES).unwrap();
        assert_eq!(text(dest.as_ptr()), "Ab_1-2.name");
        assert_eq!(*end, 33);
    }
}
#[test]
fn tokenizer_bad_quote_publishes_partial_output_and_first_warning_failure() {
    reset();
    unsafe {
        let source = wide("\"ab\nrest");
        let mut output = [88; 16];
        assert!(
            read_token(source.as_ptr(), output.as_mut_ptr(), 16, false, &SERVICES)
                .unwrap()
                .is_null()
        );
        assert_eq!(&output[..3], &[97, 98, 88]);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.fail_warning = true;
            s.warnings.clear();
        });
        let source = wide("abcd");
        assert_eq!(
            read_token(source.as_ptr(), output.as_mut_ptr(), 3, false, &SERVICES),
            Err(79)
        );
        STATE.with(|s| assert_eq!(s.borrow().warnings, [Hp2TextWarning::TextTokenOverflow]));
    }
}
#[test]
fn scalar_cursor_and_enum_numeric_fallback_are_not_normalized() {
    reset();
    unsafe {
        let mut p = Node::new(Hp2TextKind::TextInt);
        let mut value = 91;
        let input = wide("abc");
        assert_eq!(
            import(&mut p, &input, obj(&mut value), 0).unwrap(),
            input.as_ptr()
        );
        assert_eq!(value, 91);
        let input = wide("-12--3,x");
        let end = import(&mut p, &input, obj(&mut value), 0).unwrap();
        assert_eq!(value, -12);
        assert_eq!(*end, 44);
        let mut byte = Node::new(Hp2TextKind::TextByte);
        byte.size = 1;
        byte.enumeration = Some(vec![1, 2]);
        let mut value = 55u8;
        let input = wide("Second,");
        assert_eq!(*import(&mut byte, &input, obj(&mut value), 0).unwrap(), 44);
        assert_eq!(value, 1);
        let input = wide("123,");
        assert!(
            import(&mut byte, &input, obj(&mut value), 0)
                .unwrap()
                .is_null()
        );
        assert_eq!(value, 1);
        let input = wide("\"missing\"257,");
        assert_eq!(*import(&mut byte, &input, obj(&mut value), 0).unwrap(), 44);
        assert_eq!(value, 1);
        let mut float = Node::new(Hp2TextKind::TextFloat);
        let mut number = 0.0f32;
        let input = wide("1.25 garbage,tail");
        let end = import(&mut float, &input, obj(&mut number), 0).unwrap();
        assert_eq!(number, 1.25);
        assert_eq!(*end, 44);
    }
}
#[test]
fn boolean_masks_localization_and_invalid_input_keep_unrelated_bits() {
    reset();
    unsafe {
        let mut p = Node::new(Hp2TextKind::TextBool);
        let mut value = 0xa0u32;
        import(&mut p, &wide("Oui,"), obj(&mut value), 0).unwrap();
        assert_eq!(value, 0xa4);
        let out = export(&mut p, obj(&mut value), null_mut(), 2);
        assert_eq!(text(out.as_ptr()), "Oui");
        import(&mut p, &wide("false,"), obj(&mut value), 0).unwrap();
        assert_eq!(value, 0xa0);
        assert!(
            import(&mut p, &wide("perhaps,"), obj(&mut value), 0)
                .unwrap()
                .is_null()
        );
        assert_eq!(value, 0xa0);
    }
}
#[test]
fn strings_and_names_round_trip_without_inventing_escape_output() {
    reset();
    unsafe {
        let mut p = Node::new(Hp2TextKind::TextString);
        let mut value = wide("old");
        let input = wide("a,b rest");
        assert_eq!(
            import(&mut p, &input, obj(&mut value), 0).unwrap(),
            input.as_ptr()
        );
        assert_eq!(text(value.as_ptr()), "a,b rest");
        let output = export(&mut p, obj(&mut value), null_mut(), 1);
        assert_eq!(text(output.as_ptr()), "\"a,b rest\"");
        let mut roundtrip = wide("");
        let end = import(&mut p, &output, obj(&mut roundtrip), 1).unwrap();
        assert_eq!(*end, 0);
        assert_eq!(value, roundtrip);
        value = wide("a\"b");
        let output = export(&mut p, obj(&mut value), null_mut(), 1);
        assert_eq!(text(output.as_ptr()), "\"a\"b\"");
        let mut p = Node::new(Hp2TextKind::TextName);
        for (id, expected) in [(5, "\"a b\""), (6, "\"a,b\""), (7, "a\tb")] {
            let mut value = id;
            let output = export(&mut p, obj(&mut value), null_mut(), 0);
            assert_eq!(text(output.as_ptr()), expected);
        }
        let mut value = 5;
        let output = export(&mut p, obj(&mut value), null_mut(), 0);
        let mut parsed = 0;
        import(&mut p, &output, obj(&mut parsed), 0).unwrap();
        assert_eq!(parsed, value);
    }
}
#[test]
fn fixed_array_retains_last_delimiter_quirk_and_partial_mutation() {
    reset();
    unsafe {
        let mut inner = Node::new(Hp2TextKind::TextInt);
        let mut p = Node::new(Hp2TextKind::TextFixed);
        p.inner = obj(&mut inner);
        p.count = 2;
        p.size = 8;
        let mut values = [7i32, 9];
        let output = export(&mut p, obj(&mut values), null_mut(), 0);
        assert_eq!(text(output.as_ptr()), "(7,9)");
        let mut parsed = [99i32; 2];
        assert!(
            import(&mut p, &output, obj(&mut parsed), 0)
                .unwrap()
                .is_null()
        );
        assert_eq!(parsed, [7, 9]);
        let input = wide("(7,9,)tail");
        let end = import(&mut p, &input, obj(&mut parsed), 0).unwrap();
        assert_eq!(text(end), "tail");
        let input = wide("(3;4)");
        assert!(
            import(&mut p, &input, obj(&mut parsed), 0)
                .unwrap()
                .is_null()
        );
        assert_eq!(parsed, [3, 0]);
        let input = wide("bad");
        parsed = [11, 12];
        assert!(
            import(&mut p, &input, obj(&mut parsed), 0)
                .unwrap()
                .is_null()
        );
        assert_eq!(parsed, [11, 12]);
    }
}
#[test]
fn dynamic_array_round_trip_trailing_comma_and_first_error_publication() {
    reset();
    unsafe {
        let mut inner = Node::new(Hp2TextKind::TextInt);
        let mut p = Node::new(Hp2TextKind::TextArray);
        p.inner = obj(&mut inner);
        let mut values = vec![8i32, 9];
        let input = wide("(1,2,)tail");
        let end = import(&mut p, &input, obj(&mut values), 0).unwrap();
        assert_eq!(text(end), "tail");
        assert_eq!(values, [1, 2]);
        let output = export(&mut p, obj(&mut values), null_mut(), 0);
        assert_eq!(text(output.as_ptr()), "(1,2)");
        let mut parsed = Vec::<i32>::new();
        import(&mut p, &output, obj(&mut parsed), 0).unwrap();
        assert_eq!(parsed, values);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.imports = 0;
            s.fail_import = 2;
        });
        assert_eq!(
            import(&mut p, &wide("(5,6,7)"), obj(&mut values), 0),
            Err(73)
        );
        assert_eq!(values, [5, 0]);
        STATE.with(|s| assert_eq!(s.borrow().imports, 2));
    }
}
#[test]
fn generic_export_clears_before_matching_and_preserves_first_error() {
    reset();
    unsafe {
        let mut p = Node::new(Hp2TextKind::TextInt);
        let mut value = 5;
        let mut default = 5;
        let mut output = [88; 64];
        let result = run(
            p.kind,
            Hp2TextOperation::TextExport,
            obj(&mut p),
            output.as_mut_ptr().cast(),
            obj(&mut value),
            obj(&mut default),
            0,
            0,
            &SERVICES,
        )
        .unwrap();
        assert_eq!(result.1, 0);
        assert_eq!(output[0], 0);
        assert_eq!(output[1], 88);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls = 0;
            s.fail_call = 1;
        });
        output[0] = 88;
        assert_eq!(
            run(
                p.kind,
                Hp2TextOperation::TextExport,
                obj(&mut p),
                output.as_mut_ptr().cast(),
                obj(&mut value),
                obj(&mut default),
                0,
                0,
                &SERVICES
            ),
            Err(71)
        );
        assert_eq!(output[0], 0);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.fail_call = 0;
            s.calls = 0;
        });
        let same = obj(&mut value);
        run(
            p.kind,
            Hp2TextOperation::TextExport,
            obj(&mut p),
            output.as_mut_ptr().cast(),
            same,
            same,
            0,
            0,
            &SERVICES,
        )
        .unwrap();
        assert_eq!(text(output.as_ptr()), "5");
        STATE.with(|s| assert_eq!(s.borrow().calls, 1));
    }
}
#[test]
fn struct_inherited_fields_defaults_port_flags_and_live_next_are_preserved() {
    reset();
    unsafe {
        let mut first = Node::new(Hp2TextKind::TextInt);
        first.name = 1;
        let mut second = Node::new(Hp2TextKind::TextInt);
        second.name = 2;
        second.offset = 4;
        let mut base = Node::new(Hp2TextKind::TextGeneric);
        base.children = obj(&mut second);
        let mut owner = Node::new(Hp2TextKind::TextGeneric);
        owner.children = obj(&mut first);
        owner.parent = obj(&mut base);
        let mut p = Node::new(Hp2TextKind::TextStruct);
        p.structure = obj(&mut owner);
        let mut values = [3i32, 4];
        let mut defaults = [3i32, 0];
        let output = export(&mut p, obj(&mut values), obj(&mut defaults), 2);
        assert_eq!(text(output.as_ptr()), "(Second=4)");
        STATE.with(|s| assert_eq!(s.borrow().export_flags, [1]));
        let mut parsed = [8i32, 0];
        import(&mut p, &output, obj(&mut parsed), 0).unwrap();
        assert_eq!(parsed, [8, 4]);
        let mut all_defaults = [3i32, 4];
        let mut untouched = [88; 32];
        run(
            p.kind,
            Hp2TextOperation::TextExportItem,
            obj(&mut p),
            untouched.as_mut_ptr().cast(),
            obj(&mut values),
            obj(&mut all_defaults),
            0,
            0,
            &SERVICES,
        )
        .unwrap();
        assert_eq!(untouched, [88; 32]);
        let mut third = Node::new(Hp2TextKind::TextInt);
        third.name = 3;
        third.offset = 4;
        owner.parent = null_mut();
        STATE.with(|s| s.borrow_mut().replace_next = Some((obj(&mut first), obj(&mut third))));
        let output = export(&mut p, obj(&mut values), null_mut(), 0);
        assert_eq!(text(output.as_ptr()), "(First=3,Third=4)");
        second.flags = 4;
        let port = run(
            second.kind,
            Hp2TextOperation::TextPort,
            obj(&mut second),
            null_mut(),
            null_mut(),
            null_mut(),
            0,
            0,
            &SERVICES,
        )
        .unwrap();
        assert_eq!(port.1, 0);
        second.category = 1;
        assert_eq!(
            run(
                second.kind,
                Hp2TextOperation::TextPort,
                obj(&mut second),
                null_mut(),
                null_mut(),
                null_mut(),
                0,
                0,
                &SERVICES
            )
            .unwrap()
            .1,
            1
        );
    }
}
#[test]
fn struct_unknown_skip_and_first_error_leave_prior_fields_published() {
    reset();
    unsafe {
        let mut first = Node::new(Hp2TextKind::TextInt);
        let mut owner = Node::new(Hp2TextKind::TextGeneric);
        owner.children = obj(&mut first);
        let mut p = Node::new(Hp2TextKind::TextStruct);
        p.structure = obj(&mut owner);
        let mut value = 0i32;
        let source = wide("(First=7,unknown=(a,b),First=9)");
        assert!(
            !import(&mut p, &source, obj(&mut value), 0)
                .unwrap()
                .is_null()
        );
        assert_eq!(value, 9);
        STATE.with(|s| s.borrow_mut().warnings.clear());
        assert!(
            import(
                &mut p,
                &wide("(First=5,unknown=\"a,b\")"),
                obj(&mut value),
                0
            )
            .unwrap()
            .is_null()
        );
        assert_eq!(value, 5);
        STATE.with(|s| {
            assert_eq!(
                s.borrow().warnings,
                [
                    Hp2TextWarning::TextUnknownElement,
                    Hp2TextWarning::TextMissingKey
                ]
            )
        });
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.warnings.clear();
            s.fail_warning = true;
        });
        assert_eq!(
            import(&mut p, &wide("(unknown=(bad"), obj(&mut value), 0),
            Err(79)
        );
        STATE.with(|s| assert_eq!(s.borrow().warnings, [Hp2TextWarning::TextUnknownElement]));
    }
}
#[test]
fn object_lookup_cursor_and_class_rejection_preserve_mutation_order() {
    reset();
    unsafe {
        let mut class = Node::new(Hp2TextKind::TextClass);
        let mut found = Node::new(Hp2TextKind::TextObject);
        found.class = obj(&mut class);
        let mut p = Node::new(Hp2TextKind::TextObject);
        p.class = obj(&mut class);
        let found_pointer = obj(&mut found);
        let class_pointer = obj(&mut class);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.classes.push(("Kind".into(), class_pointer));
            s.objects
                .push((class_pointer, "Package.Item".into(), found_pointer));
        });
        let mut value = null_mut::<c_void>();
        let source = wide("Package.Item,tail");
        let end = import(&mut p, &source, obj(&mut value), 0).unwrap();
        assert_eq!(text(end), "tail");
        assert_eq!(value, found_pointer);
        let source = wide("Kind'Package.Item',tail");
        let end = import(&mut p, &source, obj(&mut value), 0).unwrap();
        assert_eq!(*end, 44);
        assert_eq!(value, found_pointer);
        assert!(
            import(&mut p, &wide("Missing'Package.Item'"), obj(&mut value), 0)
                .unwrap()
                .is_null()
        );
        assert_eq!(value, found_pointer);
        assert!(
            import(&mut p, &wide("Kind'Missing'"), obj(&mut value), 0)
                .unwrap()
                .is_null()
        );
        assert!(value.is_null());
        p.kind = Hp2TextKind::TextClass;
        p.meta = class_pointer;
        let source = wide("Kind'Package.Item'");
        assert!(
            !import(&mut p, &source, obj(&mut value), 0)
                .unwrap()
                .is_null()
        );
        assert!(value.is_null());
        let source = wide("None,");
        assert_eq!(*import(&mut p, &source, obj(&mut value), 0).unwrap(), 44);
        assert!(value.is_null());
    }
}
#[test]
fn cpp_type_composition_and_general_declarator_policy() {
    reset();
    unsafe {
        let mut p = Node::new(Hp2TextKind::TextBool);
        let mut out = String::new();
        run(
            p.kind,
            Hp2TextOperation::TextCpp,
            obj(&mut p),
            obj(&mut out),
            null_mut(),
            null_mut(),
            0,
            0,
            &SERVICES,
        )
        .unwrap();
        assert_eq!(out, "BITFIELD First:1");
        p.kind = Hp2TextKind::TextString;
        out.clear();
        run(
            p.kind,
            Hp2TextOperation::TextCpp,
            obj(&mut p),
            obj(&mut out),
            null_mut(),
            null_mut(),
            0,
            1,
            &SERVICES,
        )
        .unwrap();
        assert_eq!(out, "const FString& First");
        p.dim = 3;
        out.clear();
        run(
            p.kind,
            Hp2TextOperation::TextCpp,
            obj(&mut p),
            obj(&mut out),
            null_mut(),
            null_mut(),
            0,
            0,
            &SERVICES,
        )
        .unwrap();
        assert_eq!(out, "FStringNoInit First[3]");
        let mut key = Node::new(Hp2TextKind::TextName);
        let mut value = Node::new(Hp2TextKind::TextArray);
        value.inner = obj(&mut p);
        let mut map = Node::new(Hp2TextKind::TextMap);
        map.key = obj(&mut key);
        map.value = obj(&mut value);
        out.clear();
        run(
            map.kind,
            Hp2TextOperation::TextCppItem,
            obj(&mut map),
            obj(&mut out),
            null_mut(),
            null_mut(),
            0,
            0,
            &SERVICES,
        )
        .unwrap();
        assert_eq!(out, "TMap<FName,TArray<FString>>");
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls = 0;
            s.fail_call = 3;
        });
        out.clear();
        assert_eq!(
            run(
                map.kind,
                Hp2TextOperation::TextCppItem,
                obj(&mut map),
                obj(&mut out),
                null_mut(),
                null_mut(),
                0,
                0,
                &SERVICES
            ),
            Err(71)
        );
        assert_eq!(out, "TMap<FName,");
    }
}

#[test]
fn array_parse_failure_and_export_reentry_use_live_storage() {
    reset();
    unsafe {
        let mut inner = Node::new(Hp2TextKind::TextBool);
        let mut p = Node::new(Hp2TextKind::TextArray);
        p.inner = obj(&mut inner);
        let mut values = vec![99i32];
        assert!(
            import(&mut p, &wide("(True,invalid,False)"), obj(&mut values), 0)
                .unwrap()
                .is_null()
        );
        assert_eq!(values, [4, 0]);
        STATE.with(|s| assert_eq!(s.borrow().imports, 2));
        inner.kind = Hp2TextKind::TextInt;
        values = vec![1, 2, 3];
        STATE.with(|s| s.borrow_mut().shrink_array = Some(obj(&mut values)));
        let output = export(&mut p, obj(&mut values), null_mut(), 0);
        assert_eq!(text(output.as_ptr()), "(1)");
        assert_eq!(values, [1]);
    }
}

#[test]
fn struct_array_indices_and_empty_index_preserve_original_policy() {
    reset();
    unsafe {
        let mut field = Node::new(Hp2TextKind::TextInt);
        field.dim = 2;
        let mut owner = Node::new(Hp2TextKind::TextGeneric);
        owner.children = obj(&mut field);
        let mut p = Node::new(Hp2TextKind::TextStruct);
        p.structure = obj(&mut owner);
        let mut values = [0i32; 2];
        let source = wide("(First[1]=8,First[]=3,First[9]=77)");
        assert!(
            !import(&mut p, &source, obj(&mut values), 0)
                .unwrap()
                .is_null()
        );
        assert_eq!(values, [3, 8]);
        STATE.with(|s| assert_eq!(s.borrow().warnings, [Hp2TextWarning::TextUnknownElement]));
        let output = export(&mut p, obj(&mut values), null_mut(), 0);
        assert_eq!(text(output.as_ptr()), "(First[0]=3,First[1]=8)");
        let mut parsed = [0i32; 2];
        import(&mut p, &output, obj(&mut parsed), 0).unwrap();
        assert_eq!(parsed, values);
        assert!(
            import(&mut p, &wide("(First[-1]=4)"), obj(&mut parsed), 0)
                .unwrap()
                .is_null()
        );
        assert_eq!(parsed, values);
        STATE.with(|s| {
            assert_eq!(
                s.borrow().warnings.last(),
                Some(&Hp2TextWarning::TextArrayElement)
            )
        });
    }
}

#[test]
fn numeric_range_failure_precedes_assignment_and_array_continuation() {
    reset();
    unsafe {
        let mut integer = Node::new(Hp2TextKind::TextInt);
        let mut value = 17i32;
        assert_eq!(
            import(&mut integer, &wide("2147483648,"), obj(&mut value), 0),
            Err(1)
        );
        assert_eq!(value, 17);
        STATE.with(|s| assert_eq!(s.borrow().numeric_asserts, [2213]));
        let mut array = Node::new(Hp2TextKind::TextArray);
        array.inner = obj(&mut integer);
        let mut values = vec![99i32];
        assert_eq!(
            import(&mut array, &wide("(4,-2147483649,6)"), obj(&mut values), 0),
            Err(1)
        );
        assert_eq!(values, [4, 0]);
        STATE.with(|s| {
            let s = s.borrow();
            assert_eq!(s.imports, 2);
            assert_eq!(s.numeric_asserts, [2213, 2213]);
        });
    }
}
