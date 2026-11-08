//! Game-runtime Editor transaction classes over Core's shared transaction kernel.
//!
//! The native game registers these classes but never creates GEditor
//! (`Launch/Src/HP2EditorRuntime.cpp`). Snapshots, history, native references,
//! temporary-object GC and object-array changes are real Core resources.

use hp2_abi::engine::generated::{TArrayRaw, TTransArrayRaw, UClass, UModel, UObject};
use hp2_abi::render_span::FMemMark;
use hp2_core::{
    TemporaryObject,
    array_storage::{self, Hp2ArraySlots},
    core_access::{
        CoreAccess, ObjectArrayTransactionOperation, ObjectTransactions, RuntimeError,
        RuntimeResult,
    },
    core_class_specs::{NativeClassSpec, NativeRecordKind},
    object_archive::{ArchiveFlags, ArchiveMapping, BorrowedArchiveCursor, ObjectArchive},
    object_owner::ClassOps,
    transaction::{
        self, History, Hp2TransRecordLeaf, Hp2TransRecordServices, Hp2TransRecordView,
        Hp2TransactionLeaf, Hp2TransactionResult, Hp2TransactionServices, Transaction,
    },
};
use std::{
    alloc::Layout,
    cell::{Cell, RefCell},
    ffi::c_void,
    ptr,
    rc::Rc,
};

const OBJECT: ClassOps = NativeRecordKind::Object.operations();
const OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
#[repr(C)]
struct TransBufferRecord {
    base: UObject,
    history: *mut History,
}
const BUFFER_OPS: ClassOps = {
    let mut ops = OBJECT;
    ops.construct = construct;
    ops.serialize = serialize;
    ops.destroy = destroy;
    ops
};
pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[
    NativeClassSpec {
        short_name: "Transactor",
        package: "Editor",
        super_name: Some("Core.Object"),
        within: "Core.Object",
        flags: 9,
        object_flags: OBJECT_FLAGS,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<UObject>()),
        operations: Some(OBJECT),
        properties: &[],
        defaults: &[],
    },
    // DECLARE_CLASS names UObject, despite the C++ virtual UTransactor interface.
    NativeClassSpec {
        short_name: "TransBuffer",
        package: "Editor",
        super_name: Some("Core.Object"),
        within: "Core.Object",
        flags: 8,
        object_flags: OBJECT_FLAGS,
        config: "System",
        guid: [0; 4],
        kind: NativeRecordKind::Object,
        layout: Some(Layout::new::<TransBufferRecord>()),
        operations: Some(BUFFER_OPS),
        properties: &[],
        defaults: &[],
    },
];

fn status_error(status: i32) -> RuntimeError {
    RuntimeError::new("Canonical transaction operation failed").with_context(&status.to_string())
}
fn defaults() -> ArchiveFlags {
    let c = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
    ArchiveFlags::new(c.version as u16, c.licensee as u16, false)
}

struct Context<'a> {
    access: CoreAccess<'a>,
    shell: *const Shell,
    archive: *const c_void,
    error: RefCell<Option<RuntimeError>>,
}
impl Context<'_> {
    fn fail(&self, error: RuntimeError) -> i32 {
        let mut first = self.error.borrow_mut();
        if first.is_none() {
            *first = Some(error)
        };
        1
    }
    fn archive(&self) -> RuntimeResult<&ObjectArchive<'_>> {
        if self.archive.is_null() {
            Err(RuntimeError::new(
                "Transaction serialization has no archive",
            ))
        } else {
            Ok(unsafe { &*self.archive.cast::<ObjectArchive<'_>>() })
        }
    }
}
fn run<T>(
    access: CoreAccess<'_>,
    shell: *const Shell,
    archive: Option<&ObjectArchive<'_>>,
    operation: impl FnOnce(*mut c_void) -> Result<T, i32>,
) -> RuntimeResult<T> {
    let mut context = Context {
        access,
        shell,
        archive: archive.map_or(ptr::null(), |a| ptr::from_ref(a).cast()),
        error: RefCell::new(None),
    };
    let result = operation(ptr::from_mut(&mut context).cast());
    match (result, context.error.into_inner()) {
        (_, Some(error)) => Err(error),
        (Ok(value), None) => Ok(value),
        (Err(status), None) => Err(status_error(status)),
    }
}

struct Shell {
    state: *mut Transaction,
    owns_state: bool,
}
impl Drop for Shell {
    fn drop(&mut self) {
        if self.owns_state {
            let _ = unsafe { transaction::destroy(self.state) };
        }
    }
}
impl ObjectTransactions for Shell {
    unsafe fn save_object(
        &self,
        access: CoreAccess<'_>,
        object: *mut UObject,
    ) -> RuntimeResult<()> {
        if object.is_null() {
            return Err(RuntimeError::new(
                "Transaction SaveObject requires an object",
            ));
        }
        let view = Hp2TransRecordView {
            object: object.cast(),
            array: ptr::null_mut(),
            index: 0,
            count: 0,
            oper: 0,
            element_size: 0,
            serializer: ptr::null_mut(),
            destructor: ptr::null_mut(),
            data: ptr::null_mut(),
        };
        run(access, self, None, |context| unsafe {
            transaction::save(self.state, context, SERVICES, view)
        })
    }
    unsafe fn save_object_array_range(
        &self,
        access: CoreAccess<'_>,
        owner: *mut UObject,
        array: *mut TTransArrayRaw,
        index: i32,
        count: i32,
        operation: ObjectArrayTransactionOperation,
        element_size: usize,
    ) -> RuntimeResult<()> {
        let view = Hp2TransRecordView {
            object: owner.cast(),
            array: array.cast(),
            index,
            count,
            oper: match operation {
                ObjectArrayTransactionOperation::Modify => 0,
                ObjectArrayTransactionOperation::Add => 1,
            },
            element_size: element_size as i32,
            serializer: pointer_serialize as *const () as *mut c_void,
            destructor: pointer_destroy as *const () as *mut c_void,
            data: ptr::null_mut(),
        };
        run(access, self, None, |context| unsafe {
            transaction::save(self.state, context, SERVICES, view)
        })
    }
}

/// Owned equivalent of an internal FTransaction shell, with real canonical snapshots.
pub struct InternalTransaction {
    shell: Rc<Shell>,
}
impl InternalTransaction {
    /// # Safety
    /// Object remains live through recursive capture and preload.
    pub unsafe fn save_object(
        &self,
        access: CoreAccess<'_>,
        object: *mut UObject,
    ) -> RuntimeResult<()> {
        unsafe { self.shell.save_object(access, object) }
    }
    /// # Safety
    /// Snapshot objects remain live through apply and post-load callbacks.
    pub unsafe fn apply(&self, access: CoreAccess<'_>) -> RuntimeResult<()> {
        run(access, Rc::as_ptr(&self.shell), None, |ctx| unsafe {
            transaction::apply(self.shell.state, ctx, SERVICES)
        })
    }
    /// # Safety
    /// Object remains live for synchronous recursive preload.
    pub unsafe fn preload(
        &self,
        access: CoreAccess<'_>,
        object: *mut UObject,
    ) -> RuntimeResult<()> {
        run(access, Rc::as_ptr(&self.shell), None, |ctx| unsafe {
            transaction::preload(self.shell.state, ctx, SERVICES, object.cast())
        })
    }
    /// # Safety
    /// Referenced objects and external archive remain live during traversal.
    pub unsafe fn serialize(
        &self,
        access: CoreAccess<'_>,
        archive: &ObjectArchive<'_>,
    ) -> RuntimeResult<()> {
        run(
            access,
            Rc::as_ptr(&self.shell),
            Some(archive),
            |ctx| unsafe { transaction::serialize(self.shell.state, ctx, SERVICES) },
        )
    }
}
/// Source CreateInternalTransaction uses title Internal and Flip=false.
pub fn create_internal_transaction() -> RuntimeResult<InternalTransaction> {
    let state = unsafe {
        transaction::create(
            hp2_core::lit!("Internal").as_ptr().cast(),
            false,
            array_storage::system_allocator(),
        )
    }
    .map_err(status_error)?;
    Ok(InternalTransaction {
        shell: Rc::new(Shell {
            state,
            owns_state: true,
        }),
    })
}

unsafe fn history(object: *mut UObject) -> RuntimeResult<*mut History> {
    if object.is_null() {
        return Err(RuntimeError::new("Null transaction buffer"));
    }
    let state =
        unsafe { ptr::addr_of!((*object.cast::<TransBufferRecord>()).history).read_unaligned() };
    if state.is_null() {
        Err(RuntimeError::new(
            "Transaction buffer requires its MaxMemory constructor",
        ))
    } else {
        Ok(state)
    }
}
unsafe fn command(
    access: CoreAccess<'_>,
    object: *mut UObject,
    operation: i32,
    name: *const i32,
    archive: Option<&ObjectArchive<'_>>,
) -> RuntimeResult<usize> {
    let state = unsafe { history(object) }?;
    run(access, ptr::null(), archive, |context| unsafe {
        transaction::history_command(state, context, SERVICES, operation, name)
    })
}
unsafe fn check_state(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    if hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.checks != 0 {
        let len = unsafe { command(access, object, 13, ptr::null(), None) }?;
        let undo = unsafe { command(access, object, 12, ptr::null(), None) }?;
        let active = unsafe { command(access, object, 11, ptr::null(), None) }? as i32;
        if len < undo || active < 0 {
            return Err(RuntimeError::new("Invalid transaction history state"));
        }
    }
    Ok(())
}
/// Constructs the source MaxMemory variant; the protected default constructor has no state.
pub unsafe fn create_buffer(
    access: CoreAccess<'_>,
    max_memory: usize,
) -> RuntimeResult<*mut UObject> {
    let object = unsafe {
        access.construct_object(
            access.native_class("Editor.TransBuffer")?,
            access.transient_package(),
            0,
            0,
            ptr::null_mut(),
        )
    }?;
    unsafe {
        ptr::addr_of_mut!((*object.cast::<TransBufferRecord>()).history).write_unaligned(
            transaction::history_create(max_memory, array_storage::system_allocator()),
        )
    };
    let initialized = unsafe { reset(access, object, hp2_core::lit!("startup")) }
        .and_then(|()| unsafe { check_state(access, object) })
        .and_then(|()| {
            access.emit_log(
                access.intern_str("Init", hp2_core::name_owner::FindName::Find)?,
                &hp2_core::lit!("Transaction tracking system initialized")[..39],
            )
        });
    if let Err(error) = initialized {
        let _ = unsafe { access.delete_object(object) };
        return Err(error);
    }
    Ok(object)
}
pub unsafe fn reset(
    access: CoreAccess<'_>,
    object: *mut UObject,
    reason: &[u32],
) -> RuntimeResult<()> {
    unsafe { check_state(access, object) }?;
    if hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.checks != 0
        && unsafe { command(access, object, 11, ptr::null(), None) }? != 0
    {
        return Err(RuntimeError::new("Cannot reset an active transaction"));
    }
    unsafe {
        command(access, object, 0, reason.as_ptr().cast(), None)?;
        check_state(access, object)
    }
}
pub unsafe fn begin(
    access: CoreAccess<'_>,
    object: *mut UObject,
    name: &[u32],
) -> RuntimeResult<()> {
    unsafe {
        check_state(access, object)?;
        command(access, object, 1, name.as_ptr().cast(), None)?;
        check_state(access, object)
    }
}
pub unsafe fn end(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { check_state(access, object) }?;
    if hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.checks != 0
        && unsafe { command(access, object, 11, ptr::null(), None) }? < 1
    {
        return Err(RuntimeError::new("Cannot end an inactive transaction"));
    }
    unsafe {
        command(access, object, 2, ptr::null(), None)?;
        check_state(access, object)
    }
}
pub unsafe fn continue_transaction(
    access: CoreAccess<'_>,
    object: *mut UObject,
) -> RuntimeResult<()> {
    unsafe {
        check_state(access, object)?;
        command(access, object, 3, ptr::null(), None)?;
        check_state(access, object)
    }
}
pub unsafe fn can_undo(
    access: CoreAccess<'_>,
    object: *mut UObject,
    reason: *mut TArrayRaw,
) -> RuntimeResult<bool> {
    unsafe { check_state(access, object) }?;
    let code = unsafe { command(access, object, 4, ptr::null(), None) }?;
    if !reason.is_null() {
        match code {
            1 => unsafe {
                access.assign_fstring(reason, hp2_core::lit!("Can't undo during a transaction"))
            }?,
            2 => {
                unsafe { access.assign_fstring(reason, hp2_core::lit!("Can't undo after ")) }?;
                let state = unsafe { history(object) }?;
                let text = unsafe {
                    hp2_core::native_text::fstring_units(transaction::history_reset_reason(state))
                };
                unsafe { access.append_vm_fstring_result(reason, text) }?
            }
            _ => {}
        }
    }
    Ok(code == 0)
}
pub unsafe fn can_redo(
    access: CoreAccess<'_>,
    object: *mut UObject,
    reason: *mut TArrayRaw,
) -> RuntimeResult<bool> {
    unsafe { check_state(access, object) }?;
    let code = unsafe { command(access, object, 5, ptr::null(), None) }?;
    if !reason.is_null() {
        let text = match code {
            1 => Some(hp2_core::lit!("Can't redo during a transaction").as_slice()),
            2 => Some(hp2_core::lit!("Nothing to redo").as_slice()),
            _ => None,
        };
        if let Some(text) = text {
            unsafe { access.assign_fstring(reason, text) }?
        }
    }
    Ok(code == 0)
}
pub unsafe fn undo(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<bool> {
    unsafe {
        check_state(access, object)?;
        let result = command(access, object, 6, ptr::null(), None)?;
        check_state(access, object)?;
        Ok(result != 0)
    }
}
pub unsafe fn redo(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<bool> {
    unsafe {
        check_state(access, object)?;
        let result = command(access, object, 7, ptr::null(), None)?;
        check_state(access, object)?;
        Ok(result != 0)
    }
}
pub unsafe fn undo_data_size(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<usize> {
    unsafe { command(access, object, 9, ptr::null(), None) }
}

unsafe fn construct(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe {
        (OBJECT.construct)(access, object)?;
        ptr::addr_of_mut!((*object.cast::<TransBufferRecord>()).history)
            .write_unaligned(ptr::null_mut())
    };
    Ok(())
}
unsafe fn serialize(
    access: CoreAccess<'_>,
    object: *mut UObject,
    archive: &ObjectArchive<'_>,
) -> RuntimeResult<()> {
    unsafe {
        check_state(access, object)?;
        (OBJECT.serialize)(access, object, archive)?;
        command(access, object, 8, ptr::null(), Some(archive))?;
        check_state(access, object)
    }
}
unsafe fn destroy(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { check_state(access, object) }?;
    access.emit_log(
        access.intern_str("Exit", hp2_core::name_owner::FindName::Find)?,
        &hp2_core::lit!("Transaction tracking system shut down")[..37],
    )?;
    let state = unsafe {
        ptr::addr_of_mut!((*object.cast::<TransBufferRecord>()).history).replace(ptr::null_mut())
    };
    let released = run(access, ptr::null(), None, |ctx| unsafe {
        transaction::history_destroy(state, ctx, SERVICES)
    });
    let base = unsafe { (OBJECT.destroy)(access, object) };
    released.and(base)
}

struct RawMapping;
impl ArchiveMapping for RawMapping {
    fn load_object(&self, _: i32) -> RuntimeResult<*mut UObject> {
        Err(RuntimeError::new(
            "Transaction snapshots contain native object pointers, not package indices",
        ))
    }
    fn save_object(&self, _: *mut UObject) -> RuntimeResult<i32> {
        Err(RuntimeError::new(
            "Transaction snapshots contain native object pointers, not package indices",
        ))
    }
    fn load_name(&self, _: i32) -> RuntimeResult<i32> {
        Err(RuntimeError::new(
            "Transaction snapshots contain global FName indices",
        ))
    }
    fn save_name(&self, _: i32) -> RuntimeResult<i32> {
        Err(RuntimeError::new(
            "Transaction snapshots contain global FName indices",
        ))
    }
    fn visit_object(&self, _: *mut *mut UObject) -> RuntimeResult<()> {
        Err(RuntimeError::new(
            "Transaction snapshot traversal requires its external reference archive",
        ))
    }
    fn visit_name(&self, _: i32) -> RuntimeResult<()> {
        Err(RuntimeError::new(
            "Transaction snapshot traversal requires its external reference archive",
        ))
    }
}
struct SnapshotCursor<'a> {
    access: CoreAccess<'a>,
    owner: *const Shell,
    buffer: *mut c_void,
    offset: Cell<i32>,
    loading: bool,
    error: Cell<bool>,
}
impl<'a> SnapshotCursor<'a> {
    fn new(
        access: CoreAccess<'a>,
        owner: *const Shell,
        buffer: *mut c_void,
        loading: bool,
    ) -> Self {
        Self {
            access,
            owner,
            buffer,
            offset: Cell::new(0),
            loading,
            error: Cell::new(false),
        }
    }
    unsafe fn transfer(&self, data: *mut c_void, count: usize) -> RuntimeResult<()> {
        let count = i32::try_from(count)
            .map_err(|_| RuntimeError::new("Transaction byte count exceeds INT"))?;
        let mut offset = self.offset.get();
        let result =
            unsafe { transaction::bytes(self.buffer, &mut offset, data, count, self.loading) }
                .map_err(status_error);
        self.offset.set(offset);
        if result.is_err() {
            self.error.set(true)
        };
        result
    }
}
impl BorrowedArchiveCursor for SnapshotCursor<'_> {
    // The source FTransactionReader/Writer inherit INDEX_NONE Tell/TotalSize and empty Seek.
    fn tell(&self) -> RuntimeResult<usize> {
        Ok(usize::MAX)
    }
    fn total_size(&self) -> RuntimeResult<usize> {
        Ok(usize::MAX)
    }
    fn seek(&self, _: usize) -> RuntimeResult<()> {
        Ok(())
    }
    fn is_error(&self) -> RuntimeResult<bool> {
        Ok(self.error.get())
    }
    fn read(&self, output: &mut [u8]) -> RuntimeResult<()> {
        unsafe { self.transfer(output.as_mut_ptr().cast(), output.len()) }
    }
    fn write(&self, input: &[u8]) -> RuntimeResult<()> {
        unsafe { self.transfer(input.as_ptr().cast_mut().cast(), input.len()) }
    }
    unsafe fn serialize_name(&self, index: *mut i32) -> RuntimeResult<()> {
        unsafe { self.transfer(index.cast(), size_of::<i32>()) }
    }
    unsafe fn preload_object(&self, object: *mut UObject) -> RuntimeResult<()> {
        if self.loading && !self.owner.is_null() {
            run(self.access, self.owner, None, |context| unsafe {
                transaction::preload((*self.owner).state, context, SERVICES, object.cast())
            })?;
        }
        Ok(())
    }
}

type Serializer =
    for<'a, 'b> unsafe fn(CoreAccess<'a>, &ObjectArchive<'b>, *mut u8) -> RuntimeResult<()>;
type Destructor = for<'a> unsafe fn(CoreAccess<'a>, *mut u8) -> RuntimeResult<()>;
unsafe fn pointer_serialize(
    _: CoreAccess<'_>,
    archive: &ObjectArchive<'_>,
    slot: *mut u8,
) -> RuntimeResult<()> {
    unsafe { archive.serialize_object_slot(slot.cast()) }
}
unsafe fn pointer_destroy(_: CoreAccess<'_>, _: *mut u8) -> RuntimeResult<()> {
    Ok(())
}
unsafe fn slots(array: *mut c_void) -> Hp2ArraySlots {
    let raw = array.cast::<TArrayRaw>();
    unsafe {
        Hp2ArraySlots {
            data: ptr::addr_of_mut!((*raw).data),
            num: ptr::addr_of_mut!((*raw).num),
            max: ptr::addr_of_mut!((*raw).max),
        }
    }
}

struct RecordContext<'a> {
    access: CoreAccess<'a>,
    view: Hp2TransRecordView,
    current: *const c_void,
    reader: *const c_void,
    writer: *const c_void,
    external: *const c_void,
    pseudo: Option<TemporaryObject<'a>>,
    temp: *mut u8,
    mark: Option<FMemMark>,
    error: RefCell<Option<RuntimeError>>,
}
impl RecordContext<'_> {
    fn archive(&self, which: *const c_void) -> RuntimeResult<&ObjectArchive<'_>> {
        if which.is_null() {
            Err(RuntimeError::new("Missing active transaction archive"))
        } else {
            Ok(unsafe { &*which.cast::<ObjectArchive<'_>>() })
        }
    }
    fn fail(&self, error: RuntimeError) -> i32 {
        let mut first = self.error.borrow_mut();
        if first.is_none() {
            *first = Some(error)
        };
        1
    }
}
impl Drop for RecordContext<'_> {
    fn drop(&mut self) {
        drop(self.pseudo.take());
        if let Some(mark) = self.mark.take() {
            unsafe { self.access.scratch_memory().pop(&mark) }
        }
    }
}
unsafe fn visit_record(
    context: *mut Context<'_>,
    view: Hp2TransRecordView,
    flip: *mut c_void,
    mode: i32,
) -> RuntimeResult<()> {
    let access = unsafe { (*context).access };
    let owner = if mode == 1 {
        unsafe { (*context).shell }
    } else {
        ptr::null()
    };
    let reader_cursor = SnapshotCursor::new(access, owner, view.data, true);
    let writer_cursor = SnapshotCursor::new(
        access,
        ptr::null(),
        if mode == 0 { view.data } else { flip },
        false,
    );
    let mut read_flags = defaults();
    read_flags.transaction_loading = true;
    let reader = ObjectArchive::transaction_borrowed(&reader_cursor, read_flags, &RawMapping);
    let writer = ObjectArchive::transaction_borrowed(&writer_cursor, defaults(), &RawMapping);
    let reader = ptr::from_ref(&reader).cast::<c_void>();
    let writer = ptr::from_ref(&writer).cast::<c_void>();
    let mark = if mode == 2 {
        Some(unsafe { access.scratch_memory().mark() })
    } else {
        None
    };
    let mut record = RecordContext {
        access,
        view,
        current: if mode == 0 { writer } else { ptr::null() },
        reader,
        writer,
        external: unsafe { (*context).archive },
        pseudo: None,
        temp: ptr::null_mut(),
        mark,
        error: RefCell::new(None),
    };
    let result = unsafe {
        transaction::visit(
            ptr::from_mut(&mut record).cast(),
            &RECORD_SERVICES,
            view,
            mode,
            !flip.is_null(),
        )
    };
    if let Some(error) = record.error.borrow_mut().take() {
        return Err(error);
    }
    result.map_err(status_error)
}
unsafe extern "C" fn record_leaf(
    raw: *mut c_void,
    leaf: Hp2TransRecordLeaf,
    a: usize,
    b: usize,
    _out: *mut usize,
) -> i32 {
    let record = raw.cast::<RecordContext<'_>>();
    let operation = (|| -> RuntimeResult<()> {
        unsafe {
            let access = (*record).access;
            let view = (*record).view;
            match leaf {
                Hp2TransRecordLeaf::ValidateContents => {
                    if hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.slow_checks != 0 {
                        let valid = if view.array.is_null() {
                            view.index == 0 && view.element_size == 0 && view.serializer.is_null()
                        } else {
                            let class = (*view.object.cast::<UObject>()).class.cast::<UClass>();
                            let start = view.object as usize + size_of::<UObject>();
                            let end = view.object as usize + (*class).properties_size as usize;
                            view.array as usize >= start
                                && (view.array as usize + size_of::<TArrayRaw>()) <= end
                                && view.element_size != 0
                                && !view.serializer.is_null()
                                && view.index >= 0
                                && view.count >= 0
                        };
                        if !valid {
                            return Err(RuntimeError::new("Invalid transaction record contents"));
                        }
                    }
                }
                Hp2TransRecordLeaf::ValidateRange => {
                    if hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.slow_checks != 0 {
                        let count =
                            ptr::addr_of!((*view.array.cast::<TArrayRaw>()).num).read_unaligned();
                        if view.index + view.count > count {
                            return Err(RuntimeError::new("Invalid transaction array range"));
                        }
                    }
                }
                Hp2TransRecordLeaf::Destructor => {
                    let f: Destructor = std::mem::transmute(view.destructor);
                    let data = ptr::addr_of!((*view.array.cast::<TArrayRaw>()).data)
                        .read_unaligned()
                        .cast::<u8>();
                    f(access, data.add(a * view.element_size as usize))?;
                }
                Hp2TransRecordLeaf::Serializer => {
                    let f: Serializer = std::mem::transmute(view.serializer);
                    let data = ptr::addr_of!((*view.array.cast::<TArrayRaw>()).data)
                        .read_unaligned()
                        .cast::<u8>();
                    let archive = (*record).archive((*record).current)?;
                    f(access, archive, data.add(a * view.element_size as usize))?;
                }
                Hp2TransRecordLeaf::ArrayRemove => array_storage::remove(
                    slots(view.array),
                    a as i32,
                    b as i32,
                    view.element_size,
                    ptr::null(),
                    array_storage::system_allocator(),
                )
                .map_err(status_error)?,
                Hp2TransRecordLeaf::ArrayInsertZeroed => array_storage::insert_zeroed(
                    slots(view.array),
                    a as i32,
                    b as i32,
                    view.element_size,
                    ptr::null(),
                    array_storage::system_allocator(),
                )
                .map_err(status_error)?,
                Hp2TransRecordLeaf::ObjectSerialize => {
                    let archive = (*record).archive((*record).current)?;
                    access.serialize_object(view.object.cast(), archive)?;
                }
                Hp2TransRecordLeaf::SelectWriterFlip => (*record).current = (*record).writer,
                Hp2TransRecordLeaf::SelectReaderData => (*record).current = (*record).reader,
                Hp2TransRecordLeaf::PseudoBegin => {
                    let class = (*view.object.cast::<UObject>()).class.cast::<UClass>();
                    let temporary = access.temporary_object(class)?;
                    (*record).pseudo = Some(temporary);
                }
                Hp2TransRecordLeaf::PseudoSerializeReader
                | Hp2TransRecordLeaf::PseudoSerializeAr => {
                    let object = (*record)
                        .pseudo
                        .as_ref()
                        .ok_or_else(|| RuntimeError::new("Missing transaction temporary object"))?
                        .as_ptr();
                    let which = if leaf == Hp2TransRecordLeaf::PseudoSerializeReader {
                        (*record).reader
                    } else {
                        (*record).external
                    };
                    let archive = (*record).archive(which)?;
                    access.serialize_object(object, archive)?;
                }
                Hp2TransRecordLeaf::PseudoEnd => {
                    let owner = (*record).pseudo.take().ok_or_else(|| {
                        RuntimeError::new("Missing transaction temporary lifetime")
                    })?;
                    owner.finish()?;
                }
                Hp2TransRecordLeaf::ArrayTempBegin => {
                    (*record).temp = access.scratch_memory().push_bytes(view.element_size, 16)?;
                    ptr::write_bytes((*record).temp, 0, view.element_size as usize);
                }
                Hp2TransRecordLeaf::ArrayElementZero => {
                    ptr::write_bytes((*record).temp, 0, view.element_size as usize)
                }
                Hp2TransRecordLeaf::ArraySerializerReader
                | Hp2TransRecordLeaf::ArraySerializerAr => {
                    let which = if leaf == Hp2TransRecordLeaf::ArraySerializerReader {
                        (*record).reader
                    } else {
                        (*record).external
                    };
                    let archive = (*record).archive(which)?;
                    let f: Serializer = std::mem::transmute(view.serializer);
                    f(access, archive, (*record).temp)?;
                }
                Hp2TransRecordLeaf::ArrayDestructorTemp => {
                    let f: Destructor = std::mem::transmute(view.destructor);
                    f(access, (*record).temp)?;
                }
            }
            Ok(())
        }
    })();
    match operation {
        Ok(()) => 0,
        Err(error) => unsafe { (*record).fail(error) },
    }
}
const RECORD_SERVICES: Hp2TransRecordServices = Hp2TransRecordServices { call: record_leaf };

unsafe extern "C" fn service(
    raw: *mut c_void,
    leaf: Hp2TransactionLeaf,
    a: *mut c_void,
    b: *mut c_void,
    out: *mut Hp2TransactionResult,
) -> i32 {
    let context = raw.cast::<Context<'_>>();
    let operation = (|| -> RuntimeResult<()> {
        unsafe {
            let access = (*context).access;
            match leaf {
                Hp2TransactionLeaf::CaptureRecord
                | Hp2TransactionLeaf::RestoreRecord
                | Hp2TransactionLeaf::GcRecord => {
                    let mode = match leaf {
                        Hp2TransactionLeaf::CaptureRecord => 0,
                        Hp2TransactionLeaf::RestoreRecord => 1,
                        _ => 2,
                    };
                    visit_record(
                        context,
                        a.cast::<Hp2TransRecordView>().read_unaligned(),
                        b,
                        mode,
                    )?;
                }
                Hp2TransactionLeaf::InitObject => {
                    (*a.cast::<UObject>()).object_flags |=
                        hp2_core::object_lifecycle::RF_NEED_POST_LOAD
                }
                Hp2TransactionLeaf::NeedsPostLoad => {
                    (*out).integer = usize::from(
                        (*a.cast::<UObject>()).object_flags
                            & hp2_core::object_lifecycle::RF_NEED_POST_LOAD
                            != 0,
                    )
                }
                Hp2TransactionLeaf::PostLoad => access.conditional_post_load(a.cast())?,
                Hp2TransactionLeaf::ObjectModel => {
                    let model = access.native_class("Engine.Model")?;
                    (*out).pointer = if hp2_core::object_model::is_a(a, model.cast()) {
                        a
                    } else {
                        ptr::null_mut()
                    };
                }
                // The first-party game runtime explicitly initializes GEditor to NULL
                // and never constructs an Editor engine (HP2EditorRuntime.cpp11–12).
                Hp2TransactionLeaf::EditorAvailable => (*out).integer = 0,
                Hp2TransactionLeaf::ModelNodeCount => {
                    (*out).integer = ptr::addr_of!((*a.cast::<UModel>()).nodes.array_num)
                        .read_unaligned() as usize
                }
                Hp2TransactionLeaf::BuildBounds | Hp2TransactionLeaf::Finish => {
                    return Err(RuntimeError::new(
                        "Editor transaction completion requires a live GEditor; the game runtime has none",
                    ));
                }
                Hp2TransactionLeaf::SerializeText => {
                    let archive = (*context).archive()?;
                    access.serialize_owned_fstring(archive, a.cast())?;
                }
                Hp2TransactionLeaf::SerializeObject => {
                    let archive = (*context).archive()?;
                    archive.serialize_object_slot(a.cast())?;
                }
                Hp2TransactionLeaf::SerializeInt => {
                    let archive = (*context).archive()?;
                    let mut value = a.cast::<i32>().read_unaligned();
                    archive.serialize_i32(&mut value)?;
                    a.cast::<i32>().write_unaligned(value);
                }
                Hp2TransactionLeaf::SerializeCount => {
                    let archive = (*context).archive()?;
                    let mut value = a.cast::<i32>().read_unaligned();
                    archive.serialize_compact_index(&mut value)?;
                    a.cast::<i32>().write_unaligned(value);
                }
                Hp2TransactionLeaf::CreateShell => {
                    (*out).pointer = Rc::into_raw(Rc::new(Shell {
                        state: a.cast(),
                        owns_state: false,
                    }))
                    .cast_mut()
                    .cast()
                }
                Hp2TransactionLeaf::DestroyShell => drop(Rc::from_raw(a.cast::<Shell>())),
                Hp2TransactionLeaf::ApplyShell => {
                    let shell = a.cast::<Shell>();
                    run(access, shell, None, |ctx| {
                        transaction::apply((*shell).state, ctx, SERVICES)
                    })?;
                }
                Hp2TransactionLeaf::SerializeShell => {
                    let shell = a.cast::<Shell>();
                    let archive = (*context).archive()?;
                    run(access, shell, Some(archive), |ctx| {
                        transaction::serialize((*shell).state, ctx, SERVICES)
                    })?;
                }
                Hp2TransactionLeaf::SetUndo => {
                    if a.is_null() {
                        access.set_transactions(None)
                    } else {
                        let shell = a.cast::<Shell>();
                        Rc::increment_strong_count(shell);
                        let shell: Rc<dyn ObjectTransactions> = Rc::from_raw(shell);
                        access.set_transactions(Some(shell));
                    }
                }
                Hp2TransactionLeaf::Log => {
                    let redo = b.cast::<i32>().read_unaligned() != 0;
                    let message = hp2_core::native_text::fmt(&[
                        hp2_core::native_text::TextPart::L(if redo { "Redo " } else { "Undo " }),
                        hp2_core::native_text::TextPart::P(a.cast()),
                    ]);
                    access.emit_log(
                        access.intern_str("Log", hp2_core::name_owner::FindName::Find)?,
                        &message[..message.len() - 1],
                    )?;
                }
            }
            Ok(())
        }
    })();
    match operation {
        Ok(()) => 0,
        Err(error) => unsafe { (*context).fail(error) },
    }
}
const SERVICES: Hp2TransactionServices = Hp2TransactionServices { call: service };
