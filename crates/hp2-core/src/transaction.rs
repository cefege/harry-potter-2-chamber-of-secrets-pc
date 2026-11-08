//! Rust-owned editor transactions and undo history, with synchronous SDK leaves.
//!
//! Records are boxed so a serializer may append records or preload another record
//! without invalidating an active archive. Only initialized snapshot bytes are read.
//! Raw state pointers, never Rust references, cross a callback/reentry boundary.
use crate::ansi::Tchar;
use crate::array_storage::{self as arrays, Hp2ArraySlots};
use hp2_abi::engine::generated::TArrayRaw;
use std::ffi::c_void;
use std::ptr;

static ARRAY_TAG: [Tchar; 1] = [0];
static STRING_TAG: [Tchar; 8] = [70, 83, 116, 114, 105, 110, 103, 0];

/// Native SDK operations; none owns transaction containers or snapshot storage.
/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hp2TransactionLeaf {
    CaptureRecord, RestoreRecord, GcRecord, InitObject, NeedsPostLoad,
    PostLoad, ObjectModel, EditorAvailable, ModelNodeCount, BuildBounds,
    SerializeText, SerializeObject, SerializeInt, SerializeCount,
    CreateShell, DestroyShell, ApplyShell, SerializeShell, SetUndo, Log, Finish,
}

/// Separate scalar and pointer results retain provenance across SDK callbacks.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Hp2TransactionResult {
    pub integer: usize,
    pub pointer: *mut c_void,
}

/// Synchronous, exception-contained SDK callback.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2TransactionServices {
    pub call: unsafe extern "C" fn(*mut c_void, Hp2TransactionLeaf,
        *mut c_void, *mut c_void, *mut Hp2TransactionResult) -> i32,
}

/// Nonowning metadata view of a stable Rust record.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Hp2TransRecordView {
    pub object: *mut c_void,
    pub array: *mut c_void,
    pub index: i32,
    pub count: i32,
    pub oper: i32,
    pub element_size: i32,
    pub serializer: *mut c_void,
    pub destructor: *mut c_void,
    pub data: *mut c_void,
}

/// Typed archive/object operations for one active serialization call.
/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hp2TransRecordLeaf {
    ValidateContents, ValidateRange, Destructor, Serializer, ArrayRemove,
    ArrayInsertZeroed, ObjectSerialize, SelectWriterFlip, SelectReaderData,
    PseudoBegin, PseudoSerializeReader, PseudoSerializeAr,
    PseudoEnd, ArrayTempBegin, ArrayElementZero, ArraySerializerReader,
    ArraySerializerAr, ArrayDestructorTemp,
}

/// Active native archive callbacks; native exceptions never unwind into Rust.
#[repr(C)]
pub struct Hp2TransRecordServices {
    pub call: unsafe extern "C" fn(*mut c_void, Hp2TransRecordLeaf, usize, usize, *mut usize) -> i32,
}

fn status(code: i32) -> Result<(), i32> {
    if code == 0 { Ok(()) } else { Err(code) }
}

unsafe fn call(context: *mut c_void, services: Hp2TransactionServices,
    leaf: Hp2TransactionLeaf, a: *mut c_void, b: *mut c_void) -> Result<Hp2TransactionResult, i32> {
    let mut out = Hp2TransactionResult::default();
    // SAFETY: caller keeps the context and callback alive; callback catches unwinds.
    unsafe { status((services.call)(context, leaf, a, b, &mut out))?; }
    Ok(out)
}

struct Buffer { raw: TArrayRaw, allocator: arrays::Hp2ArrayServices }
impl Buffer {
    fn new(allocator: arrays::Hp2ArrayServices) -> Self {
        Self { raw: TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }, allocator }
    }
    fn slots(&mut self) -> Hp2ArraySlots {
        Hp2ArraySlots { data: &raw mut self.raw.data, num: &raw mut self.raw.num,
            max: &raw mut self.raw.max }
    }
    unsafe fn text(input: *const Tchar, allocator: arrays::Hp2ArrayServices) -> Result<Self, i32> {
        let mut result = Self::new(allocator);
        // SAFETY: input is terminated and live; canonical string policy owns allocation.
        unsafe { crate::string_storage::assign_with_allocator(result.slots(), None,
            input, true, allocator, STRING_TAG.as_ptr().cast())?; }
        Ok(result)
    }
    fn text_ptr(&self) -> *const Tchar {
        static EMPTY: [Tchar; 1] = [0];
        if self.raw.num == 0 { EMPTY.as_ptr() } else { self.raw.data.cast() }
    }
}
impl Drop for Buffer {
    fn drop(&mut self) {
        // SAFETY: this resource uniquely owns its initialized canonical array.
        // The native allocator preserves the first pending exception during cleanup.
        unsafe {
            if !self.raw.data.is_null() {
                let _ = arrays::release(self.slots(), self.allocator);
            }
        }
    }
}

unsafe fn release_buffer(buffer: *mut Buffer) -> Result<(), i32> {
    let svc = unsafe { (*buffer).allocator };
    let result = unsafe { arrays::release((*buffer).slots(), svc) };
    if result.is_err() {
        // A final free may have consumed the allocation before reporting an
        // error. Never retry it from Drop and replace the first native failure.
        unsafe { (*buffer).raw = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }; }
    }
    result
}

struct Record { view: Hp2TransRecordView, bytes: Buffer, restored: bool }

// Keep ownership without reborrowing the whole Box pointee: archive callbacks
// retain raw pointers into its snapshot while records are appended or visited.
struct OwnedRecord(*mut Record);
impl Drop for OwnedRecord {
    fn drop(&mut self) {
        // SAFETY: this owner uniquely owns the allocation; callbacks have ended.
        unsafe { drop(Box::from_raw(self.0)); }
    }
}
/// Transaction storage. Access only through the engine-thread serialized API.
pub struct Transaction {
    records: Vec<OwnedRecord>,
    title: Buffer,
    flip: bool,
    inc: i32,
}

/// # Safety
/// `title` is terminated TCHAR input; allocator leaves stay live through destruction.
pub unsafe fn create(title: *const Tchar, flip: bool, allocator: arrays::Hp2ArrayServices) -> Result<*mut Transaction, i32> {
    Ok(Box::into_raw(Box::new(Transaction { records: Vec::new(),
        title: unsafe { Buffer::text(title, allocator)? }, flip, inc: -1 })))
}

/// # Safety
/// `state` was returned by create and has no active callbacks or other owner.
pub unsafe fn destroy(state: *mut Transaction) -> Result<(), i32> {
    if state.is_null() { return Ok(()); }
    let mut first = Ok(());
    let count = unsafe { (*state).records.len() };
    for index in 0..count {
        let record = unsafe { record_at(state, index) };
        let next = unsafe { release_buffer(&raw mut (*record).bytes) };
        if first.is_ok() { first = next; }
    }
    let next = unsafe { release_buffer(&raw mut (*state).title) };
    if first.is_ok() { first = next; }
    // SAFETY: all resource releases have been attempted exactly once.
    unsafe { drop(Box::from_raw(state)); }
    first
}

/// # Safety
/// State, input and callback are live. Reentry may append but not destroy state.
pub unsafe fn save(state: *mut Transaction, context: *mut c_void,
    svc: Hp2TransactionServices, mut view: Hp2TransRecordView) -> Result<(), i32> {
    view.data = ptr::null_mut();
    let record = OwnedRecord(Box::into_raw(Box::new(Record {
        view, bytes: Buffer::new(unsafe { (*state).title.allocator }), restored: false,
    })));
    let view_ptr = unsafe {
        (*record.0).view.data = (&raw mut (*record.0).bytes).cast();
        &raw mut (*record.0).view
    };
    // Publish before capture: recursive SaveObject/Preload sees a stable record,
    // and a failed capture leaves only its initialized prefix owned and destructible.
    unsafe { (*state).records.push(record); }
    unsafe { call(context, svc, Hp2TransactionLeaf::CaptureRecord, view_ptr.cast(), ptr::null_mut())?; }
    Ok(())
}

unsafe fn record_at(state: *mut Transaction, index: usize) -> *mut Record {
    // SAFETY: copy the stable pointer; no record reference crosses a callback.
    unsafe { (&(*state).records)[index].0 }
}

unsafe fn restore(state: *mut Transaction, index: usize, context: *mut c_void,
    svc: Hp2TransactionServices) -> Result<(), i32> {
    let record = unsafe { record_at(state, index) };
    if unsafe { (*record).restored } { return Ok(()); }
    unsafe { (*record).restored = true; }
    let mut flip = Buffer::new(unsafe { (*state).title.allocator });
    let do_flip = unsafe { (*state).flip };
    unsafe { call(context, svc, Hp2TransactionLeaf::RestoreRecord,
        (&raw mut (*record).view).cast(),
        if do_flip { (&raw mut flip).cast() } else { ptr::null_mut() })?; }
    if do_flip {
        // No reference survives a callback; exchange allocations, never byte-copy.
        unsafe { ptr::swap(&raw mut (*record).bytes, &raw mut flip);
            (*record).view.oper = -(*record).view.oper; }
    }
    unsafe { release_buffer(&raw mut flip) }
}

/// # Safety
/// State and callbacks stay live, including through synchronous preload reentry.
pub unsafe fn preload(state: *mut Transaction, context: *mut c_void,
    svc: Hp2TransactionServices, object: *mut c_void) -> Result<(), i32> {
    let mut index = 0;
    while index < unsafe { (*state).records.len() } {
        let record = unsafe { record_at(state, index) };
        if unsafe { (*record).view.object == object } {
            unsafe { restore(state, index, context, svc)?; }
        }
        index += 1;
    }
    Ok(())
}

/// # Safety
/// Same reentry contract as preload. No caller may destroy active state.
pub unsafe fn apply(state: *mut Transaction, context: *mut c_void,
    svc: Hp2TransactionServices) -> Result<(), i32> {
    use Hp2TransactionLeaf::*;
    let inc = unsafe { (*state).inc };
    let count = unsafe { (*state).records.len() } as i32;
    let (start, end) = if inc == 1 { (0, count) } else { (count - 1, -1) };
    let mut i = start;
    while i != end {
        let record = unsafe { record_at(state, i as usize) };
        unsafe { (*record).restored = false;
            call(context, svc, InitObject, (*record).view.object, ptr::null_mut())?; }
        i += inc;
    }
    let mut i = start;
    while i != end { unsafe { restore(state, i as usize, context, svc)?; } i += inc; }
    let mut i = start;
    while i != end {
        let object = unsafe { (*record_at(state, i as usize)).view.object };
        if unsafe { call(context, svc, NeedsPostLoad, object, ptr::null_mut())? }.integer != 0 {
            unsafe { call(context, svc, PostLoad, object, ptr::null_mut())?; }
            let model = unsafe { call(context, svc, ObjectModel, object, ptr::null_mut())? }.pointer;
            if !model.is_null() && unsafe { call(context, svc, EditorAvailable, ptr::null_mut(), ptr::null_mut())? }.integer != 0
                && unsafe { call(context, svc, ModelNodeCount, model, ptr::null_mut())? }.integer != 0 {
                unsafe { call(context, svc, BuildBounds, model, ptr::null_mut())?; }
            }
        }
        i += inc;
    }
    if unsafe { (*state).flip } { unsafe { (*state).inc = -inc; } }
    Ok(())
}

/// # Safety
/// State is live and access is engine-thread serialized.
unsafe fn data_size(state: *mut Transaction) -> usize {
    // SAFETY: no callbacks occur while this immutable iteration is live.
    unsafe { (*state).records.iter().map(|r| (*r.0).bytes.raw.num as usize).sum() }
}

/// # Safety
/// State and archive callbacks stay live. The transaction archive is nonpersistent.
pub unsafe fn serialize(state: *mut Transaction, context: *mut c_void,
    svc: Hp2TransactionServices) -> Result<(), i32> {
    let mut count = unsafe { (*state).records.len() as i32 };
    unsafe { call(context, svc, Hp2TransactionLeaf::SerializeCount,
        (&raw mut count).cast(), ptr::null_mut())?; }
    // Native FObjectRecord's archive never persisted snapshots or metadata, so
    // loading a differently sized transaction was not a supported operation.
    if count != unsafe { (*state).records.len() as i32 } { return Err(1); }
    for i in 0..count as usize {
        let record = unsafe { record_at(state, i) };
        unsafe { call(context, svc, Hp2TransactionLeaf::SerializeObject,
            (&raw mut (*record).view.object).cast(), ptr::null_mut())?;
            call(context, svc, Hp2TransactionLeaf::GcRecord,
                (&raw mut (*record).view).cast(), ptr::null_mut())?; }
    }
    unsafe { call(context, svc, Hp2TransactionLeaf::SerializeText,
        (&raw mut (*state).title.raw).cast(), ptr::null_mut())?; }
    Ok(())
}

/// # Safety
/// Buffer is a live Rust snapshot, data is valid for num initialized bytes. The
/// writer input does not overlap the snapshot; offset is a live archive cursor.
pub unsafe fn bytes(buffer: *mut c_void, offset: *mut i32, data: *mut c_void,
    num: i32, loading: bool) -> Result<(), i32> {
    let buffer = buffer.cast::<Buffer>();
    let start = unsafe { *offset };
    if num < 0 || start < 0 { return Err(1); }
    if loading {
        let end = start.checked_add(num).ok_or(1)?;
        if end > unsafe { (*buffer).raw.num } { return Err(1); }
        if num != 0 {
            unsafe { ptr::copy_nonoverlapping((*buffer).raw.data.cast::<u8>().add(start as usize),
                data.cast::<u8>(), num as usize); }
        }
        unsafe { *offset = end; }
    } else {
        // Reserve first without publishing uninitialized bytes, then commit the
        // initialized prefix. Re-resolve storage after allocator callbacks.
        let old = unsafe { (*buffer).raw.num };
        let end = old.checked_add(num).ok_or(1)?;
        let svc = unsafe { (*buffer).allocator };
        if end > unsafe { (*buffer).raw.max } {
            let old_max = unsafe { (*buffer).raw.max };
            unsafe { (*buffer).raw.max = end.saturating_add((i64::from(end) * 3 / 8) as i32).saturating_add(32); }
            if let Err(error) = unsafe { arrays::realloc((*buffer).slots(), 1, ARRAY_TAG.as_ptr().cast(), svc) } {
                unsafe { (*buffer).raw.max = old_max; }
                return Err(error);
            }
        }
        if num != 0 {
            unsafe { ptr::copy_nonoverlapping(data.cast::<u8>(),
                (*buffer).raw.data.cast::<u8>().add(old as usize), num as usize); }
        }
        unsafe { (*buffer).raw.num = end; *offset = end; }
    }
    Ok(())
}

unsafe fn record_call(context: *mut c_void, svc: &Hp2TransRecordServices,
    leaf: Hp2TransRecordLeaf, a: usize, b: usize) -> Result<(), i32> {
    let mut result = 0;
    unsafe { status((svc.call)(context, leaf, a, b, &mut result)) }
}
unsafe fn contents(context: *mut c_void, svc: &Hp2TransRecordServices,
    view: Hp2TransRecordView, loading: bool, oper: i32) -> Result<(), i32> {
    use Hp2TransRecordLeaf::*;
    unsafe { record_call(context, svc, ValidateContents, loading as usize, oper as usize)?; }
    if view.array.is_null() {
        unsafe { record_call(context, svc, ObjectSerialize, 0, 0)?; }
    } else if oper == 1 {
        if loading {
            unsafe { record_call(context, svc, ValidateRange, 0, 0)?; }
            for i in view.index..view.index + view.count {
                unsafe { record_call(context, svc, Destructor, i as usize, 0)?; }
            }
            unsafe { record_call(context, svc, ArrayRemove, view.index as usize, view.count as usize)?; }
        }
    } else {
        if oper == -1 && loading {
            unsafe { record_call(context, svc, ArrayInsertZeroed, view.index as usize, view.count as usize)?; }
        }
        unsafe { record_call(context, svc, ValidateRange, 0, 0)?; }
        for i in view.index..view.index + view.count {
            unsafe { record_call(context, svc, Serializer, i as usize, 0)?; }
        }
    }
    Ok(())
}

/// Runs one scoped native archive visit: capture (0), restore (1), GC (2).
/// # Safety
/// View is a copied initialized record; callbacks own only typed SDK temporaries.
pub unsafe fn visit(context: *mut c_void, svc: &Hp2TransRecordServices,
    view: Hp2TransRecordView, mode: i32, flip: bool) -> Result<(), i32> {
    use Hp2TransRecordLeaf::*;
    if mode == 0 { return unsafe { contents(context, svc, view, false, view.oper) }; }
    if mode == 1 {
        if flip {
            unsafe { record_call(context, svc, SelectWriterFlip, 0, 0)?;
                contents(context, svc, view, false, -view.oper)?; }
        }
        unsafe { record_call(context, svc, SelectReaderData, 0, 0)?;
            contents(context, svc, view, true, view.oper)?; }
        return Ok(());
    }
    if view.array.is_null() {
        unsafe { record_call(context, svc, PseudoBegin, 0, 0)?; }
        let primary = unsafe { record_call(context, svc, PseudoSerializeReader, 0, 0)
            .and_then(|()| record_call(context, svc, PseudoSerializeAr, 0, 0)) };
        let cleanup = unsafe { record_call(context, svc, PseudoEnd, 0, 0) };
        primary.and(cleanup)?;
    } else if unsafe { (*view.data.cast::<Buffer>()).raw.num } != 0 {
        unsafe { record_call(context, svc, ArrayTempBegin, 0, 0)?; }
        for i in view.index..view.index + view.count {
            unsafe { record_call(context, svc, ArrayElementZero, 0, 0)?; }
            let primary = unsafe { record_call(context, svc, ArraySerializerReader, i as usize, 0)
                .and_then(|()| record_call(context, svc, ArraySerializerAr, i as usize, 0)) };
            let cleanup = unsafe { record_call(context, svc, ArrayDestructorTemp, 0, 0) };
            primary.and(cleanup)?;
        }
    }
    Ok(())
}

struct Entry { state: *mut Transaction, shell: *mut c_void }
/// Rust-owned undo history, SDK shell lifetimes, reset text and cursor state.
pub struct History {
    entries: Vec<Entry>, reset: Buffer, undo: i32, active: i32,
    overflow: i32, max_memory: usize,
}
/// # Safety
/// Allocator leaves stay live through destruction; max_memory is a native SIZE_T.
pub unsafe fn history_create(max_memory: usize, allocator: arrays::Hp2ArrayServices) -> *mut History {
    Box::into_raw(Box::new(History { entries: Vec::new(), reset: Buffer::new(allocator),
        undo: 0, active: 0, overflow: 0, max_memory }))
}
unsafe fn remove(state: *mut History, index: usize, context: *mut c_void,
    svc: Hp2TransactionServices) -> Result<(), i32> {
    let entry = unsafe { (*state).entries.remove(index) };
    let result = unsafe { call(context, svc, Hp2TransactionLeaf::DestroyShell, entry.shell, ptr::null_mut()) };
    let cleanup = unsafe { destroy(entry.state) };
    result.map(|_| ()).and(cleanup)
}
/// # Safety
/// History is uniquely owned and has no active SDK callbacks.
pub unsafe fn history_destroy(state: *mut History, context: *mut c_void,
    svc: Hp2TransactionServices) -> Result<(), i32> {
    if state.is_null() { return Ok(()); }
    let mut first = Ok(());
    while unsafe { !(*state).entries.is_empty() } {
        let next = unsafe { remove(state, 0, context, svc) };
        if first.is_ok() { first = next; }
    }
    let cleanup = unsafe { release_buffer(&raw mut (*state).reset) };
    if first.is_ok() { first = cleanup; }
    unsafe { drop(Box::from_raw(state)); }
    first
}
unsafe fn total(state: *mut History) -> usize {
    // SAFETY: iteration invokes no native callback.
    unsafe { (*state).entries.iter().map(|entry| data_size(entry.state)).sum() }
}

/// Borrow the canonical reset string without converting its address to an integer.
/// # Safety
/// History remains live and engine-thread serialized while this view is used.
pub unsafe fn history_reset_reason(state: *const History) -> *const TArrayRaw {
    unsafe { &raw const (*state).reset.raw }
}

/// History command codes: reset=0, begin=1, end=2, continue=3, can undo=4,
/// can redo=5, undo=6, redo=7, serialize=8, size=9,
/// active count=11, undo count=12, history length=13.
/// # Safety
/// Live state and synchronous exception-contained callbacks. Name is terminated
/// input for reset/begin. No callback may destroy the active history.
pub unsafe fn history_command(state: *mut History, context: *mut c_void,
    svc: Hp2TransactionServices, command: i32, name: *const Tchar) -> Result<usize, i32> {
    use Hp2TransactionLeaf::*;
    match command {
        0 => {
            while unsafe { !(*state).entries.is_empty() } {
                unsafe { remove(state, 0, context, svc)?; }
            }
            unsafe { (*state).undo = 0; }
            let reason = unsafe { Buffer::text(name, (*state).reset.allocator)? };
            unsafe { (*state).reset = reason; (*state).active = 0; (*state).overflow = 0; }
        }
        1 => {
            let active = unsafe { (*state).active };
            unsafe { (*state).active += 1; }
            if active == 0 {
                while unsafe { (*state).undo != 0 } {
                    let index = unsafe { (*state).entries.len() - (*state).undo as usize };
                    unsafe { remove(state, index, context, svc)?; (*state).undo -= 1; }
                }
                while unsafe { total(state) > (*state).max_memory } {
                    unsafe { remove(state, 0, context, svc)?; }
                }
                let transaction = unsafe { create(name, true, (*state).reset.allocator)? };
                let shell = match unsafe { call(context, svc, CreateShell, transaction.cast(), ptr::null_mut()) } {
                    Ok(result) => result.pointer,
                    Err(error) => { let _ = unsafe { destroy(transaction) }; return Err(error); }
                };
                unsafe { (*state).entries.push(Entry { state: transaction, shell });
                    call(context, svc, SetUndo, shell, ptr::null_mut())?; (*state).overflow = 0; }
            }
        }
        2 => {
            unsafe { (*state).active -= 1; }
            if unsafe { (*state).active == 0 } { unsafe { call(context, svc, SetUndo, ptr::null_mut(), ptr::null_mut())?; } }
        }
        3 => {
            if unsafe { (*state).active == 0 && (*state).undo == 0 && !(*state).entries.is_empty() } {
                unsafe { (*state).active += 1; }
                let shell = unsafe { (*state).entries.last().expect("nonempty history").shell };
                unsafe { call(context, svc, SetUndo, shell, ptr::null_mut())?; }
            }
        }
        4 => return Ok(unsafe { if (*state).active != 0 { 1 }
            else if (*state).entries.len() == (*state).undo as usize { 2 } else { 0 } }),
        5 => return Ok(unsafe { if (*state).active != 0 { 1 } else if (*state).undo == 0 { 2 } else { 0 } }),
        6 | 7 => {
            let redo = command == 7;
            if unsafe { history_command(state, context, svc, if redo { 5 } else { 4 }, ptr::null())? } != 0 {
                return Ok(0);
            }
            let index = unsafe {
                if !redo { (*state).undo += 1; }
                let index = (*state).entries.len() - (*state).undo as usize;
                if redo { (*state).undo -= 1; }
                index
            };
            let transaction = unsafe { (&(*state).entries)[index].state };
            let shell = unsafe { (&(*state).entries)[index].shell };
            // Native shell supplies its own preload identity while applying.
            let redo_flag = i32::from(redo);
            unsafe { call(context, svc, Log, (*transaction).title.text_ptr().cast_mut().cast(),
                    (&raw const redo_flag).cast_mut().cast())?;
                call(context, svc, ApplyShell, shell, ptr::null_mut())?;
                call(context, svc, Finish, ptr::null_mut(), ptr::null_mut())?; }
            return Ok(1);
        }
        8 => {
            let mut count = unsafe { (*state).entries.len() as i32 };
            unsafe { call(context, svc, SerializeCount, (&raw mut count).cast(), ptr::null_mut())?; }
            if count != unsafe { (*state).entries.len() as i32 } { return Err(1); }
            for i in 0..count as usize {
                let shell = unsafe { (&(*state).entries)[i].shell };
                unsafe { call(context, svc, SerializeShell, shell, ptr::null_mut())?; }
            }
            unsafe { call(context, svc, SerializeText, (&raw mut (*state).reset.raw).cast(), ptr::null_mut())?;
                call(context, svc, SerializeInt, (&raw mut (*state).undo).cast(), ptr::null_mut())?;
                call(context, svc, SerializeInt, (&raw mut (*state).active).cast(), ptr::null_mut())?;
                call(context, svc, SerializeInt, (&raw mut (*state).overflow).cast(), ptr::null_mut())?; }
        }
        9 => return Ok(unsafe { total(state) }),
        11 => return Ok(unsafe { (*state).active as usize }),
        12 => return Ok(unsafe { (*state).undo as usize }),
        13 => return Ok(unsafe { (*state).entries.len() }),
        _ => return Err(1),
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    const TITLE: [Tchar; 2] = [84, 0];
    #[derive(Default)]
    struct Object {
        value: i32,
        array: Vec<i32>,
        preload: *mut Object,
        append: *mut Object,
    }
    #[derive(Default)]
    struct World {
        transaction: *mut Transaction,
        undo: *mut Transaction,
        destroyed: usize,
        loads: usize,
        fail_capture: bool,
        fail_destroy: bool,
    }
    fn object_view(object: *mut Object) -> Hp2TransRecordView {
        Hp2TransRecordView { object: object.cast(), array: ptr::null_mut(),
            index: 0, count: 0, oper: 0, element_size: 0,
            serializer: ptr::null_mut(), destructor: ptr::null_mut(), data: ptr::null_mut() }
    }
    fn array_view(object: *mut Object, index: i32, count: i32, oper: i32) -> Hp2TransRecordView {
        Hp2TransRecordView { array: object.cast(), index, count, oper,
            element_size: 4, ..object_view(object) }
    }
    struct Active {
        world: *mut World,
        view: Hp2TransRecordView,
        buffer: *mut c_void,
        flip: *mut c_void,
        offset: i32,
        reader_offset: i32,
        loading: bool,
        temp: i32,
    }
    unsafe fn transfer(active: *mut Active, value: *mut i32) -> Result<(), i32> {
        unsafe { bytes((*active).buffer, &raw mut (*active).offset, value.cast(), 4, (*active).loading) }
    }
    unsafe extern "C" fn record_service(context: *mut c_void, leaf: Hp2TransRecordLeaf,
        a: usize, b: usize, _out: *mut usize) -> i32 {
        // SAFETY: each test keeps all active stack contexts and objects live;
        // only raw pointers are retained while a nested callback may run.
        let result = unsafe {
            let active = context.cast::<Active>();
            let world = (*active).world;
            let object = (*active).view.object.cast::<Object>();
            use Hp2TransRecordLeaf::*;
            match leaf {
                ObjectSerialize => {
                    transfer(active, &raw mut (*object).value).and_then(|()| {
                        if (*active).loading {
                            (*world).loads += 1;
                            let target = (*object).preload;
                            if !target.is_null() {
                                preload((*world).transaction, world.cast(), SERVICES, target.cast())?;
                            }
                        } else {
                            let extra = (*object).append;
                            (*object).append = ptr::null_mut();
                            if !extra.is_null() {
                                save((*world).transaction, world.cast(), SERVICES, object_view(extra))?;
                            }
                            if (*world).fail_capture { return Err(37); }
                        }
                        Ok(())
                    })
                }
                Serializer => {
                    let value = (&mut (*object).array).as_mut_ptr().add(a);
                    transfer(active, value)
                }
                ArrayInsertZeroed => {
                    (&mut (*object).array).splice(a..a, std::iter::repeat_n(0, b));
                    Ok(())
                }
                ArrayRemove => { (&mut (*object).array).drain(a..a + b); Ok(()) }
                SelectWriterFlip => {
                    (*active).buffer = (*active).flip;
                    (*active).loading = false;
                    (*active).offset = 0;
                    Ok(())
                }
                SelectReaderData => {
                    (*active).buffer = (*active).view.data;
                    (*active).loading = true;
                    (*active).offset = 0;
                    Ok(())
                }
                PseudoSerializeReader | ArraySerializerReader => {
                    bytes((*active).view.data, &raw mut (*active).reader_offset,
                        (&raw mut (*active).temp).cast(), 4, true)
                }
                _ => Ok(()),
            }
        };
        result.err().unwrap_or(0)
    }
    const RECORD: Hp2TransRecordServices = Hp2TransRecordServices { call: record_service };
    unsafe extern "C" fn service(context: *mut c_void, leaf: Hp2TransactionLeaf,
        a: *mut c_void, b: *mut c_void, out: *mut Hp2TransactionResult) -> i32 {
        let world = context.cast::<World>();
        // SAFETY: fixture objects and contexts are live for the synchronous visit.
        let result = unsafe {
            use Hp2TransactionLeaf::*;
            match leaf {
                CaptureRecord | RestoreRecord | GcRecord => {
                    let view = a.cast::<Hp2TransRecordView>().read();
                    let mode = if leaf == CaptureRecord { 0 } else if leaf == RestoreRecord { 1 } else { 2 };
                    let mut active = Active { world, view, buffer: view.data, flip: b,
                        offset: 0, reader_offset: 0, loading: mode != 0, temp: 0 };
                    visit((&raw mut active).cast(), &RECORD, view, mode, !b.is_null())
                }
                CreateShell => { (*out).pointer = a; Ok(()) }
                DestroyShell => {
                    (*world).destroyed += 1;
                    if (*world).fail_destroy {
                        Err(if (*world).destroyed == 1 { 37 } else { 91 })
                    } else { Ok(()) }
                }
                SetUndo => { (*world).undo = a.cast(); Ok(()) }
                ApplyShell => {
                    let old = (*world).transaction;
                    (*world).transaction = a.cast();
                    let result = apply(a.cast(), context, SERVICES);
                    (*world).transaction = old;
                    result
                }
                SerializeShell => serialize(a.cast(), context, SERVICES),
                _ => { (*out).integer = 0; Ok(()) }
            }
        };
        result.err().unwrap_or(0)
    }
    const SERVICES: Hp2TransactionServices = Hp2TransactionServices { call: service };

    #[test]
    fn object_flip_preload_reentry_and_append_preserve_live_record_identity() {
        let mut second = Object { value: 2, ..Object::default() };
        let second_ptr = &raw mut second;
        let mut first = Object { value: 1, append: second_ptr, ..Object::default() };
        let first_ptr = &raw mut first;
        let state = unsafe { create(TITLE.as_ptr(), true, arrays::system_allocator()) }.unwrap();
        let mut world = World { transaction: state, ..World::default() };
        let context = (&raw mut world).cast();
        unsafe { save(state, context, SERVICES, object_view(first_ptr)) }.unwrap();
        assert_eq!(unsafe { data_size(state) }, 8);
        unsafe { (*first_ptr).value = 11; (*second_ptr).value = 22; }
        // Last record restores first, preloads the earlier record, whose self
        // preload must observe Restored before touching its live archive again.
        unsafe { (*second_ptr).preload = first_ptr; (*first_ptr).preload = first_ptr; }
        unsafe { apply(state, context, SERVICES) }.unwrap();
        assert_eq!(unsafe { ((*first_ptr).value, (*second_ptr).value, world.loads) }, (1, 2, 2));
        unsafe { apply(state, context, SERVICES) }.unwrap();
        assert_eq!(unsafe { ((*first_ptr).value, (*second_ptr).value) }, (11, 22));
        unsafe { destroy(state) }.unwrap();
    }

    #[test]
    fn array_insert_remove_and_modify_round_trip() {
        for oper in [-1, 0, 1] {
            let mut object = Object { array: vec![10, 20, 30], ..Object::default() };
            let state = unsafe { create(TITLE.as_ptr(), true, arrays::system_allocator()) }.unwrap();
            let mut world = World { transaction: state, ..World::default() };
            let context = (&raw mut world).cast();
            let object_ptr = &raw mut object;
            unsafe { save(state, context, SERVICES, array_view(object_ptr, 1, 1, oper)) }.unwrap();
            // The SDK keeps this object pointer; mutate through the same raw
            // provenance rather than creating a competing Rust &mut to its Vec.
            unsafe {
                match oper {
                    -1 => { (&mut (*object_ptr).array).remove(1); }
                    0 => (&mut (*object_ptr).array)[1] = 99,
                    1 => {}
                    _ => unreachable!(),
                }
            }
            let changed = unsafe { (&(*object_ptr).array).clone() };
            unsafe { apply(state, context, SERVICES) }.unwrap();
            let expected = if oper == 1 { vec![10, 30] } else { vec![10, 20, 30] };
            assert_eq!(unsafe { &(*object_ptr).array }, &expected, "undo operation {oper}");
            unsafe { apply(state, context, SERVICES) }.unwrap();
            assert_eq!(unsafe { &(*object_ptr).array }, &changed, "redo operation {oper}");
            unsafe { destroy(state) }.unwrap();
        }
    }

    #[test]
    fn failed_capture_keeps_initialized_prefix_and_restore_guard() {
        let mut object = Object { value: 41, ..Object::default() };
        let state = unsafe { create(TITLE.as_ptr(), true, arrays::system_allocator()) }.unwrap();
        let mut world = World { transaction: state, fail_capture: true, ..World::default() };
        let context = (&raw mut world).cast();
        let object_ptr = &raw mut object;
        assert_eq!(unsafe { save(state, context, SERVICES, object_view(object_ptr)) }, Err(37));
        assert_eq!(unsafe { data_size(state) }, 4, "failed capture owns only initialized bytes");
        unsafe { (*object_ptr).value = 73; }
        assert_eq!(unsafe { apply(state, context, SERVICES) }, Err(37));
        unsafe { (*context.cast::<World>()).fail_capture = false; }
        unsafe { preload(state, context, SERVICES, object_ptr.cast()) }.unwrap();
        assert_eq!(unsafe { (*object_ptr).value }, 73, "failed restore remains guarded against reentry");
        unsafe { apply(state, context, SERVICES) }.unwrap();
        assert_eq!(unsafe { (*object_ptr).value }, 41);
        unsafe { destroy(state) }.unwrap();
    }


    #[test]
    fn history_nested_continue_redo_eviction_reset_and_shell_lifetimes() {
        let state = unsafe { history_create(4, arrays::system_allocator()) };
        let mut world = World::default();
        let context = (&raw mut world).cast();
        let command = |op| unsafe { history_command(state, context, SERVICES, op, TITLE.as_ptr()) };
        command(0).unwrap();
        command(1).unwrap();
        let first = world.undo;
        command(1).unwrap();
        command(2).unwrap();
        assert_eq!(world.undo, first, "nested end retains active shell");
        command(2).unwrap();
        assert!(world.undo.is_null());
        command(3).unwrap();
        assert_eq!(world.undo, first, "continue retains shell identity");
        command(2).unwrap();
        let mut object = Object { value: 1, ..Object::default() };
        let object_ptr = &raw mut object;
        unsafe { (*context.cast::<World>()).transaction = first; }
        unsafe { save(first, context, SERVICES, object_view(object_ptr)) }.unwrap();
        command(1).unwrap();
        assert_eq!(world.destroyed, 0, "equal budget is not evicted");
        let second = world.undo;
        unsafe { (*context.cast::<World>()).transaction = second; }
        unsafe { save(second, context, SERVICES, object_view(object_ptr)) }.unwrap();
        command(2).unwrap();
        command(6).unwrap();
        assert_eq!(command(5).unwrap(), 0);
        command(1).unwrap();
        assert_eq!(world.destroyed, 1, "new begin removes redo history");
        let third = world.undo;
        unsafe { (*context.cast::<World>()).transaction = third; }
        unsafe { save(third, context, SERVICES, object_view(object_ptr)) }.unwrap();
        command(2).unwrap();
        command(1).unwrap();
        assert_eq!(world.destroyed, 2, "strictly over budget evicts oldest snapshot");
        command(2).unwrap();
        command(0).unwrap();
        assert_eq!(command(4).unwrap(), 2);
        let reason = unsafe { history_reset_reason(state) };
        assert_eq!(unsafe { *(*reason).data.cast::<Tchar>() }, TITLE[0]);
        unsafe { history_destroy(state, context, SERVICES) }.unwrap();
        assert_eq!(world.destroyed, 4, "each SDK shell is destroyed once");
    }

    #[test]
    fn gc_partial_read_releases_typed_temporary_and_preserves_first_failure() {
        struct Scratch { owned: Option<Box<i32>> }
        unsafe extern "C" fn failing(context: *mut c_void, leaf: Hp2TransRecordLeaf,
            _a: usize, _b: usize, _out: *mut usize) -> i32 {
            let scratch = unsafe { &mut *context.cast::<Scratch>() };
            use Hp2TransRecordLeaf::*;
            match leaf {
                PseudoBegin | ArrayTempBegin => { scratch.owned = Some(Box::new(7)); 0 }
                PseudoSerializeReader | ArraySerializerReader => 37,
                PseudoEnd | ArrayDestructorTemp => { drop(scratch.owned.take()); 91 }
                _ => 0,
            }
        }
        let services = Hp2TransRecordServices { call: failing };
        let mut buffer = Buffer::new(arrays::system_allocator());
        let mut offset = 0;
        let mut value = 7i32;
        unsafe { bytes((&raw mut buffer).cast(), &mut offset, (&raw mut value).cast(), 4, false) }.unwrap();
        for is_array in [false, true] {
            let mut scratch = Scratch { owned: None };
            let mut view = object_view(ptr::null_mut());
            if is_array { view.array = (&raw mut value).cast(); view.count = 1; }
            view.data = (&raw mut buffer).cast();
            assert_eq!(unsafe { visit((&raw mut scratch).cast(), &services, view, 2, false) }, Err(37));
            assert!(scratch.owned.is_none(), "partial SDK temporary must be destroyed");
        }
    }

    #[test]
    fn history_teardown_releases_all_entries_without_replacing_first_failure() {
        let state = unsafe { history_create(1024, arrays::system_allocator()) };
        let mut world = World::default();
        let context = (&raw mut world).cast();
        for _ in 0..2 {
            unsafe { history_command(state, context, SERVICES, 1, TITLE.as_ptr()) }.unwrap();
            unsafe { history_command(state, context, SERVICES, 2, ptr::null()) }.unwrap();
        }
        unsafe { (*context.cast::<World>()).fail_destroy = true; }
        assert_eq!(unsafe { history_destroy(state, context, SERVICES) }, Err(37));
        assert_eq!(world.destroyed, 2, "later shells must still be released after the first failure");
    }
}
