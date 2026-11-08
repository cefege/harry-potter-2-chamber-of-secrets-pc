//! Rust-owned object-reference archive operations over bounded package or memory cursors.
use crate::core_access::{RuntimeError, RuntimeResult};
use std::cell::{Cell, RefCell};
use crate::package_archive::{ExportArchive, PackageArchiveWriter};
use hp2_abi::engine::generated::{UClass, UObject};
use std::ptr;

/// Runtime hooks supplied by the canonical Core/name/package owner.
pub trait ArchiveMapping {
    fn load_object(&self, index: i32) -> RuntimeResult<*mut UObject>;
    fn save_object(&self, object: *mut UObject) -> RuntimeResult<i32>;
    fn load_name(&self, local_index: i32) -> RuntimeResult<i32>;
    fn save_name(&self, global_index: i32) -> RuntimeResult<i32>;
    fn visit_object(&self, object_slot: *mut *mut UObject) -> RuntimeResult<()>;
    fn visit_name(&self, name_index: i32) -> RuntimeResult<()>;
    fn linker_identity(&self) -> Option<*mut UObject> { None }
}

/// Typed network-bit reader supplied by Engine without an FArchive vtable.
///
/// Implementations preserve the bit reader's overflow flag and partial bounded
/// values. They must not retain a mutable record borrow across mapping or VM
/// execution. Pointer operands remain live only for the individual operation.
pub trait NetworkReader {
    /// # Safety
    /// `output` spans `count` writable bytes, which need not be initialized.
    unsafe fn read_raw(&self, output: *mut u8, count: usize) -> RuntimeResult<()>;
    fn read_bounded(&self, value: &mut u32, maximum: u32) -> RuntimeResult<()>;
    /// # Safety
    /// `output` spans at least `(bit_count + 7) / 8` writable bytes.
    unsafe fn read_bits(&self, output: *mut u8, bit_count: u32) -> RuntimeResult<()>;
    fn position_bits(&self) -> usize;
    fn total_bits(&self) -> usize;
    /// # Safety
    /// The writable object slot and expected class remain live through mapping.
    unsafe fn read_object(&self, slot: *mut *mut UObject, expected_class: *mut UClass) -> RuntimeResult<i32>;
    /// # Safety
    /// The writable global-name slot remains live through mapping.
    unsafe fn read_name(&self, slot: *mut i32) -> RuntimeResult<()>;
}

/// Typed network-bit writer supplied by Engine without an FArchive vtable.
///
/// This is the saving counterpart of [`NetworkReader`]; implementations write
/// the same bounded-int, bit, object-reference, and name encodings directly
/// into the engine's packet cursor. They must not retain a mutable record
/// borrow across mapping or VM execution.
pub trait NetworkWriter {
    /// # Safety
    /// `input` spans `count` readable bytes for this call.
    unsafe fn write_raw(&self, input: *const u8, count: usize) -> RuntimeResult<()>;
    fn write_bounded(&self, value: u32, maximum: u32) -> RuntimeResult<()>;
    /// # Safety
    /// `input` spans at least `(bit_count + 7) / 8` readable bytes.
    unsafe fn write_bits(&self, input: *const u8, bit_count: u32) -> RuntimeResult<()>;
    fn position_bits(&self) -> usize;
    fn total_bits(&self) -> usize;
    /// # Safety
    /// The writable object slot and expected class remain live through mapping.
    unsafe fn write_object(&self, slot: *mut *mut UObject, expected_class: *mut UClass) -> RuntimeResult<i32>;
    /// # Safety
    /// The writable global-name slot remains live through mapping.
    unsafe fn write_name(&self, slot: *mut i32) -> RuntimeResult<()>;
}

/// A synchronous borrowed byte cursor for Engine-owned archive resources.
///
/// Implementations must never call a C++ method or retain this capability past
/// the `ObjectArchive` lifetime. Calls are made while the cursor is borrowed,
/// so they must not reenter the same `ObjectArchive`.
pub trait BorrowedArchiveCursor {
    fn tell(&self) -> RuntimeResult<usize>;
    fn seek(&self, position: usize) -> RuntimeResult<()>;
    fn total_size(&self) -> RuntimeResult<usize>;
    fn is_error(&self) -> RuntimeResult<bool>;
    fn read(&self, output: &mut [u8]) -> RuntimeResult<()>;
    fn write(&self, input: &[u8]) -> RuntimeResult<()>;
    /// # Safety
    /// `name_index` is a live writable global FName index for this operation.
    unsafe fn serialize_name(&self, name_index: *mut i32) -> RuntimeResult<()>;
    /// Source FArchive::Preload is empty unless a transaction reader supplies an owner.
    ///
    /// # Safety
    /// The object and cursor remain live during synchronous preload reentry.
    unsafe fn preload_object(&self, _object: *mut UObject) -> RuntimeResult<()> { Ok(()) }
}


/// The semantic operation performed by an archive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveMode {
    Loading,
    Saving,
    /// Counts retained allocations without saving, loading, or visiting references.
    MemoryCount,
    Reference,
    TagExports,
    TagImports,
    Transaction,
}

/// Stable archive flags consumed by Core/ClassOps and serializer consumers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchiveFlags {
    pub version: u16,
    pub licensee_version: u16,
    pub persistent: bool,
    /// For transaction memory archives, true identifies FBufferReader direction.
    pub transaction_loading: bool,
}

impl ArchiveFlags {
    pub const fn new(version: u16, licensee_version: u16, persistent: bool) -> Self {
        Self { version, licensee_version, persistent, transaction_loading: false }
    }
}

#[derive(Clone, Copy)]
enum CursorMode {
    Loading,
    Saving,
}

enum Cursor<'a> {
    Read(ExportArchive<'a>),
    Write { output: PackageArchiveWriter, origin: usize },
    Count { size: usize, map_indices: bool },
    Network(&'a dyn NetworkReader),
    NetworkWrite(&'a dyn NetworkWriter),
    Borrowed(&'a dyn BorrowedArchiveCursor, CursorMode),
    None,
}
/// Captures the source `FActorSlotCompactIndexTrace` payload for one live
/// object-reference serialization.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ActorSlotCompactIndexTrace {
    pub index: i32,
    pub offset_after: i32,
    pub captured: i32,
}

/// Object/name serialization with direct Rust mapping and no FArchive vtable.
pub struct ObjectArchive<'a> {
    mode: ArchiveMode,
    flags: ArchiveFlags,
    cursor: RefCell<Cursor<'a>>,
    mapping: &'a dyn ArchiveMapping,
    counted_bytes: Cell<(usize, usize)>,
    actor_slot_compact_index_trace: Cell<*mut ActorSlotCompactIndexTrace>,
}

impl<'a> ObjectArchive<'a> {
    /// Creates a package export reader; input cannot cross its serial payload.
    pub fn loading(input: ExportArchive<'a>, flags: ArchiveFlags, mapping: &'a dyn ArchiveMapping) -> Self {
        Self {
            mode: ArchiveMode::Loading,
            flags,
            cursor: RefCell::new(Cursor::Read(input)),
            mapping,
            counted_bytes: Cell::new((0, 0)),
            actor_slot_compact_index_trace: Cell::new(ptr::null_mut()),
        }
    }

    /// Borrows Engine's network bit cursor without copying its packet payload.
    pub fn network_loading(input: &'a dyn NetworkReader, flags: ArchiveFlags, mapping: &'a dyn ArchiveMapping) -> Self {
        Self {
            mode: ArchiveMode::Loading,
            flags,
            cursor: RefCell::new(Cursor::Network(input)),
            mapping,
            counted_bytes: Cell::new((0, 0)),
            actor_slot_compact_index_trace: Cell::new(ptr::null_mut()),
        }
    }

    /// Borrows Engine's network bit writer so reflected serialization writes
    /// directly into the live packet cursor.
    pub fn network_saving(input: &'a dyn NetworkWriter, flags: ArchiveFlags, mapping: &'a dyn ArchiveMapping) -> Self {
        Self {
            mode: ArchiveMode::Saving,
            flags,
            cursor: RefCell::new(Cursor::NetworkWrite(input)),
            mapping,
            counted_bytes: Cell::new((0, 0)),
            actor_slot_compact_index_trace: Cell::new(ptr::null_mut()),
        }
    }

    /// Borrows an Engine-owned byte archive without copying its cursor or payload.
    ///
    /// Only ordinary loading/saving modes are accepted. The cursor is invoked
    /// synchronously for each byte operation; it must not reenter this archive.
    pub fn borrowed(
        input: &'a dyn BorrowedArchiveCursor,
        mode: ArchiveMode,
        flags: ArchiveFlags,
        mapping: &'a dyn ArchiveMapping,
    ) -> RuntimeResult<Self> {
        let cursor_mode = match mode {
            ArchiveMode::Loading => CursorMode::Loading,
            ArchiveMode::Saving => CursorMode::Saving,
            _ => return Err(mode_error("borrowed archive requires loading or saving mode")),
        };
        Ok(Self {
            mode,
            flags,
            cursor: RefCell::new(Cursor::Borrowed(input, cursor_mode)),
            mapping,
            counted_bytes: Cell::new((0, 0)),
            actor_slot_compact_index_trace: Cell::new(ptr::null_mut()),
        })
    }

    fn network_reader(&self) -> Option<&'a dyn NetworkReader> {
        match &*self.cursor.borrow() {
            Cursor::Network(input) => Some(*input),
            _ => None,
        }
    }

    fn network_writer(&self) -> Option<&'a dyn NetworkWriter> {
        match &*self.cursor.borrow() {
            Cursor::NetworkWrite(input) => Some(*input),
            _ => None,
        }
    }

    /// Serializes the network object's actual package-map reference and class check.
    ///
    /// # Safety
    /// The object slot and expected class remain live across network mapping.
    pub unsafe fn serialize_net_object(
        &self,
        slot: *mut *mut UObject,
        expected_class: *mut UClass,
    ) -> RuntimeResult<i32> {
        if let Some(output) = self.network_writer() {
            // SAFETY: the backend receives only the caller's live mapping operands.
            return unsafe { output.write_object(slot, expected_class) };
        }
        let input = self.network_reader()
            .ok_or_else(|| mode_error("object net serialization requires a network archive"))?;
        // SAFETY: the backend receives only the caller's live mapping operands.
        unsafe { input.read_object(slot, expected_class) }
    }

    /// Creates an origin-zero writer over a fresh in-memory archive.
    pub fn saving(flags: ArchiveFlags, mapping: &'a dyn ArchiveMapping) -> Self {
        Self::saving_at(flags, mapping, 0)
    }

    /// Buffers one export while preserving its real package-file cursor.
    /// Native serializers can emit absolute file offsets without a relocation
    /// pass or another serialization of the live object.
    pub(crate) fn saving_at(flags: ArchiveFlags, mapping: &'a dyn ArchiveMapping, origin: usize) -> Self {
        Self {
            mode: ArchiveMode::Saving,
            flags,
            cursor: RefCell::new(Cursor::Write { output: PackageArchiveWriter::new(), origin }),
            mapping,
            counted_bytes: Cell::new((0, 0)),
            actor_slot_compact_index_trace: Cell::new(ptr::null_mut()),
        }
    }

    /// Creates the saving property-tag size counter without retaining payload.
    /// Like FArchiveCountSize, only saving/persistence are inherited; versions
    /// retain the ordinary FArchive constructor defaults.
    pub(crate) fn counting(&self) -> Self {
        let defaults = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
        Self {
            mode: ArchiveMode::Saving,
            flags: ArchiveFlags::new(defaults.version as u16, defaults.licensee as u16, self.is_persistent()),
            cursor: RefCell::new(Cursor::Count { size: 0, map_indices: self.uses_index_mapping() }),
            mapping: self.mapping,
            counted_bytes: Cell::new((0, 0)),
            actor_slot_compact_index_trace: Cell::new(ptr::null_mut()),
        }
    }

    /// Creates FArchiveCountMem: default versions, no persistence or byte output.
    /// This is distinct from the saving property-tag wire-size counter.
    pub fn memory_count(mapping: &'a dyn ArchiveMapping) -> Self {
        let defaults = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
        Self {
            mode: ArchiveMode::MemoryCount,
            flags: ArchiveFlags::new(defaults.version as u16, defaults.licensee as u16, false),
            cursor: RefCell::new(Cursor::None),
            mapping,
            counted_bytes: Cell::new((0, 0)),
            actor_slot_compact_index_trace: Cell::new(ptr::null_mut()),
        }
    }

    fn uses_index_mapping(&self) -> bool {
        match &*self.cursor.borrow() {
            Cursor::Count { map_indices, .. } => *map_indices,
            Cursor::Borrowed(_, _) => false,
            // Tag archives inherit FArchive's zero MapObject/MapName results.
            // Only the final saving archive has package-local indices.
            _ => self.mode == ArchiveMode::Saving,
        }
    }

    /// Creates a persistent saving archive that traverses object references as exports.
    pub fn tag_exports(flags: ArchiveFlags, mapping: &'a dyn ArchiveMapping) -> Self {
        Self {
            mode: ArchiveMode::TagExports,
            flags: ArchiveFlags { persistent: true, ..flags },
            cursor: RefCell::new(Cursor::None),
            mapping,
            counted_bytes: Cell::new((0, 0)),
            actor_slot_compact_index_trace: Cell::new(ptr::null_mut()),
        }
    }
    /// Creates a persistent saving archive that tags object and name imports.
    pub fn tag_imports(flags: ArchiveFlags, mapping: &'a dyn ArchiveMapping) -> Self {
        Self {
            mode: ArchiveMode::TagImports,
            flags: ArchiveFlags { persistent: true, ..flags },
            cursor: RefCell::new(Cursor::None),
            mapping,
            counted_bytes: Cell::new((0, 0)),
            actor_slot_compact_index_trace: Cell::new(ptr::null_mut()),
        }
    }
    /// Creates a reference visitor. Object operations visit identity and perform no byte I/O.
    pub fn reference(flags: ArchiveFlags, mapping: &'a dyn ArchiveMapping) -> Self {
        Self {
            mode: ArchiveMode::Reference,
            flags,
            cursor: RefCell::new(Cursor::None),
            mapping,
            counted_bytes: Cell::new((0, 0)),
            actor_slot_compact_index_trace: Cell::new(ptr::null_mut()),
        }
    }

    /// Creates an FBufferReader-equivalent transaction archive over memory bytes.
    pub fn transaction_loading(input: ExportArchive<'a>, mut flags: ArchiveFlags, mapping: &'a dyn ArchiveMapping) -> Self {
        flags.transaction_loading = true;
        Self {
            mode: ArchiveMode::Transaction,
            flags,
            cursor: RefCell::new(Cursor::Read(input)),
            mapping,
            counted_bytes: Cell::new((0, 0)),
            actor_slot_compact_index_trace: Cell::new(ptr::null_mut()),
        }
    }
    /// Creates a transaction writer over owned memory.
    pub fn transaction_saving(mut flags: ArchiveFlags, mapping: &'a dyn ArchiveMapping) -> Self {
        Self {
            mode: ArchiveMode::Transaction,
            flags,
            cursor: RefCell::new(Cursor::Write { output: PackageArchiveWriter::new(), origin: 0 }),
            mapping,
            counted_bytes: Cell::new((0, 0)),
            actor_slot_compact_index_trace: Cell::new(ptr::null_mut()),
        }
    }

    /// Borrows transaction snapshot storage without allocating or copying its payload.
    /// The cursor supplies raw FName bytes; UObject references retain native pointer width.
    pub fn transaction_borrowed(
        input: &'a dyn BorrowedArchiveCursor,
        flags: ArchiveFlags,
        mapping: &'a dyn ArchiveMapping,
    ) -> Self {
        let direction = if flags.transaction_loading { CursorMode::Loading } else { CursorMode::Saving };
        Self {
            mode: ArchiveMode::Transaction,
            flags,
            cursor: RefCell::new(Cursor::Borrowed(input, direction)),
            mapping,
            counted_bytes: Cell::new((0, 0)),
            actor_slot_compact_index_trace: Cell::new(ptr::null_mut()),
        }
    }

    /// Runs the transaction reader's owner preload without holding the cursor borrow.
    ///
    /// # Safety
    /// The object and any active snapshot owner remain live during reentry.
    pub unsafe fn preload_transaction_object(&self, object: *mut UObject) -> RuntimeResult<()> {
        let input = {
            let cursor = self.cursor.borrow();
            match &*cursor {
                Cursor::Borrowed(input, _) => Some(*input),
                _ => None,
            }
        };
        if let Some(input) = input {
            unsafe { input.preload_object(object)?; }
        }
        Ok(())
    }

    pub fn mode(&self) -> ArchiveMode { self.mode }
    pub fn flags(&self) -> ArchiveFlags { self.flags }
    pub fn is_loading(&self) -> bool { self.mode == ArchiveMode::Loading || (self.mode == ArchiveMode::Transaction && self.flags.transaction_loading) }
    pub fn is_saving(&self) -> bool { matches!(self.mode, ArchiveMode::Saving | ArchiveMode::TagExports | ArchiveMode::TagImports) || (self.mode == ArchiveMode::Transaction && !self.flags.transaction_loading) }
    /// Transaction readers and writers both carry FArchive's transaction flag.
    pub fn is_trans(&self) -> bool { self.mode == ArchiveMode::Transaction }
    pub fn is_persistent(&self) -> bool { self.flags.persistent }
    /// Returns a borrowed archive's sticky error flag; in-memory cursors are error-free here.
    pub fn is_error(&self) -> RuntimeResult<bool> {
        match &*self.cursor.borrow() {
            Cursor::Borrowed(input, _) => input.is_error(),
            _ => Ok(false),
        }
    }

    /// Adds native FArchive memory-accounting values without changing wire bytes.
    pub fn count_bytes(&self, count: usize, capacity: usize) -> RuntimeResult<()> {
        let (old_count, old_capacity) = self.counted_bytes.get();
        let new_count = old_count.checked_add(count)
            .ok_or_else(|| mode_error("archive counted-byte overflow"))?;
        let new_capacity = old_capacity.checked_add(capacity)
            .ok_or_else(|| mode_error("archive capacity-byte overflow"))?;
        self.counted_bytes.set((new_count, new_capacity));
        Ok(())
    }

    /// Returns the accumulated `(Num, Max)` values supplied to `CountBytes`.
    pub fn counted_bytes(&self) -> (usize, usize) {
        self.counted_bytes.get()
    }

    /// Whether this package export reader can report the exact compact object
    /// index and file cursor consumed by one actor reference.
    pub fn supports_actor_slot_compact_index_trace(&self) -> bool {
        self.is_loading() && matches!(&*self.cursor.borrow(), Cursor::Read(_))
    }

    /// Installs one stack trace record only for the duration of a single real
    /// object-slot serialization. The `Cell` is not borrowed while the slot
    /// maps the resolved object back into Core.
    ///
    /// # Safety
    /// `trace` remains writable and live until `operation` returns.
    pub unsafe fn with_actor_slot_compact_index_trace<T>(
        &self,
        trace: *mut ActorSlotCompactIndexTrace,
        operation: impl FnOnce() -> RuntimeResult<T>,
    ) -> RuntimeResult<T> {
        if trace.is_null() {
            return Err(mode_error("actor-slot compact trace record is null"));
        }
        if !self.supports_actor_slot_compact_index_trace() {
            return Err(mode_error("archive does not support actor-slot compact-index tracing"));
        }
        let previous = self.actor_slot_compact_index_trace.replace(trace);
        let result = operation();
        self.actor_slot_compact_index_trace.set(previous);
        result
    }

    /// Returns the cursor relative to this buffered payload.
    pub fn tell(&self) -> RuntimeResult<usize> {
        match &*self.cursor.borrow() {
            Cursor::Read(input) => Ok(input.tell()),
            Cursor::Write { output, .. } => Ok(output.tell()),
            Cursor::Count { size, .. } => Ok(*size),
            Cursor::Borrowed(input, _) => input.tell(),
            Cursor::None => Err(mode_error("reference archive has no byte cursor")),
            Cursor::Network(_) => Err(mode_error("network archive has a bit cursor, not a byte cursor")),
            Cursor::NetworkWrite(_) => Err(mode_error("network archive has a bit cursor, not a byte cursor")),
        }
    }
    pub(crate) fn shared_source_range(
        &self,
    ) -> Option<(std::rc::Rc<Vec<u8>>, std::ops::Range<usize>)> {
        match &*self.cursor.borrow() {
            Cursor::Read(input) => input.shared_source_range(),
            _ => None,
        }
    }

    pub(crate) fn linker_identity(&self) -> Option<*mut UObject> {
        self.mapping.linker_identity()
    }

    pub fn seek(&self, position: usize) -> RuntimeResult<()> {
        match &mut *self.cursor.borrow_mut() {
            Cursor::Read(input) => input.seek(position),
            Cursor::Write { output, .. } => output.seek(position),
            Cursor::Count { size, .. } => { *size = position; Ok(()) }
            Cursor::Borrowed(input, _) => input.seek(position),
            Cursor::None => Err(mode_error("reference archive has no byte cursor")),
            Cursor::Network(_) => Err(mode_error("network bit readers do not support byte seeking")),
            Cursor::NetworkWrite(_) => Err(mode_error("network bit writers do not support byte seeking")),
        }
    }

    /// Returns the actual FArchive file coordinate for native wire offsets.
    /// Ordinary memory and transaction archives retain their origin-zero cursor.
    pub fn tell_file(&self) -> RuntimeResult<usize> {
        let origin = self.file_origin();
        self.tell()?.checked_add(origin).ok_or_else(|| mode_error("archive position overflow"))
    }

    pub(crate) fn seek_file(&self, position: usize) -> RuntimeResult<()> {
        let relative = position.checked_sub(self.file_origin())
            .ok_or_else(|| mode_error("archive seek precedes file origin"))?;
        self.seek(relative)
    }

    fn file_origin(&self) -> usize {
        let read_origin = match &*self.cursor.borrow() {
            Cursor::Write { origin, .. } => return *origin,
            Cursor::Read(input) => input.shared_source_origin(),
            _ => None,
        };
        // Do not retain a cursor borrow across the mapping callback.
        if !self.is_trans() && self.linker_identity().is_some() {
            read_origin.unwrap_or(0)
        } else {
            0
        }
    }

    /// Takes bytes written into this owner without copying the completed payload.
    pub(crate) fn take_bytes(&self) -> RuntimeResult<Vec<u8>> {
        match &mut *self.cursor.borrow_mut() {
            Cursor::Write { output, .. } => Ok(std::mem::take(output).into_bytes()),
            _ => Err(mode_error("archive is not an owned writer")),
        }
    }

    pub fn read(&self, output: &mut [u8]) -> RuntimeResult<()> {
        if !self.is_loading() { return Err(mode_error("archive is not loading")); }
        if let Some(input) = self.network_reader() {
            // SAFETY: a mutable slice supplies its complete writable byte span.
            return unsafe { input.read_raw(output.as_mut_ptr(), output.len()) };
        }
        match &mut *self.cursor.borrow_mut() {
            Cursor::Read(input) => input.read_into(output),
            Cursor::Borrowed(input, CursorMode::Loading) => input.read(output),
            _ => Err(mode_error("archive has no reader")),
        }
    }

    /// Reads into writable storage without requiring its bytes to be initialized.
    ///
    /// # Safety
    /// `output` spans `count` writable bytes, does not overlap the archive input,
    /// and remains valid for this call. A zero count permits a null pointer.
    pub unsafe fn read_raw(&self, output: *mut u8, count: usize) -> RuntimeResult<()> {
        if !self.is_loading() { return Err(mode_error("archive is not loading")); }
        if let Some(input) = self.network_reader() {
            // SAFETY: the caller supplies the writable, nonoverlapping span.
            // network_reader releases the cursor borrow before backend reentry.
            return unsafe { input.read_raw(output, count) };
        }
        match &mut *self.cursor.borrow_mut() {
            Cursor::Read(input) => {
                let bytes = input.read(count)?;
                if count != 0 {
                    // SAFETY: the caller supplies the writable, nonoverlapping span.
                    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), output, count); }
                }
                Ok(())
            }
            Cursor::Borrowed(input, CursorMode::Loading) => {
                if count == 0 {
                    input.read(&mut [])
                } else if output.is_null() {
                    Err(mode_error("null borrowed archive output"))
                } else {
                    // SAFETY: the caller guarantees `count` writable bytes.
                    input.read(unsafe { std::slice::from_raw_parts_mut(output, count) })
                }
            }
            _ => Err(mode_error("archive has no reader")),
        }
    }

    pub fn write(&self, bytes: &[u8]) -> RuntimeResult<()> {
        if !self.is_saving() { return Err(mode_error("archive is not saving")); }
        match &mut *self.cursor.borrow_mut() {
            Cursor::Write { output, .. } => output.write(bytes),
            Cursor::Count { size, .. } => {
                *size = size.checked_add(bytes.len()).ok_or_else(|| mode_error("archive size overflow"))?;
                Ok(())
            }
            Cursor::NetworkWrite(output) => {
                // SAFETY: the slice supplies its complete readable span.
                unsafe { output.write_raw(bytes.as_ptr(), bytes.len()) }
            }
            Cursor::Borrowed(input, CursorMode::Saving) => input.write(bytes),
            Cursor::None if matches!(self.mode, ArchiveMode::TagExports | ArchiveMode::TagImports) => Ok(()),
            _ => Err(mode_error("archive has no writer")),
        }
    }
    pub fn serialize_i32(&self, value: &mut i32) -> RuntimeResult<()> {
        if self.is_loading() { let mut bytes=value.to_le_bytes(); self.read(&mut bytes)?; *value=i32::from_le_bytes(bytes); }
        else if self.is_saving() { self.write(&value.to_le_bytes())?; }
        Ok(())
    }

    pub fn serialize_u32(&self, value: &mut u32) -> RuntimeResult<()> {
        if self.is_loading() { let mut bytes=value.to_le_bytes(); self.read(&mut bytes)?; *value=u32::from_le_bytes(bytes); }
        else if self.is_saving() { self.write(&value.to_le_bytes())?; }
        Ok(())
    }

    /// Dispatches to bit-reader bounded packing or the base byte-archive DWORD.
    pub fn serialize_bounded(&self, value: &mut u32, maximum: u32) -> RuntimeResult<()> {
        if let Some(input) = self.network_reader() {
            return input.read_bounded(value, maximum);
        }
        if let Some(output) = self.network_writer() {
            return output.write_bounded(*value, maximum);
        }
        self.serialize_u32(value)
    }

    /// Base FArchive bit encoding with the historical post-load byte mask.
    /// Aligned and zero-bit loads select the following byte; callers must
    /// include that byte in `bytes` to preserve this native archive behavior.
    pub fn serialize_bits(&self, bytes: &mut [u8], bit_count: u32) -> RuntimeResult<()> {
        use crate::base_archive::{Hp2BaseArchiveLeaf as Leaf, serialize_bits_with};
        if let Some(input) = self.network_reader() {
            let bytes_needed = (bit_count as usize).div_ceil(8);
            if bytes_needed > bytes.len() {
                return Err(mode_error("network bit buffer is too small"));
            }
            // SAFETY: the checked slice covers the complete writable bit span.
            return unsafe { input.read_bits(bytes.as_mut_ptr(), bit_count) };
        }
        if let Some(output) = self.network_writer() {
            let bytes_needed = (bit_count as usize).div_ceil(8);
            if bytes_needed > bytes.len() {
                return Err(mode_error("network bit buffer is too small"));
            }
            // SAFETY: the checked slice covers the complete readable bit span.
            return unsafe { output.write_bits(bytes.as_ptr(), bit_count) };
        }
        if bit_count > (i32::MAX - 7) as u32 {
            return Err(mode_error("archive bit count overflow"));
        }
        serialize_bits_with(bytes.as_mut_ptr().cast(), bit_count as i32, |leaf, io| {
            match leaf {
                Leaf::Serialize => {
                    let length = io.length as usize;
                    let operand = bytes.get_mut(..length)
                        .ok_or_else(|| mode_error("archive bit buffer is too small"))?;
                    if self.is_loading() {
                        self.read(operand)?;
                    } else if self.is_saving() {
                        self.write(operand)?;
                    }
                }
                Leaf::Loading => io.value = i32::from(self.is_loading()),
                Leaf::MaskByte => {
                    let byte = bytes.get_mut(io.length as usize)
                        .ok_or_else(|| mode_error("archive bit mask is outside the buffer"))?;
                    *byte &= io.value as u8;
                }
                _ => unreachable!("base bit policy requested an unrelated archive operation"),
            }
            Ok(())
        })
    }

    pub fn serialize_u16(&self, value: &mut u16) -> RuntimeResult<()> {
        if self.is_loading() { let mut bytes=value.to_le_bytes(); self.read(&mut bytes)?; *value=u16::from_le_bytes(bytes); }
        else if self.is_saving() { self.write(&value.to_le_bytes())?; }
        Ok(())
    }
    pub fn serialize_f32(&self, value: &mut f32) -> RuntimeResult<()> {
        if self.is_loading() { let mut bytes=value.to_bits().to_le_bytes(); self.read(&mut bytes)?; *value=f32::from_bits(u32::from_le_bytes(bytes)); }
        else if self.is_saving() { self.write(&value.to_bits().to_le_bytes())?; }
        Ok(())
    }


    /// Returns the buffered payload span, excluding any package-file origin.
    pub fn total_size(&self) -> RuntimeResult<usize> {
        match &*self.cursor.borrow() {
            Cursor::Read(input) => Ok(input.len()),
            Cursor::Network(_) => Err(mode_error("network archive has a bit size, not a byte size")),
            Cursor::NetworkWrite(_) => Err(mode_error("network archive has a bit size, not a byte size")),
            Cursor::Write { output, .. } => Ok(output.bytes().len()),
            Cursor::Count { size, .. } => Ok(*size),
            Cursor::Borrowed(input, _) => input.total_size(),
            Cursor::None => Err(mode_error("reference archive has no byte size")),
        }
    }


    pub fn serialize_u8(&self, value: &mut u8) -> RuntimeResult<()> {
        if self.is_loading() { let mut byte=[*value]; self.read(&mut byte)?; *value=byte[0]; }
        else if self.is_saving() { self.write(&[*value])?; }
        Ok(())
    }

    pub fn serialize_compact_index(&self, value: &mut i32) -> RuntimeResult<()> {
        match self.mode {
            ArchiveMode::Loading | ArchiveMode::Transaction if self.is_loading() => {
                *value = crate::compact_index::decode(|| self.read_byte())?;
            }
            ArchiveMode::Saving | ArchiveMode::Transaction if self.is_saving() => {
                crate::compact_index::encode(*value, |byte| { self.write(&[byte])?; Ok(byte) })?;
            }
            _ => {}
        }
        Ok(())
    }

    pub fn serialize_fstring(&self, value: &mut Vec<u32>) -> RuntimeResult<()> {
        if self.is_loading() {
            let length = self.read_compact()?;
            let count = length.checked_abs().ok_or_else(|| mode_error("invalid FString length"))? as usize;
            let width = if length < 0 { 2 } else { 1 };
            let payload = count.checked_mul(width).ok_or_else(|| mode_error("FString length overflow"))?;
            // A native borrowed cursor owns its range checks and GNull error
            // flag; don't replace that contract with a package-parser exception.
            let bounded = !matches!(&*self.cursor.borrow(), Cursor::Borrowed(_, _));
            if bounded && payload > self.remaining()? { return Err(mode_error("FString outside archive range")); }
            *value = crate::fstring_archive::decode_utf32(length, |unit| match unit {
                crate::fstring_archive::FStringWireUnit::Ansi => Ok(u16::from(self.read_byte()?)),
                crate::fstring_archive::FStringWireUnit::Utf16 => {
                    let mut pair = [0; 2];
                    self.read(&mut pair)?;
                    Ok(u16::from_le_bytes(pair))
                }
            })?;
        } else if self.is_saving() {
            self.write_fstring(value)?;
        }
        Ok(())
    }

    /// Encodes borrowed UTF-32 text into an active saving archive.
    pub(crate) fn write_fstring(&self, value: &[u32]) -> RuntimeResult<()> {
        let length = crate::fstring_archive::utf32_wire_length(value)
            .ok_or_else(|| mode_error("FString length overflow"))?;
        let mut wire_length = length;
        self.serialize_compact_index(&mut wire_length)?;
        crate::fstring_archive::encode_utf32(value, length > 0, |unit, scalar| match unit {
            crate::fstring_archive::FStringWireUnit::Ansi => self.write(&[scalar as u8]),
            crate::fstring_archive::FStringWireUnit::Utf16 => self.write(&scalar.to_le_bytes()),
        })
    }

    /// Serializes a raw UObject slot without retaining a Rust borrow across mapping callbacks.
    ///
    /// # Safety
    /// `object_slot` must point to a live, writable UObject pointer slot for the duration of the call.
    pub unsafe fn serialize_object_slot(&self, object_slot: *mut *mut UObject) -> RuntimeResult<()> {
        if object_slot.is_null() { return Err(mode_error("null object reference slot")); }
        if let Some(input) = self.network_reader() {
            // SAFETY: the caller's writable slot remains live through mapping.
            return unsafe { input.read_object(object_slot, std::ptr::null_mut()).map(|_| ()) };
        }
        if let Some(output) = self.network_writer() {
            // SAFETY: the caller's writable slot remains live through mapping.
            return unsafe { output.write_object(object_slot, std::ptr::null_mut()).map(|_| ()) };
        }
        if self.mode != ArchiveMode::Transaction && matches!(&*self.cursor.borrow(), Cursor::Borrowed(_, _)) {
            // The source FArchive base class leaves UObject serialization untouched.
            return Ok(());
        }
        match self.mode {
            ArchiveMode::MemoryCount => Ok(()),
            ArchiveMode::Reference | ArchiveMode::TagExports | ArchiveMode::TagImports =>
                self.mapping.visit_object(object_slot),
            ArchiveMode::Transaction if self.is_loading() => {
                let mut bytes = [0; std::mem::size_of::<usize>()];
                self.read(&mut bytes)?;
                std::ptr::write_unaligned(object_slot, usize::from_le_bytes(bytes) as *mut UObject);
                Ok(())
            }
            ArchiveMode::Transaction if self.is_saving() => {
                let object = std::ptr::read_unaligned(object_slot);
                self.write(&(object as usize).to_le_bytes())
            }
            ArchiveMode::Transaction => Ok(()),
            ArchiveMode::Loading => {
                let index = self.read_compact()?;
                let trace = self.actor_slot_compact_index_trace.get();
                if !trace.is_null() {
                    let offset_after = i32::try_from(self.tell_file()?)
                        .map_err(|_| mode_error("actor-slot archive offset exceeds the INT range"))?;
                    unsafe {
                        std::ptr::write_unaligned(std::ptr::addr_of_mut!((*trace).index), index);
                        std::ptr::write_unaligned(std::ptr::addr_of_mut!((*trace).offset_after), offset_after);
                        std::ptr::write_unaligned(std::ptr::addr_of_mut!((*trace).captured), 1);
                    }
                }
                let object = self.mapping.load_object(index)?;
                std::ptr::write_unaligned(object_slot, object);
                Ok(())
            }
            ArchiveMode::Saving => {
                let object = std::ptr::read_unaligned(object_slot);
                let index = if self.uses_index_mapping() { self.mapping.save_object(object)? } else { 0 };
                self.write_compact(index)
            }
        }
    }

    pub fn load_object(&self) -> RuntimeResult<*mut UObject> {
        if !self.is_loading() { return Err(mode_error("archive is not an object-index reader")); }
        let mut object = std::ptr::null_mut();
        unsafe { self.serialize_object_slot(&mut object)?; }
        Ok(object)
    }

    pub fn save_object(&self, object: *mut UObject) -> RuntimeResult<()> {
        if !self.is_saving() && self.mode != ArchiveMode::Reference && self.mode != ArchiveMode::Transaction {
            return Err(mode_error("archive is not saving"));
        }
        let mut object = object;
        unsafe { self.serialize_object_slot(&mut object) }
    }

    /// Serializes a name-index slot through package-local mapping.
    ///
    /// # Safety
    /// `name_index` must point to a live, writable name-index slot for the duration of the call.
    pub unsafe fn serialize_name_slot(&self, name_index: *mut i32) -> RuntimeResult<()> {
        if name_index.is_null() { return Err(mode_error("null name index slot")); }
        if let Some(input) = self.network_reader() {
            // SAFETY: the caller's writable slot remains live through mapping.
            return unsafe { input.read_name(name_index) };
        }
        if let Some(output) = self.network_writer() {
            // SAFETY: the caller's writable slot remains live through mapping.
            return unsafe { output.write_name(name_index) };
        }
        let borrowed = {
            let cursor = self.cursor.borrow();
            match &*cursor {
                Cursor::Borrowed(input, _) => Some(*input),
                _ => None,
            }
        };
        if let Some(input) = borrowed {
            // SAFETY: the caller guarantees a live writable name slot.
            return unsafe { input.serialize_name(name_index) };
        }
        match self.mode {
            ArchiveMode::MemoryCount => Ok(()),
            ArchiveMode::Reference => self.mapping.visit_name(std::ptr::read_unaligned(name_index)),
            ArchiveMode::TagImports => self.mapping.visit_name(std::ptr::read_unaligned(name_index)),
            ArchiveMode::TagExports => Ok(()),
            ArchiveMode::Transaction if self.is_loading() => {
                let mut bytes = [0; 4];
                self.read(&mut bytes)?;
                std::ptr::write_unaligned(name_index, i32::from_le_bytes(bytes));
                Ok(())
            }
            ArchiveMode::Transaction if self.is_saving() => {
                let name = std::ptr::read_unaligned(name_index);
                self.write(&name.to_le_bytes())
            }
            ArchiveMode::Transaction => Ok(()),
            ArchiveMode::Loading => {
                let local_index = self.read_compact()?;
                let name = self.mapping.load_name(local_index)?;
                std::ptr::write_unaligned(name_index, name);
                Ok(())
            }
            ArchiveMode::Saving => {
                let name = std::ptr::read_unaligned(name_index);
                let index = if self.uses_index_mapping() { self.mapping.save_name(name)? } else { 0 };
                self.write_compact(index)
            }
        }
    }

    pub fn load_name(&self, global_index: &mut i32) -> RuntimeResult<()> {
        if !self.is_loading() { return Err(mode_error("archive is not loading")); }
        unsafe { self.serialize_name_slot(global_index) }
    }

    pub fn save_name(&self, global_index: i32) -> RuntimeResult<()> {
        if !self.is_saving() { return Err(mode_error("archive is not saving")); }
        let mut name = global_index;
        unsafe { self.serialize_name_slot(&mut name) }
    }

    pub fn into_written_bytes(self) -> RuntimeResult<Vec<u8>> {
        let is_saving = self.is_saving();
        match self.cursor.into_inner() {
            Cursor::Write { output, .. } if is_saving => Ok(output.into_bytes()),
            _ => Err(mode_error("archive has no written byte buffer")),
        }
    }

    fn read_i32(&self) -> RuntimeResult<i32> {
        let mut bytes = [0; 4]; self.read(&mut bytes)?; Ok(i32::from_le_bytes(bytes))
    }
    fn read_u32(&self) -> RuntimeResult<u32> {
        let mut bytes = [0; 4]; self.read(&mut bytes)?; Ok(u32::from_le_bytes(bytes))
    }
    fn read_byte(&self) -> RuntimeResult<u8> {
        let mut byte = [0]; self.read(&mut byte)?; Ok(byte[0])
    }
    fn read_compact(&self) -> RuntimeResult<i32> { crate::compact_index::decode(|| self.read_byte()) }
    fn write_compact(&self, value: i32) -> RuntimeResult<()> {
        crate::compact_index::encode(value, |byte| {
            self.write(&[byte])?;
            Ok(byte)
        }).map(|_| ())
    }
    fn remaining(&self) -> RuntimeResult<usize> {
        if let Some(input) = self.network_reader() {
            return Ok(input.total_bits().saturating_sub(input.position_bits()) / 8);
        }
        if let Some(output) = self.network_writer() {
            return Ok(output.total_bits().saturating_sub(output.position_bits()) / 8);
        }
        match &*self.cursor.borrow() {
            Cursor::Read(input) => Ok(input.len().saturating_sub(input.tell())),
            Cursor::Borrowed(input, _) => Ok(input.total_size()?.saturating_sub(input.tell()?)),
            _ => Err(mode_error("archive has no reader")),
        }
    }
}

fn mode_error(message: &str) -> RuntimeError { RuntimeError::new(message) }
