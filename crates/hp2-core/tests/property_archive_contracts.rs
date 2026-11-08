use Hp2PropertyArchiveInteger::*;
use Hp2PropertyArchiveName::*;
use Hp2PropertyArchivePointer::*;
use Hp2PropertyArchiveWarning::*;
use hp2_core::property_archive::{self as archive, *};
use std::{ffi::c_void, ptr};
type P = *mut c_void;
struct Field {
    name: i32,
    kind: i32,
    property: bool,
    dim: i32,
    offset: i32,
    size: i32,
    children: P,
    next: P,
    super_: P,
    link: P,
    link_next: P,
    skip: bool,
    symbol: Option<Hp2PropertyArchiveName>,
    struct_name: i32,
    shrink_on_item: bool,
    next_on_item: P,
    move_on_count: bool,
}
impl Field {
    fn new(name: i32, offset: i32) -> Self {
        Self {
            name,
            kind: 2,
            property: true,
            dim: 1,
            offset,
            size: 4,
            children: ptr::null_mut(),
            next: ptr::null_mut(),
            super_: ptr::null_mut(),
            link: ptr::null_mut(),
            link_next: ptr::null_mut(),
            skip: false,
            symbol: None,
            struct_name: 99,
            shrink_on_item: false,
            next_on_item: ptr::null_mut(),
            move_on_count: false,
        }
    }
}
struct Context {
    loading: bool,
    version: i32,
    // Native FPropertyArchiveContext stores a BYTE* to object data, not inline bytes.
    data: Box<[u8; 64]>,
    defaults: Vec<u8>,
    defaults_error: i32,
    wire: Vec<u8>,
    pos: usize,
    name: i32,
    index: i32,
    warnings: Vec<Hp2PropertyArchiveWarning>,
    fail_at: Option<usize>,
    item_error: i32,
    strings: Vec<(i32, String)>,
}
impl Context {
    fn new(loading: bool) -> Self {
        Self {
            loading,
            version: 79,
            data: Box::new([0; 64]),
            defaults: vec![],
            defaults_error: 0,
            wire: vec![],
            pos: 0,
            name: 0,
            index: -1,
            warnings: vec![],
            fail_at: None,
            item_error: 0,
            strings: vec![],
        }
    }
    fn p(&mut self) -> P {
        (self as *mut Self).cast()
    }
    fn value(&self, index: usize) -> i32 {
        i32::from_le_bytes(self.data[index * 4..index * 4 + 4].try_into().unwrap())
    }
    fn put(&mut self, index: usize, value: i32) {
        self.data[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn byte(&mut self) -> std::result::Result<u8, i32> {
        if self.fail_at == Some(self.pos) {
            return Err(73);
        }
        let b = self.wire[self.pos];
        self.pos += 1;
        Ok(b)
    }
    fn word(&mut self) -> std::result::Result<i32, i32> {
        let mut b = [0; 4];
        for v in &mut b {
            *v = self.byte()?;
        }
        Ok(i32::from_le_bytes(b))
    }
}
struct Fixture {
    fields: Vec<Box<Field>>,
    context: Context,
}
impl Fixture {
    fn new(loading: bool) -> Self {
        Self {
            fields: vec![],
            context: Context::new(loading),
        }
    }
    fn field(&mut self, name: i32, offset: i32) -> P {
        let mut f = Box::new(Field::new(name, offset));
        let p = (&mut *f as *mut Field).cast();
        self.fields.push(f);
        p
    }
    fn root(&mut self, children: P) -> P {
        let r = self.field(500, 0);
        unsafe {
            (*r.cast::<Field>()).property = false;
            (*r.cast::<Field>()).children = children;
            (*r.cast::<Field>()).link = children;
        }
        r
    }
    fn tagged(&mut self, root: P, persistent: bool) -> std::result::Result<(), i32> {
        let data = self.context.data.as_mut_ptr().cast();
        unsafe { archive::tagged(root, self.context.p(), data, persistent, &SERVICES) }
    }
    fn binary(&mut self, root: P) -> std::result::Result<(), i32> {
        let data = self.context.data.as_mut_ptr().cast();
        unsafe { archive::binary(root, self.context.p(), data, &SERVICES) }
    }
}
fn empty_tag() -> Hp2PropertyArchiveTag {
    Hp2PropertyArchiveTag {
        property_type: 0,
        info: 0,
        name: 0,
        item_name: 0,
        size: 0,
        array_index: 0,
    }
}
fn tag(name: i32, kind: u8, size: i32, index: i32) -> Hp2PropertyArchiveTag {
    Hp2PropertyArchiveTag {
        name,
        property_type: kind,
        size,
        array_index: index,
        ..empty_tag()
    }
}
fn header(out: &mut Vec<u8>, t: &Hp2PropertyArchiveTag) {
    out.extend(t.name.to_le_bytes());
    if t.name != 0 {
        out.push(t.property_type);
        out.extend(t.item_name.to_le_bytes());
        out.extend(t.size.to_le_bytes());
        out.extend(t.array_index.to_le_bytes());
    }
}
fn record(out: &mut Vec<u8>, t: &Hp2PropertyArchiveTag, bytes: &[u8]) {
    header(out, t);
    out.extend(bytes);
}
fn end(out: &mut Vec<u8>) {
    header(out, &empty_tag());
}
unsafe extern "C" fn pointer(p: P, field: Hp2PropertyArchivePointer, out: *mut P) -> i32 {
    unsafe {
        let f = &*p.cast::<Field>();
        *out = match field {
            ArchiveChildren => f.children,
            ArchiveNext => f.next,
            ArchiveSuper => f.super_,
            ArchivePropertyLink => f.link,
            ArchivePropertyLinkNext => f.link_next,
        };
    }
    0
}
unsafe extern "C" fn integer(p: P, field: Hp2PropertyArchiveInteger) -> i32 {
    unsafe {
        match field {
            ArchiveVersion => (*p.cast::<Context>()).version,
            ArchiveLoading => i32::from((*p.cast::<Context>()).loading),
            _ => {
                let f = &*p.cast::<Field>();
                match field {
                    ArchiveName => f.name,
                    ArchiveIsProperty => i32::from(f.property),
                    ArchiveArrayDim => f.dim,
                    ArchiveOffset => f.offset,
                    ArchiveElementSize => f.size,
                    ArchiveType => f.kind,
                    _ => unreachable!(),
                }
            }
        }
    }
}
unsafe extern "C" fn should(p: P, _: P) -> i32 {
    i32::from(unsafe { !(*p.cast::<Field>()).skip })
}
unsafe extern "C" fn names(p: P, n: Hp2PropertyArchiveName, out: *mut i32) -> i32 {
    unsafe {
        *out = i32::from((*p.cast::<Field>()).symbol.map(|v| v as u32) == Some(n as u32));
    }
    0
}
unsafe extern "C" fn struct_name(p: P, out: *mut i32) -> i32 {
    unsafe {
        *out = (*p.cast::<Field>()).struct_name;
    }
    0
}
unsafe extern "C" fn defaults(p: P, out: *mut P, count: *mut i32) -> i32 {
    unsafe {
        let c = &mut *p.cast::<Context>();
        if c.defaults_error != 0 {
            return c.defaults_error;
        }
        *out = if c.defaults.is_empty() {
            ptr::null_mut()
        } else {
            c.defaults.as_mut_ptr().cast()
        };
        *count = c.defaults.len() as i32;
    }
    0
}
unsafe extern "C" fn note_name(p: P, n: i32) {
    unsafe {
        (*p.cast::<Context>()).name = n;
    }
}
unsafe extern "C" fn note_index(p: P, n: i32) {
    unsafe {
        (*p.cast::<Context>()).index = n;
    }
}
unsafe extern "C" fn item(context: P, property: P, value: P) -> i32 {
    unsafe {
        let c = &mut *context.cast::<Context>();
        if c.item_error != 0 {
            return c.item_error;
        }
        if c.loading {
            let word = match c.word() {
                Ok(v) => v,
                Err(e) => return e,
            };
            ptr::write_unaligned(value.cast::<i32>(), word);
        } else {
            c.wire
                .extend(ptr::read_unaligned(value.cast::<i32>()).to_le_bytes());
        }
        let f = &mut *property.cast::<Field>();
        if f.shrink_on_item {
            f.dim = 1;
        }
        if !f.next_on_item.is_null() {
            f.next = f.next_on_item;
        }
    }
    0
}
unsafe extern "C" fn matches(property: P, data: P, default: P, index: i32, out: *mut i32) -> i32 {
    unsafe {
        let f = &*property.cast::<Field>();
        let off = f.offset + index * f.size;
        let a = ptr::read_unaligned(data.cast::<u8>().offset(off as isize).cast::<i32>());
        let b = if default.is_null() {
            0
        } else {
            ptr::read_unaligned(default.cast::<u8>().offset(off as isize).cast::<i32>())
        };
        *out = i32::from(a == b);
    }
    0
}
unsafe extern "C" fn stream(
    context: P,
    _: i32,
    body: Hp2PropertyArchiveContinuation,
    user: P,
) -> i32 {
    unsafe { body(context, user) }
}
struct TagContext {
    root: P,
    tag: Hp2PropertyArchiveTag,
}
unsafe extern "C" fn load_scope(root: P, body: Hp2PropertyArchiveContinuation, user: P) -> i32 {
    let mut c = TagContext {
        root,
        tag: empty_tag(),
    };
    unsafe { body((&raw mut c).cast(), user) }
}
unsafe extern "C" fn save_scope(
    root: P,
    p: P,
    index: i32,
    _: P,
    body: Hp2PropertyArchiveContinuation,
    user: P,
) -> i32 {
    unsafe {
        let f = p.cast::<Field>();
        let mut c = TagContext {
            root,
            tag: tag((*f).name, (*f).kind as u8, 4, index),
        };
        if (*f).move_on_count {
            (*f).offset += 4;
        }
        body((&raw mut c).cast(), user)
    }
}
unsafe extern "C" fn read_tag(context: P, out: *mut *const Hp2PropertyArchiveTag) -> i32 {
    unsafe {
        let t = &mut *context.cast::<TagContext>();
        let c = &mut *t.root.cast::<Context>();
        macro_rules! get {
            ($e:expr) => {
                match $e {
                    Ok(v) => v,
                    Err(e) => return e,
                }
            };
        }
        t.tag.name = get!(c.word());
        if t.tag.name != 0 {
            t.tag.property_type = get!(c.byte());
            t.tag.item_name = get!(c.word());
            t.tag.size = get!(c.word());
            t.tag.array_index = get!(c.word());
        }
        *out = &raw const t.tag;
    }
    0
}
unsafe extern "C" fn write_tag(context: P) -> i32 {
    unsafe {
        let t = &*context.cast::<TagContext>();
        header(&mut (*t.root.cast::<Context>()).wire, &t.tag);
    }
    0
}
unsafe extern "C" fn tagged_item(context: P, p: P, value: P) -> i32 {
    unsafe { item((*context.cast::<TagContext>()).root, p, value) }
}
unsafe extern "C" fn read_ansi_byte(context: P, out: *mut u8) -> i32 {
    unsafe {
        let c = &mut *(*context.cast::<TagContext>()).root.cast::<Context>();
        match c.byte() {
            Ok(v) => {
                *out = v;
                0
            }
            Err(e) => e,
        }
    }
}
unsafe extern "C" fn assign_fixed_string(context: P, p: P, text: *const i32) -> i32 {
    unsafe {
        let t = &*context.cast::<TagContext>();
        let c = &mut *t.root.cast::<Context>();
        let f = &*p.cast::<Field>();
        let mut s = String::new();
        let mut i = 0isize;
        loop {
            let ch = *text.offset(i);
            if ch == 0 {
                break;
            }
            s.push(char::from_u32(ch as u32).unwrap());
            i += 1;
        }
        c.strings
            .push((f.offset + t.tag.array_index * f.size, s));
    }
    0
}
unsafe extern "C" fn warn(context: P, _: P, w: Hp2PropertyArchiveWarning) -> i32 {
    unsafe {
        (*(*context.cast::<TagContext>()).root.cast::<Context>())
            .warnings
            .push(w);
    }
    0
}
unsafe extern "C" fn is_xlevel(context: P) -> i32 {
    i32::from(unsafe { (*context.cast::<TagContext>()).tag.name == 777 })
}
unsafe extern "C" fn read_byte(context: P) -> i32 {
    unsafe { (*context.cast::<Context>()).byte().err().unwrap_or(0) }
}
unsafe extern "C" fn end_names(context: P) -> i32 {
    unsafe {
        end(&mut (*context.cast::<Context>()).wire);
    }
    0
}
const SERVICES: Hp2PropertyArchiveServices = Hp2PropertyArchiveServices {
    string_property: 7,
    str_property: 13,
    struct_property: 10,
    pointer,
    integer,
    should_serialize: should,
    name_matches: names,
    struct_name,
    defaults,
    note_name,
    note_index,
    serialize_item: item,
    matches,
    stream,
    load_tag_scope: load_scope,
    save_tag_scope: save_scope,
    read_tag,
    write_tag,
    tagged_item,
    read_ansi_byte,
    assign_fixed_string,
    warn,
    is_xlevel,
    read_byte,
    end_names,
};

#[test]
fn binary_walks_live_children_then_inheritance_and_obeys_mutated_array_bounds() {
    let mut f = Fixture::new(true);
    let first = f.field(1, 0);
    let ignored = f.field(90, 0);
    let replaced = f.field(2, 4);
    let live = f.field(3, 8);
    let inherited = f.field(4, 12);
    let parent = f.root(inherited);
    let root = f.root(first);
    unsafe {
        (*first.cast::<Field>()).dim = 3;
        (*first.cast::<Field>()).shrink_on_item = true;
        (*first.cast::<Field>()).next = ignored;
        (*first.cast::<Field>()).next_on_item = live;
        (*ignored.cast::<Field>()).property = false;
        (*ignored.cast::<Field>()).next = replaced;
        (*root.cast::<Field>()).super_ = parent;
    }
    f.context.wire = [11i32, 33, 44]
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect();
    assert_eq!(f.binary(root), Ok(()));
    assert_eq!(
        [
            f.context.value(0),
            f.context.value(1),
            f.context.value(2),
            f.context.value(3)
        ],
        [11, 0, 33, 44]
    );
    assert_eq!((f.context.name, f.context.index, f.context.pos), (4, 1, 12));
}
#[test]
fn binary_consumes_obsolete_max_before_skipped_rand_only_before_version_76() {
    for version in [75, 76] {
        let mut f = Fixture::new(true);
        let rand = f.field(9, 0);
        let root = f.root(rand);
        unsafe {
            (*root.cast::<Field>()).symbol = Some(ArchiveColorParams);
            (*rand.cast::<Field>()).symbol = Some(ArchiveRand);
            (*rand.cast::<Field>()).skip = true;
        }
        f.context.version = version;
        f.context.wire = 123i32.to_le_bytes().to_vec();
        assert_eq!(f.binary(root), Ok(()));
        assert_eq!(
            (f.context.value(0), f.context.pos),
            if version == 75 { (123, 4) } else { (0, 0) }
        );
    }
}
#[test]
fn binary_error_keeps_completed_elements_and_current_diagnostic_index() {
    let mut f = Fixture::new(true);
    let p = f.field(21, 0);
    let root = f.root(p);
    unsafe {
        (*p.cast::<Field>()).dim = 3;
    }
    f.context.wire = [5i32, 6, 7]
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect();
    f.context.fail_at = Some(6);
    assert_eq!(f.binary(root), Err(73));
    assert_eq!([f.context.value(0), f.context.value(1)], [5, 0]);
    assert_eq!((f.context.name, f.context.index, f.context.pos), (21, 1, 6));
}
#[test]
fn normal_save_suppresses_defaults_and_uses_null_default_for_missing_array_tail() {
    let mut f = Fixture::new(false);
    let p = f.field(1, 0);
    let root = f.root(p);
    unsafe {
        (*p.cast::<Field>()).dim = 2;
    }
    f.context.put(0, 11);
    f.context.put(1, 22);
    f.context.defaults = 11i32.to_le_bytes().to_vec();
    assert_eq!(f.tagged(root, false), Ok(()));
    let mut expected = vec![];
    record(&mut expected, &tag(1, 2, 4, 1), &22i32.to_le_bytes());
    end(&mut expected);
    assert_eq!(f.context.wire, expected);
    assert_eq!(f.context.index, 2);
}
#[test]
fn persistence_excludes_auxanims_but_saves_other_values_even_equal_to_defaults() {
    let mut f = Fixture::new(false);
    let aux = f.field(1, 0);
    let other = f.field(2, 4);
    let root = f.root(aux);
    unsafe {
        (*aux.cast::<Field>()).symbol = Some(ArchiveAuxAnims);
        (*aux.cast::<Field>()).next = other;
    }
    f.context.put(0, 91);
    f.context.put(1, 17);
    f.context.defaults = [91i32, 17].into_iter().flat_map(i32::to_le_bytes).collect();
    assert_eq!(f.tagged(root, true), Ok(()));
    let mut expected = vec![];
    record(&mut expected, &tag(2, 2, 4, 0), &17i32.to_le_bytes());
    end(&mut expected);
    assert_eq!(f.context.wire, expected);
    assert_eq!((f.context.name, f.context.index), (2, 1));
}
#[test]
fn save_keeps_precomputed_value_address_when_counting_changes_property_offset() {
    let mut f = Fixture::new(false);
    let p = f.field(1, 0);
    let root = f.root(p);
    unsafe {
        (*p.cast::<Field>()).move_on_count = true;
    }
    f.context.put(0, 17);
    f.context.put(1, 99);
    assert_eq!(f.tagged(root, false), Ok(()));
    let mut expected = vec![];
    record(&mut expected, &tag(1, 2, 4, 0), &17i32.to_le_bytes());
    end(&mut expected);
    assert_eq!(f.context.wire, expected);
}
#[test]
fn load_skips_each_invalid_tag_without_losing_next_valid_property() {
    let mut f = Fixture::new(true);
    let p = f.field(1, 0);
    let structure = f.field(2, 4);
    let skipped = f.field(3, 8);
    let root = f.root(p);
    unsafe {
        (*p.cast::<Field>()).link_next = structure;
        (*structure.cast::<Field>()).kind = 10;
        (*structure.cast::<Field>()).link_next = skipped;
        (*skipped.cast::<Field>()).skip = true;
    }
    record(&mut f.context.wire, &tag(100, 2, 2, 0), &[9, 8]);
    record(&mut f.context.wire, &tag(1, 4, 1, 0), &[7]);
    record(&mut f.context.wire, &tag(1, 2, 1, 1), &[6]);
    let mut mismatch = tag(2, 10, 1, 0);
    mismatch.item_name = 98;
    record(&mut f.context.wire, &mismatch, &[5]);
    record(&mut f.context.wire, &tag(3, 2, 1, 0), &[4]);
    record(&mut f.context.wire, &tag(1, 2, 4, 0), &55i32.to_le_bytes());
    end(&mut f.context.wire);
    assert_eq!(f.tagged(root, false), Ok(()));
    assert_eq!(f.context.value(0), 55);
    assert_eq!(
        f.context.warnings,
        [
            ArchiveMissingProperty,
            ArchiveSkipping,
            ArchiveTypeMismatch,
            ArchiveSkipping,
            ArchiveArrayBounds,
            ArchiveSkipping,
            ArchiveStructMismatch,
            ArchiveSkipping,
            ArchiveNotSerializable,
            ArchiveSkipping
        ]
    );
    assert_eq!(f.context.pos, f.context.wire.len());
}
#[test]
fn xlevel_suppresses_nonserializable_and_skip_warnings_but_not_missing_property() {
    for present in [false, true] {
        let mut f = Fixture::new(true);
        let p = f.field(777, 0);
        let root = f.root(if present { p } else { ptr::null_mut() });
        unsafe {
            (*p.cast::<Field>()).skip = true;
        }
        record(&mut f.context.wire, &tag(777, 2, 3, 0), &[1, 2, 3]);
        end(&mut f.context.wire);
        assert_eq!(f.tagged(root, false), Ok(()));
        assert_eq!(
            f.context.warnings,
            if present {
                vec![]
            } else {
                vec![ArchiveMissingProperty]
            }
        );
        assert_eq!(f.context.value(0), 0);
    }
}
#[test]
fn fixed_string_upgrade_precedes_serializability_and_array_bounds_and_stops_at_null() {
    let mut f = Fixture::new(true);
    let text = f.field(1, 0);
    let p = f.field(2, 4);
    let root = f.root(text);
    unsafe {
        (*text.cast::<Field>()).kind = 13;
        (*text.cast::<Field>()).skip = true;
        (*text.cast::<Field>()).dim = 0;
        (*text.cast::<Field>()).link_next = p;
    }
    record(&mut f.context.wire, &tag(1, 7, 8, 3), b"ab\0");
    record(&mut f.context.wire, &tag(2, 2, 4, 0), &61i32.to_le_bytes());
    end(&mut f.context.wire);
    assert_eq!(f.tagged(root, false), Ok(()));
    assert_eq!(f.context.strings, [(12, "ab".into())]);
    assert_eq!(f.context.value(1), 61);
    assert_eq!(f.context.warnings, []);
}
#[test]
fn load_skip_error_stops_at_exact_archive_position_and_preserves_tag_diagnostic() {
    let mut f = Fixture::new(true);
    let root = f.root(ptr::null_mut());
    record(&mut f.context.wire, &tag(32, 2, 4, 0), &[1, 2, 3, 4]);
    end(&mut f.context.wire);
    f.context.fail_at = Some(19);
    assert_eq!(f.tagged(root, false), Err(73));
    assert_eq!(
        (f.context.pos, f.context.name, f.context.index),
        (19, 32, -1)
    );
    assert_eq!(
        f.context.warnings,
        [ArchiveMissingProperty, ArchiveSkipping]
    );
}
#[test]
fn defaults_failure_precedes_tag_read_on_load_and_save() {
    for loading in [true, false] {
        let mut f = Fixture::new(loading);
        let root = f.root(ptr::null_mut());
        f.context.defaults_error = 37;
        assert_eq!(f.tagged(root, true), Err(37));
        assert_eq!((f.context.pos, f.context.name, f.context.index), (0, 0, -1));
        assert_eq!(f.context.wire, []);
    }
}
#[test]
fn payload_error_does_not_write_terminator_or_advance_array_index() {
    let mut f = Fixture::new(false);
    let p = f.field(1, 0);
    let root = f.root(p);
    f.context.put(0, 2);
    f.context.item_error = 67;
    assert_eq!(f.tagged(root, false), Err(67));
    let mut expected = vec![];
    header(&mut expected, &tag(1, 2, 4, 0));
    assert_eq!(f.context.wire, expected);
    assert_eq!((f.context.name, f.context.index), (1, 0));
}
