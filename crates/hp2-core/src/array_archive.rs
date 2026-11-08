//! Array archive, transaction, lazy ownership, and memory-buffer policy.
//!
//! Native callbacks retain only virtual/typed dispatch, raw storage operations,
//! and the original check/checkSlow sites. Every callback status is propagated
//! immediately: in particular a failed lazy load does not restore the cursor or
//! negate its saved position. No Rust references to native objects span calls.

use std::ffi::c_void;
use std::ptr;

pub type Hp2ArrayArchiveCallback =
    unsafe extern "C" fn(*mut c_void, *mut c_void, i32, i32, *mut c_void) -> i32;

// Kept in lockstep with EHp2ArrayArchiveOperation in UnArc.h.
pub const LOADING: i32 = 1;
pub const SAVING: i32 = 2;
pub(crate) const TRANS: i32 = 3;
pub(crate) const VERSION: i32 = 4;
pub(crate) const TELL: i32 = 5;
pub(crate) const SEEK: i32 = 6;
pub const COMPACT: i32 = 7;
pub(crate) const INTEGER: i32 = 8;
pub(crate) const LEGACY_CHECK: i32 = 9;
pub(crate) const CHECKED_SIZE: i32 = 10;
pub const COUNT_BYTES: i32 = 20;
pub const NUM: i32 = 21;
pub const COUNT_INDEX: i32 = 22;
pub const SET_EXACT: i32 = 23;
pub const RAW_SERIALIZE: i32 = 24;
pub const DESTRUCT: i32 = 25;
pub const EMPTY_STORAGE: i32 = 26;
pub const CONSTRUCT_SERIALIZE: i32 = 27;
pub const ITEM_SERIALIZE: i32 = 28;
pub const FAST: i32 = 29;
pub const NEEDS_DESTRUCTOR: i32 = 30;
const REMOVE_CHECK: i32 = 31;
const REMOVE_STORAGE: i32 = 32;
pub(crate) const SAVED_ARCHIVE: i32 = 40;
pub(crate) const SAVED_POS: i32 = 41;
pub(crate) const SET_SAVED_POS: i32 = 42;
pub(crate) const ATTACH: i32 = 43;
pub(crate) const DETACH: i32 = 44;
pub(crate) const BASE_SERIALIZE: i32 = 45;
pub(crate) const LAZY_ENABLED: i32 = 46;
pub(crate) const LOAD: i32 = 47;
pub(crate) const UNDO_EXISTS: i32 = 50;
pub(crate) const SAVE_ARRAY: i32 = 51;
const ADD: i32 = 52;
const INSERT: i32 = 53;
const REMOVE: i32 = 54;
pub(crate) const EMPTY: i32 = 55;
const ZERO_RANGE: i32 = 56;
pub(crate) const BUFFER_NUM: i32 = 60;
pub(crate) const BUFFER_GROW: i32 = 61;
pub(crate) const WRITE_ONE: i32 = 62;
pub(crate) const WRITE_MANY: i32 = 63;
pub(crate) const READ_ONE: i32 = 64;
pub(crate) const READ_MANY: i32 = 65;
pub(crate) const READ_CHECK: i32 = 66;
pub(crate) const SEEK_CHECK: i32 = 67;
pub(crate) const POSITION: i32 = 68;
pub(crate) const SET_POSITION: i32 = 69;

// EHp2ArrayArchiveAction in UnArc.h. These IDs are an internal service ABI.
pub mod action {
    pub const ARRAY: i32 = 0;
    pub(super) const REFERENCE: i32 = 1;
    pub(super) const TRANSACTION: i32 = 2;
    pub(crate) const LAZY_SERIALIZE: i32 = 3;
    pub(crate) const LAZY_LOAD: i32 = 4;
    pub(crate) const LAZY_UNLOAD: i32 = 5;
    pub(crate) const LAZY_DESTROY: i32 = 6;
    pub(super) const ARRAY_EMPTY: i32 = 7;
    pub(super) const ARRAY_REMOVE: i32 = 8;
    pub(super) const TRANSACTION_ADD: i32 = 9;
    pub(super) const TRANSACTION_INSERT: i32 = 10;
    pub(super) const TRANSACTION_REMOVE: i32 = 11;
    pub(super) const TRANSACTION_EMPTY: i32 = 12;
    pub(super) const TRANSACTION_MODIFY: i32 = 13;
    pub(super) const TRANSACTION_MODIFY_ALL: i32 = 14;
    pub(crate) const BUFFER_WRITE: i32 = 15;
    pub(crate) const BUFFER_READ: i32 = 16;
    pub(crate) const BUFFER_TELL: i32 = 17;
    pub(crate) const BUFFER_SIZE: i32 = 18;
    pub(crate) const BUFFER_SEEK_WRITE: i32 = 19;
    pub(crate) const BUFFER_SEEK_READ: i32 = 20;
    pub(crate) const BUFFER_AT_END: i32 = 21;
    pub(super) const TRANSACTION_ADD_ZEROED: i32 = 22;
}

#[repr(C)]
struct Range {
    index: i32,
    count: i32,
    operation: i32,
}

#[repr(C)]
pub(crate) struct CheckedSize {
    pub(crate) size: usize,
    pub(crate) result: i32,
}

#[derive(Clone, Copy)]
struct Native {
    object: *mut c_void,
    archive: *mut c_void,
    callback: Hp2ArrayArchiveCallback,
}

impl Native {
    // All callers are inside run's unsafe native-object/service contract.
    fn call(self, op: i32, arg: i32, value: *mut c_void) -> Result<(), i32> {
        let status = unsafe { (self.callback)(self.object, self.archive, op, arg, value) };
        if status == 0 { Ok(()) } else { Err(status) }
    }

    fn command(self, op: i32, arg: i32) -> Result<(), i32> {
        self.call(op, arg, ptr::null_mut())
    }

    fn value(self, op: i32) -> Result<i32, i32> {
        let mut value = 0;
        self.call(op, 0, (&mut value as *mut i32).cast())?;
        Ok(value)
    }

    fn integer(self, op: i32, value: &mut i32) -> Result<(), i32> {
        self.call(op, 0, (value as *mut i32).cast())
    }

    fn saved(self) -> Result<Self, i32> {
        let mut archive: *mut c_void = ptr::null_mut();
        self.call(SAVED_ARCHIVE, 0, (&mut archive as *mut *mut c_void).cast())?;
        Ok(Self { archive, ..self })
    }
}

fn empty(native: Native, slack: i32) -> Result<(), i32> {
    if native.value(NEEDS_DESTRUCTOR)? != 0 {
        let mut index = 0i32;
        while index < native.value(NUM)? {
            native.command(DESTRUCT, index)?;
            index = index.wrapping_add(1);
        }
    }
    native.command(EMPTY_STORAGE, slack)
}

fn remove(native: Native, index: i32, mut count: i32) -> Result<(), i32> {
    native.call(REMOVE_CHECK, index, (&mut count as *mut i32).cast())?;
    if native.value(NEEDS_DESTRUCTOR)? != 0 {
        let mut item = index;
        while item < index.wrapping_add(count) {
            native.command(DESTRUCT, item)?;
            item = item.wrapping_add(1);
        }
    }
    native.call(REMOVE_STORAGE, index, (&mut count as *mut i32).cast())
}

fn serialize(native: Native) -> Result<(), i32> {
    native.command(COUNT_BYTES, 0)?;
    if native.value(FAST)? != 0 {
        native.command(COUNT_INDEX, 0)?;
        if native.value(LOADING)? != 0 {
            native.command(SET_EXACT, 0)?;
        }
        if native.value(NUM)? > 0 {
            native.command(RAW_SERIALIZE, 0)?;
        }
    } else if native.value(LOADING)? != 0 {
        let mut count = 0;
        native.integer(COMPACT, &mut count)?;
        empty(native, count)?;
        for _ in 0..count {
            native.command(CONSTRUCT_SERIALIZE, 0)?;
        }
    } else {
        native.command(COUNT_INDEX, 0)?;
        let mut index = 0i32;
        while index < native.value(NUM)? {
            native.command(ITEM_SERIALIZE, index)?;
            index = index.wrapping_add(1);
        }
    }
    Ok(())
}

fn lazy_load(native: Native) -> Result<(), i32> {
    if native.value(SAVED_POS)? > 0 {
        let pushed = native.saved()?.value(TELL)?;
        let archive = native.saved()?;
        archive.command(SEEK, native.value(SAVED_POS)?)?;
        native.saved()?.command(BASE_SERIALIZE, 0)?;
        native.command(SET_SAVED_POS, native.value(SAVED_POS)?.wrapping_neg())?;
        native.saved()?.command(SEEK, pushed)?;
    }
    Ok(())
}

fn lazy_unload(native: Native, gnu: bool) -> Result<(), i32> {
    if native.value(SAVED_POS)? < 0 && (!gnu || native.value(LAZY_ENABLED)? != 0) {
        native.command(EMPTY, 0)?;
        native.command(SET_SAVED_POS, native.value(SAVED_POS)?.wrapping_neg())?;
    }
    Ok(())
}

fn lazy_serialize(native: Native, width: usize) -> Result<(), i32> {
    if native.value(LOADING)? != 0 {
        let mut seek = 0;
        if native.value(VERSION)? <= 61 {
            native.command(ATTACH, 0)?;
            let mut count = 0;
            native.integer(COMPACT, &mut count)?;
            // The native check owns its short-circuit Tell, including entirely
            // omitting it when checks are compiled out. The following Tell is
            // intentionally a separate call, as in the original expression.
            native.command(LEGACY_CHECK, count)?;
            let mut size = CheckedSize {
                size: (native.value(TELL)? as usize)
                    .wrapping_add((count as usize).wrapping_mul(width)),
                result: 0,
            };
            native.call(CHECKED_SIZE, 0, (&mut size as *mut CheckedSize).cast())?;
            seek = size.result;
        } else {
            native.integer(INTEGER, &mut seek)?;
            native.command(ATTACH, 0)?;
        }
        if native.value(LAZY_ENABLED)? == 0 {
            // Keep virtual Load dispatch (a TLazyArray subclass may override it).
            native.command(LOAD, 0)?;
        }
        native.command(SEEK, seek)?;
    } else if native.value(SAVING)? != 0 && native.value(VERSION)? > 61 {
        let mut count_pos = native.value(TELL)?;
        native.integer(INTEGER, &mut count_pos)?;
        native.command(BASE_SERIALIZE, 0)?;
        let mut end = native.value(TELL)?;
        native.command(SEEK, count_pos)?;
        native.integer(INTEGER, &mut end)?;
        native.command(SEEK, end)?;
    } else {
        native.command(BASE_SERIALIZE, 0)?;
    }
    Ok(())
}

fn save_array(native: Native, index: i32, count: i32, operation: i32) -> Result<(), i32> {
    let mut range = Range {
        index,
        count,
        operation,
    };
    native.call(SAVE_ARRAY, 0, (&mut range as *mut Range).cast())
}

fn transaction(native: Native, action: i32, arg: i32, value: *mut c_void) -> Result<(), i32> {
    match action {
        action::TRANSACTION_ADD => {
            let mut index = 0;
            native.call(ADD, arg, (&mut index as *mut i32).cast())?;
            if native.value(UNDO_EXISTS)? != 0 {
                save_array(native, index, arg, 1)?;
            }
            unsafe { value.cast::<i32>().write(index) };
        }
        action::TRANSACTION_INSERT | action::TRANSACTION_REMOVE => {
            let count = unsafe { value.cast::<i32>().read() };
            if action == action::TRANSACTION_INSERT {
                native.call(INSERT, arg, value)?;
            }
            if native.value(UNDO_EXISTS)? != 0 {
                save_array(
                    native,
                    arg,
                    count,
                    if action == action::TRANSACTION_INSERT {
                        1
                    } else {
                        -1
                    },
                )?;
            }
            if action == action::TRANSACTION_REMOVE {
                native.call(REMOVE, arg, value)?;
            }
        }
        action::TRANSACTION_EMPTY => {
            if native.value(UNDO_EXISTS)? != 0 {
                save_array(native, 0, native.value(NUM)?, -1)?;
            }
            native.command(EMPTY, arg)?;
        }
        action::TRANSACTION_MODIFY | action::TRANSACTION_MODIFY_ALL => {
            if native.value(UNDO_EXISTS)? != 0 {
                let (index, count) = if action == action::TRANSACTION_MODIFY {
                    (arg, 1)
                } else {
                    (0, native.value(BUFFER_NUM)?)
                };
                save_array(native, index, count, 0)?;
            }
        }
        action::TRANSACTION_ADD_ZEROED => {
            let mut index;
            if arg == 0 {
                index = native.value(NUM)?;
            } else {
                index = 0;
                transaction(
                    native,
                    action::TRANSACTION_ADD,
                    arg,
                    (&mut index as *mut i32).cast(),
                )?;
                let mut count = arg;
                native.call(ZERO_RANGE, index, (&mut count as *mut i32).cast())?;
            }
            unsafe { value.cast::<i32>().write(index) };
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn buffer(native: Native, action: i32, arg: i32, value: *mut c_void) -> Result<(), i32> {
    match action {
        action::BUFFER_WRITE => {
            let end = native.value(POSITION)?.wrapping_add(arg);
            if end > native.value(BUFFER_NUM)? {
                native.command(BUFFER_GROW, end.wrapping_sub(native.value(BUFFER_NUM)?))?;
            }
            native.call(if arg == 1 { WRITE_ONE } else { WRITE_MANY }, arg, value)?;
            native.command(SET_POSITION, native.value(POSITION)?.wrapping_add(arg))?;
        }
        action::BUFFER_READ => {
            native.command(READ_CHECK, arg)?;
            native.call(if arg == 1 { READ_ONE } else { READ_MANY }, arg, value)?;
            native.command(SET_POSITION, native.value(POSITION)?.wrapping_add(arg))?;
        }
        action::BUFFER_TELL => native.call(POSITION, 0, value)?,
        action::BUFFER_SIZE => native.call(BUFFER_NUM, 0, value)?,
        action::BUFFER_SEEK_WRITE | action::BUFFER_SEEK_READ => {
            if action == action::BUFFER_SEEK_READ {
                native.command(SEEK_CHECK, arg)?;
            }
            native.command(SET_POSITION, arg)?;
        }
        action::BUFFER_AT_END => {
            let at_end = native.value(POSITION)? >= native.value(BUFFER_NUM)?;
            unsafe { value.cast::<i32>().write(i32::from(at_end)) };
        }
        _ => unreachable!(),
    }
    Ok(())
}

/// Executes an EHp2ArrayArchiveAction with no allocation or native layout casts.
///
/// # Safety
/// Object/archive/value must match the action's native shell. The callback must
/// catch native exceptions and return their status, and provide live scalar or
/// pointer slots for the requested operation. No callback may unwind into Rust.
/// `value` is a count/output i32 for array-remove, transaction add/insert/remove,
/// buffer tell/size/at-end; a usize element width for lazy serialization; or the
/// original data pointer for buffer reads/writes. It may be null otherwise.
pub unsafe fn run(
    object: *mut c_void,
    archive: *mut c_void,
    callback: Hp2ArrayArchiveCallback,
    action: i32,
    arg: i32,
    value: *mut c_void,
) -> Result<(), i32> {
    let native = Native {
        object,
        archive,
        callback,
    };
    match action {
        action::ARRAY => serialize(native),
        action::REFERENCE => native.command(COUNT_INDEX, 0),
        action::TRANSACTION => {
            if native.value(TRANS)? == 0 {
                native.command(BASE_SERIALIZE, 0)?;
            }
            Ok(())
        }
        action::LAZY_SERIALIZE => lazy_serialize(native, unsafe { value.cast::<usize>().read() }),
        action::LAZY_LOAD => lazy_load(native),
        action::LAZY_UNLOAD => lazy_unload(native, arg != 0),
        action::LAZY_DESTROY => {
            let saved = native.saved()?;
            if !saved.archive.is_null() {
                saved.command(DETACH, 0)?;
            }
            Ok(())
        }
        action::ARRAY_EMPTY => empty(native, arg),
        action::ARRAY_REMOVE => remove(native, arg, unsafe { value.cast::<i32>().read() }),
        action::TRANSACTION_ADD..=action::TRANSACTION_MODIFY_ALL
        | action::TRANSACTION_ADD_ZEROED => transaction(native, action, arg, value),
        action::BUFFER_WRITE..=action::BUFFER_AT_END => buffer(native, action, arg, value),
        _ => Err(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        calls: Vec<(i32, i32)>,
        loading: bool,
        saving: bool,
        trans: bool,
        fast: bool,
        destruct: bool,
        undo: bool,
        lazy: bool,
        checks: bool,
        version: i32,
        num: i32,
        max: i32,
        wire_count: i32,
        wire_end: i32,
        pos: i32,
        saved_pos: i32,
        attached: bool,
        payload: i32,
        fail: Option<(i32, i32)>,
        counted: Vec<(i32, i32)>,
        transactions: Vec<(i32, i32, i32, i32)>,
        integers: Vec<(i32, i32)>,
        shrink_on_item: bool,
        replace_saved_pos: Option<i32>,
        fail_after_construct: bool,
    }

    impl Default for Fixture {
        fn default() -> Self {
            Self {
                calls: Vec::new(),
                loading: false,
                saving: false,
                trans: false,
                fast: false,
                destruct: false,
                undo: false,
                lazy: true,
                checks: true,
                version: 79,
                num: 0,
                max: 0,
                wire_count: 0,
                wire_end: 0,
                pos: 0,
                saved_pos: 0,
                attached: false,
                payload: 13,
                fail: None,
                counted: Vec::new(),
                transactions: Vec::new(),
                integers: Vec::new(),
                shrink_on_item: false,
                replace_saved_pos: None,
                fail_after_construct: false,
            }
        }
    }

    impl Fixture {
        fn execute(&mut self, action: i32, arg: i32, value: *mut c_void) -> Result<(), i32> {
            unsafe {
                run(
                    (self as *mut Self).cast(),
                    ptr::null_mut(),
                    service,
                    action,
                    arg,
                    value,
                )
            }
        }

        fn simple(&mut self, action: i32) -> Result<(), i32> {
            self.execute(action, 0, ptr::null_mut())
        }

        fn lazy_serialize(&mut self, width: usize) -> Result<(), i32> {
            let mut width = width;
            self.execute(action::LAZY_SERIALIZE, 0, (&mut width as *mut usize).cast())
        }

        fn operations(&self, operation: i32) -> Vec<i32> {
            self.calls
                .iter()
                .filter(|(op, _)| *op == operation)
                .map(|(_, arg)| *arg)
                .collect()
        }
    }

    // This fixture models the native leaves, not the migrated policy. Failure
    // injection is before the leaf's side effects, just like a throwing native
    // service; its trace exposes whether Rust continues after that failure.
    unsafe extern "C" fn service(
        object: *mut c_void,
        _archive: *mut c_void,
        op: i32,
        arg: i32,
        value: *mut c_void,
    ) -> i32 {
        let f = unsafe { &mut *object.cast::<Fixture>() };
        f.calls.push((op, arg));
        if f.fail == Some((op, arg)) {
            return 73;
        }
        let out = value.cast::<i32>();
        unsafe {
            match op {
                LOADING => out.write(i32::from(f.loading)),
                SAVING => out.write(i32::from(f.saving)),
                TRANS => out.write(i32::from(f.trans)),
                VERSION => out.write(f.version),
                FAST => out.write(i32::from(f.fast)),
                NEEDS_DESTRUCTOR => out.write(i32::from(f.destruct)),
                UNDO_EXISTS => out.write(i32::from(f.undo)),
                LAZY_ENABLED => out.write(i32::from(f.lazy)),
                NUM | BUFFER_NUM => out.write(f.num),
                TELL | POSITION => out.write(f.pos),
                SEEK | SET_POSITION => f.pos = arg,
                COUNT_BYTES => f.counted.push((f.num, f.max)),
                COUNT_INDEX => {
                    if f.loading {
                        f.num = f.wire_count;
                    }
                    f.pos += 1;
                }
                COMPACT => {
                    if f.loading {
                        out.write(f.wire_count);
                    }
                    f.pos += 1;
                }
                INTEGER => {
                    if f.loading {
                        out.write(f.wire_end);
                    }
                    f.integers.push((f.pos, out.read()));
                    f.pos += 4;
                }
                SET_EXACT => f.max = f.num,
                RAW_SERIALIZE => f.pos += f.num * 4,
                DESTRUCT => {}
                EMPTY_STORAGE => {
                    f.num = 0;
                    f.max = arg;
                }
                CONSTRUCT_SERIALIZE => {
                    f.num += 1;
                    if f.fail_after_construct {
                        return 73;
                    }
                    f.pos += 4;
                }
                ITEM_SERIALIZE => {
                    f.pos += 4;
                    if f.shrink_on_item {
                        f.num = 1;
                    }
                }
                REMOVE_CHECK => {
                    if f.checks && (arg < 0 || arg > f.num || arg.wrapping_add(out.read()) > f.num)
                    {
                        return 74;
                    }
                }
                REMOVE_STORAGE | REMOVE => f.num -= out.read(),
                LEGACY_CHECK => {
                    if f.checks {
                        if arg < 0 {
                            return 74;
                        }
                        f.calls.push((TELL, 0));
                        if f.pos < 0 {
                            return 74;
                        }
                    }
                }
                CHECKED_SIZE => {
                    let size = &mut *value.cast::<CheckedSize>();
                    if size.size > i32::MAX as usize {
                        return 75;
                    }
                    size.result = size.size as i32;
                }
                SAVED_ARCHIVE => value.cast::<*mut c_void>().write(if f.attached {
                    object
                } else {
                    ptr::null_mut()
                }),
                SAVED_POS => out.write(f.saved_pos),
                SET_SAVED_POS => f.saved_pos = arg,
                ATTACH => {
                    f.attached = true;
                    f.saved_pos = f.pos;
                }
                DETACH => {
                    f.attached = false;
                    f.saved_pos = 0;
                }
                BASE_SERIALIZE => {
                    f.pos += f.payload;
                    if let Some(pos) = f.replace_saved_pos {
                        f.saved_pos = pos;
                    }
                }
                LOAD => {}
                EMPTY => f.num = 0,
                SAVE_ARRAY => {
                    let range = &*value.cast::<Range>();
                    f.transactions
                        .push((range.index, range.count, range.operation, f.num));
                }
                ADD => {
                    out.write(f.num);
                    f.num += arg;
                }
                INSERT => f.num += out.read(),
                BUFFER_GROW => f.num += arg,
                ZERO_RANGE | WRITE_ONE | WRITE_MANY | READ_ONE | READ_MANY => {}
                READ_CHECK => {
                    if f.checks && (f.pos < 0 || f.pos.wrapping_add(arg) > f.num) {
                        return 74;
                    }
                }
                SEEK_CHECK => {
                    if f.checks && (arg < 0 || arg > f.num) {
                        return 74;
                    }
                }
                _ => return 76,
            }
        }
        0
    }

    #[test]
    fn fast_load_counts_old_storage_and_zero_replacement_has_no_payload() {
        let mut f = Fixture {
            loading: true,
            fast: true,
            num: 7,
            max: 12,
            ..Fixture::default()
        };
        assert_eq!(f.simple(action::ARRAY), Ok(()));
        assert_eq!(f.counted, [(7, 12)]);
        assert_eq!((f.num, f.max, f.pos), (0, 0, 1));
        assert!(f.operations(RAW_SERIALIZE).is_empty());
        assert!(f.operations(DESTRUCT).is_empty());
        assert_eq!(f.operations(SET_EXACT), [0]);

        f.calls.clear();
        f.wire_count = 3;
        assert_eq!(f.simple(action::REFERENCE), Ok(()));
        assert_eq!(f.calls, [(COUNT_INDEX, 0)]);
        assert_eq!((f.num, f.max), (3, 0));
    }

    #[test]
    fn typed_load_destroys_before_empty_and_constructs_each_element() {
        let mut f = Fixture {
            loading: true,
            destruct: true,
            num: 3,
            max: 8,
            wire_count: 2,
            ..Fixture::default()
        };
        assert_eq!(f.simple(action::ARRAY), Ok(()));
        assert_eq!(f.operations(DESTRUCT), [0, 1, 2]);
        assert_eq!(f.operations(EMPTY_STORAGE), [2]);
        assert_eq!(f.operations(CONSTRUCT_SERIALIZE), [0, 0]);
        assert_eq!((f.num, f.max, f.pos), (2, 2, 9));
        let destruct_last = f.calls.iter().rposition(|(op, _)| *op == DESTRUCT).unwrap();
        let empty = f
            .calls
            .iter()
            .position(|(op, _)| *op == EMPTY_STORAGE)
            .unwrap();
        let construct = f
            .calls
            .iter()
            .position(|(op, _)| *op == CONSTRUCT_SERIALIZE)
            .unwrap();
        assert!(destruct_last < empty && empty < construct);
    }

    #[test]
    fn destructor_failure_does_not_empty_or_construct_and_remove_checks_first() {
        let mut f = Fixture {
            loading: true,
            destruct: true,
            num: 3,
            wire_count: 2,
            fail: Some((DESTRUCT, 1)),
            ..Fixture::default()
        };
        assert_eq!(f.simple(action::ARRAY), Err(73));
        assert_eq!(f.operations(DESTRUCT), [0, 1]);
        assert!(f.operations(EMPTY_STORAGE).is_empty());
        assert!(f.operations(CONSTRUCT_SERIALIZE).is_empty());
        assert_eq!((f.num, f.pos), (3, 1));

        f.calls.clear();
        let mut count = 4i32;
        assert_eq!(
            f.execute(action::ARRAY_REMOVE, 0, (&mut count as *mut i32).cast()),
            Err(74)
        );
        assert_eq!(f.calls, [(REMOVE_CHECK, 0)]);
    }

    #[test]
    fn typed_field_archive_failure_keeps_the_constructed_prefix_alive() {
        let mut f = Fixture {
            loading: true,
            destruct: true,
            num: 2,
            wire_count: 3,
            fail_after_construct: true,
            ..Fixture::default()
        };
        assert_eq!(f.simple(action::ARRAY), Err(73));
        assert_eq!(f.operations(DESTRUCT), [0, 1]);
        assert_eq!(f.operations(CONSTRUCT_SERIALIZE), [0]);
        assert_eq!((f.num, f.max, f.pos), (1, 3, 1));
        assert_eq!(f.calls.last(), Some(&(CONSTRUCT_SERIALIZE, 0)));
    }

    #[test]
    fn neutral_archive_serializes_typed_items_and_rechecks_live_count() {
        let mut f = Fixture {
            num: 3,
            shrink_on_item: true,
            ..Fixture::default()
        };
        assert_eq!(f.simple(action::ARRAY), Ok(()));
        assert_eq!(f.operations(ITEM_SERIALIZE), [0]);
        assert_eq!(f.pos, 5);
        assert!(f.operations(CONSTRUCT_SERIALIZE).is_empty());
    }

    #[test]
    fn transaction_suppresses_both_directions_and_preserves_undo_order() {
        for loading in [false, true] {
            let mut f = Fixture {
                loading,
                saving: !loading,
                trans: true,
                ..Fixture::default()
            };
            assert_eq!(f.simple(action::TRANSACTION), Ok(()));
            assert_eq!(f.calls, [(TRANS, 0)]);
        }
        let mut f = Fixture {
            undo: true,
            num: 4,
            ..Fixture::default()
        };
        let mut index = -1i32;
        assert_eq!(
            f.execute(action::TRANSACTION_ADD, 2, (&mut index as *mut i32).cast()),
            Ok(())
        );
        assert_eq!((index, f.num), (4, 6));
        assert_eq!(f.transactions, [(4, 2, 1, 6)]);
        let mut count = 2i32;
        assert_eq!(
            f.execute(
                action::TRANSACTION_REMOVE,
                1,
                (&mut count as *mut i32).cast()
            ),
            Ok(())
        );
        assert_eq!(f.transactions[1], (1, 2, -1, 6));
        assert_eq!(f.num, 4);
        f.fail = Some((SAVE_ARRAY, 0));
        assert_eq!(
            f.execute(
                action::TRANSACTION_REMOVE,
                0,
                (&mut count as *mut i32).cast()
            ),
            Err(73)
        );
        assert_eq!(f.num, 4);
        assert_eq!(
            f.execute(action::TRANSACTION_ADD, 2, (&mut index as *mut i32).cast()),
            Err(73)
        );
        assert_eq!(f.num, 6);
        assert_eq!(index, 4); // Return value is not published after undo failure.
    }

    #[test]
    fn zero_add_and_disabled_undo_do_not_evaluate_unused_leaves() {
        let mut f = Fixture {
            undo: true,
            num: 7,
            ..Fixture::default()
        };
        let mut index = -1i32;
        assert_eq!(
            f.execute(
                action::TRANSACTION_ADD_ZEROED,
                0,
                (&mut index as *mut i32).cast()
            ),
            Ok(())
        );
        assert_eq!(index, 7);
        assert_eq!(f.calls, [(NUM, 0)]);
        f.calls.clear();
        f.undo = false;
        assert_eq!(f.simple(action::TRANSACTION_MODIFY_ALL), Ok(()));
        assert_eq!(f.calls, [(UNDO_EXISTS, 0)]);
        f.calls.clear();
        assert_eq!(
            f.execute(
                action::TRANSACTION_ADD_ZEROED,
                2,
                (&mut index as *mut i32).cast()
            ),
            Ok(())
        );
        assert_eq!((index, f.num), (7, 9));
        assert_eq!(f.operations(ZERO_RANGE), [7]);
    }

    #[test]
    fn lazy_version_boundary_attaches_at_the_original_count_position() {
        for version in [61, 62] {
            let mut f = Fixture {
                loading: true,
                version,
                pos: 1,
                wire_count: 3,
                wire_end: 18,
                ..Fixture::default()
            };
            assert_eq!(f.lazy_serialize(4), Ok(()));
            assert!(f.attached);
            assert_eq!(f.saved_pos, if version == 61 { 1 } else { 5 });
            assert_eq!(f.pos, if version == 61 { 14 } else { 18 });
            assert!(f.operations(LOAD).is_empty());
            f.calls.clear();
            f.lazy = false;
            assert_eq!(f.lazy_serialize(4), Ok(()));
            let load = f.calls.iter().position(|(op, _)| *op == LOAD).unwrap();
            let seek = f.calls.iter().position(|(op, _)| *op == SEEK).unwrap();
            assert!(load < seek);
        }
    }

    #[test]
    fn legacy_checks_keep_short_circuit_and_checked_size_error_order() {
        let mut f = Fixture {
            loading: true,
            version: 61,
            pos: 1,
            wire_count: -1,
            ..Fixture::default()
        };
        assert_eq!(f.lazy_serialize(4), Err(74));
        assert!(f.attached);
        assert!(f.operations(TELL).is_empty());
        assert!(f.operations(CHECKED_SIZE).is_empty());
        f.calls.clear();
        f.checks = false;
        f.pos = 1;
        // Disabled native checks do not call Tell, but size calculation still
        // does. Unsigned native arithmetic wraps before appCheckedIntSize.
        assert_eq!(f.lazy_serialize(4), Err(75));
        assert_eq!(f.operations(TELL), [0]);
        assert_eq!(f.operations(CHECKED_SIZE), [0]);
        assert!(f.operations(SEEK).is_empty());
        f.calls.clear();
        f.checks = true;
        f.pos = 1;
        f.wire_count = i32::MAX;
        assert_eq!(f.lazy_serialize(4), Err(75));
        assert_eq!(f.operations(TELL), [0, 0]);
    }

    #[test]
    fn lazy_load_failure_preserves_partial_cursor_and_negates_only_after_payload() {
        let mut f = Fixture {
            attached: true,
            saved_pos: 5,
            pos: 50,
            fail: Some((BASE_SERIALIZE, 0)),
            ..Fixture::default()
        };
        assert_eq!(f.simple(action::LAZY_LOAD), Err(73));
        assert_eq!((f.pos, f.saved_pos), (5, 5));
        assert_eq!(f.operations(SEEK), [5]);
        f.calls.clear();
        f.pos = 50;
        f.fail = Some((SEEK, 50));
        assert_eq!(f.simple(action::LAZY_LOAD), Err(73));
        assert_eq!((f.pos, f.saved_pos), (18, -5));
        assert_eq!(f.operations(SEEK), [5, 50]);
        f.calls.clear();
        assert_eq!(f.simple(action::LAZY_LOAD), Ok(()));
        assert_eq!(f.calls, [(SAVED_POS, 0)]);
    }

    #[test]
    fn lazy_load_reloads_saved_position_after_callbacks_and_zero_is_not_loadable() {
        let mut f = Fixture {
            attached: true,
            saved_pos: 5,
            pos: 50,
            replace_saved_pos: Some(9),
            ..Fixture::default()
        };
        assert_eq!(f.simple(action::LAZY_LOAD), Ok(()));
        assert_eq!((f.pos, f.saved_pos), (50, -9));
        f.saved_pos = 0;
        f.calls.clear();
        assert_eq!(f.simple(action::LAZY_LOAD), Ok(()));
        assert_eq!(f.calls, [(SAVED_POS, 0)]);
        assert_eq!(f.simple(action::LAZY_DESTROY), Ok(()));
        assert!(!f.attached);
        assert_eq!(f.operations(DETACH), [0]);
    }

    #[test]
    fn unload_preserves_gnu_lazy_flag_and_manual_detachment_lifetime() {
        let mut f = Fixture {
            attached: true,
            saved_pos: -5,
            lazy: false,
            num: 3,
            ..Fixture::default()
        };
        assert_eq!(f.execute(action::LAZY_UNLOAD, 1, ptr::null_mut()), Ok(()));
        assert_eq!((f.num, f.saved_pos), (3, -5));
        assert!(f.operations(EMPTY).is_empty());
        assert_eq!(f.execute(action::LAZY_UNLOAD, 0, ptr::null_mut()), Ok(()));
        assert_eq!((f.num, f.saved_pos), (0, 5));
        assert!(f.attached);
        assert!(f.operations(DETACH).is_empty());
        f.saved_pos = i32::MIN;
        assert_eq!(f.execute(action::LAZY_UNLOAD, 0, ptr::null_mut()), Ok(()));
        assert_eq!(f.saved_pos, i32::MIN);
    }

    #[test]
    fn modern_save_backpatches_and_neutral_or_legacy_archives_do_not() {
        let mut f = Fixture {
            saving: true,
            pos: 1,
            ..Fixture::default()
        };
        assert_eq!(f.lazy_serialize(4), Ok(()));
        assert_eq!(f.integers, [(1, 1), (1, 18)]);
        assert_eq!(f.operations(SEEK), [1, 18]);
        assert_eq!(f.pos, 18);
        for (saving, version) in [(true, 61), (false, 79)] {
            let mut f = Fixture {
                saving,
                version,
                ..Fixture::default()
            };
            assert_eq!(f.lazy_serialize(4), Ok(()));
            assert_eq!(f.operations(BASE_SERIALIZE), [0]);
            assert!(f.integers.is_empty());
            assert!(f.operations(SEEK).is_empty());
        }
    }

    #[test]
    fn buffer_zero_length_still_dispatches_copy_and_growth_is_not_truncation() {
        let mut f = Fixture {
            num: 5,
            pos: 2,
            ..Fixture::default()
        };
        assert_eq!(f.execute(action::BUFFER_WRITE, 0, ptr::null_mut()), Ok(()));
        assert_eq!(f.operations(WRITE_MANY), [0]);
        assert_eq!((f.num, f.pos), (5, 2));
        assert_eq!(f.execute(action::BUFFER_WRITE, 1, ptr::null_mut()), Ok(()));
        assert_eq!(f.operations(WRITE_ONE), [1]);
        assert_eq!((f.num, f.pos), (5, 3));
        assert_eq!(
            f.execute(action::BUFFER_SEEK_WRITE, 8, ptr::null_mut()),
            Ok(())
        );
        assert_eq!(f.execute(action::BUFFER_WRITE, 2, ptr::null_mut()), Ok(()));
        assert_eq!(f.operations(BUFFER_GROW), [5]);
        assert_eq!((f.num, f.pos), (10, 10));
        assert!(f.operations(SEEK_CHECK).is_empty());
        let mut end = 0i32;
        assert_eq!(
            f.execute(action::BUFFER_AT_END, 0, (&mut end as *mut i32).cast()),
            Ok(())
        );
        assert_eq!(end, 1);
    }

    #[test]
    fn buffer_checks_and_copy_failure_precede_cursor_update() {
        let mut f = Fixture {
            num: 5,
            pos: 4,
            ..Fixture::default()
        };
        assert_eq!(f.execute(action::BUFFER_READ, 2, ptr::null_mut()), Err(74));
        assert_eq!(f.calls, [(READ_CHECK, 2)]);
        assert_eq!(f.pos, 4);
        assert_eq!(
            f.execute(action::BUFFER_SEEK_READ, -1, ptr::null_mut()),
            Err(74)
        );
        assert_eq!(f.pos, 4);
        f.fail = Some((READ_ONE, 1));
        assert_eq!(f.execute(action::BUFFER_READ, 1, ptr::null_mut()), Err(73));
        assert_eq!(f.pos, 4);
        f.fail = Some((WRITE_MANY, 2));
        assert_eq!(f.execute(action::BUFFER_WRITE, 2, ptr::null_mut()), Err(73));
        assert_eq!((f.num, f.pos), (6, 4)); // Growth is not rolled back.
    }

    #[test]
    fn buffer_position_arithmetic_keeps_signed_wrapping() {
        let mut f = Fixture {
            num: i32::MAX,
            pos: i32::MAX,
            ..Fixture::default()
        };
        assert_eq!(f.execute(action::BUFFER_WRITE, 1, ptr::null_mut()), Ok(()));
        assert_eq!(f.pos, i32::MIN);
        assert!(f.operations(BUFFER_GROW).is_empty());
    }
}
