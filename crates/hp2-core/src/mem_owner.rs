//! Canonical scratch arenas using the shared FMemStack policy and an owner-local chunk pool.

use crate::{array_storage, mem::{self, MemServices}};
use crate::ansi::Tchar;
use crate::core_access::{RuntimeError, RuntimeResult};
use hp2_abi::engine::generated::{FMemStack, FTaggedMemory};
use hp2_abi::render_span::FMemMark;
use std::{cell::{RefCell, UnsafeCell}, ffi::{CStr, c_void}, ptr};

const FILE: &CStr = c"Core/Rust/mem_owner.rs";

/// Owns a canonical stack and its reusable chunks without a native allocator publication.
///
/// The inline stack must remain at a stable address while marks are live. Core's boxed
/// owner context supplies that stability for Engine arenas. No mutable owner borrow is
/// retained while callers execute script or recursively allocate scratch storage.
pub struct MemStackOwner {
    stack: UnsafeCell<FMemStack>,
    unused: UnsafeCell<*mut FTaggedMemory>,
}

impl MemStackOwner {
    /// Initializes the native stack settings without allocating a memory chunk.
    pub fn new(default_chunk_size: i32, tag: &'static [u32]) -> Self {
        let owner = Self {
            // The probed record consists only of scalar settings and nullable raw pointers.
            stack: UnsafeCell::new(unsafe { std::mem::zeroed() }),
            unused: UnsafeCell::new(ptr::null_mut()),
        };
        unsafe { mem::init(&mut *owner.stack.get(), default_chunk_size, tag.as_ptr().cast()); }
        owner
    }

    /// Returns the stable stack identity passed to canonical geometry allocation leaves.
    pub fn as_ptr(&self) -> *mut FMemStack { self.stack.get() }

    /// Counts the live stack cursor and older chunks using `FMemStack::GetByteCount`.
    pub fn byte_count(&self) -> RuntimeResult<i32> {
        let services = SystemMemory::default();
        unsafe { mem::get_byte_count(&*self.stack.get(), &services) }
            .map_err(|frame| services.error("counting canonical scratch memory", frame))
    }

    /// Allocates scratch storage using native alignment and chunk-growth policy.
    ///
    /// # Safety
    /// This owner remains stable until all corresponding marks are popped. Calls are on
    /// the owner thread, and no live Rust reference aliases the writable allocated bytes.
    /// The allocation is invalidated by the enclosing mark's pop or arena destruction.
    pub unsafe fn push_bytes(&self, alloc_size: i32, alignment: i32) -> RuntimeResult<*mut u8> {
        let services = SystemMemory::default();
        unsafe { mem::push_bytes_with_pool(&mut *self.stack.get(), alloc_size, alignment,
            true, FILE, &services, self.unused.get()) }
            .map_err(|frame| services.error("allocating canonical scratch memory", frame))
    }

    /// Captures the actual native stack cursor and chunk, without allocating.
    ///
    /// # Safety
    /// The owner remains stable until this mark is popped, and marks are popped in LIFO order.
    pub unsafe fn mark(&self) -> FMemMark {
        let mut mark = std::mem::MaybeUninit::<FMemMark>::uninit();
        unsafe { mem::mark_init(mark.as_mut_ptr(), self.stack.get()); mark.assume_init() }
    }

    /// Restores a mark and returns newer chunks only to this arena's unused pool.
    ///
    /// # Safety
    /// The mark belongs to this arena, is live, and is the innermost outstanding mark.
    /// No reference or pointer into released storage may subsequently be dereferenced.
    pub unsafe fn pop(&self, mark: &FMemMark) {
        unsafe { mem::mark_pop_with_pool(mark, self.unused.get()); }
    }

    /// Checks that the arena has no active chunks, preserving native Tick's invariant.
    pub fn tick(&self) -> RuntimeResult<()> {
        let services = SystemMemory::default();
        unsafe { mem::tick(&*self.stack.get(), &services, FILE.as_ptr()) }
            .map_err(|frame| services.error("ticking canonical scratch memory", frame))
    }

    /// Checks native Exit's empty-stack invariant, then releases reusable chunks.
    pub fn exit(&self) -> RuntimeResult<()> {
        let services = SystemMemory::default();
        unsafe { mem::exit_with_pool(&*self.stack.get(), &services, FILE.as_ptr(), self.unused.get()) }
            .map_err(|frame| services.error("exiting canonical scratch memory", frame))
    }
}

impl Drop for MemStackOwner {
    fn drop(&mut self) {
        // Marks cannot outlive the owning runtime. Error teardown still releases active chunks.
        unsafe {
            mem::free_chunks_with_pool(&mut *self.stack.get(), ptr::null_mut(), self.unused.get());
            let services = array_storage::system_allocator();
            while !(*self.unused.get()).is_null() {
                let chunk = *self.unused.get();
                *self.unused.get() = (*chunk).next;
                (services.free)(services.context, chunk.cast());
            }
        }
    }
}

#[derive(Default)]
struct SystemMemory { error: RefCell<Option<RuntimeError>> }

impl SystemMemory {
    fn error(&self, operation: &str, frame: i32) -> RuntimeError {
        self.error.borrow_mut().take().unwrap_or_else(||
            RuntimeError::new("Canonical memory operation failed").with_context(&format!("{operation} ({frame})")))
    }
    fn record(&self, error: RuntimeError) -> i32 {
        let mut first = self.error.borrow_mut();
        if first.is_none() { *first = Some(error); }
        1
    }
}

impl MemServices for SystemMemory {
    unsafe fn alloc(&self, size: u32, tag: *const c_void) -> Result<*mut c_void, i32> {
        let services = array_storage::system_allocator();
        let mut output = ptr::null_mut();
        let capacity = 0;
        let status = unsafe { (services.realloc)(services.context, ptr::null_mut(), size as usize,
            &capacity, 1, tag, &mut output) };
        if status == 0 { Ok(output) } else {
            Err(self.record(RuntimeError::new("Unable to allocate canonical scratch chunk")))
        }
    }
    unsafe fn release(&self, pointer: *mut c_void) {
        let services = array_storage::system_allocator();
        unsafe { (services.free)(services.context, pointer); }
    }
    fn raise_error(&self, message: &[Tchar]) -> i32 {
        let length = message.iter().position(|&unit| unit == 0).unwrap_or(message.len());
        self.record(RuntimeError::from_utf32(message[..length].iter()
            .map(|unit| u32::from_ne_bytes(unit.to_ne_bytes())).collect()))
    }
    fn fail_assert(&self, expression: &CStr, file: &CStr, line: i32) -> i32 {
        self.record(RuntimeError::new(&format!("Assertion failed: {}", expression.to_string_lossy()))
            .with_context(&format!("{}:{line}", file.to_string_lossy())))
    }
}
