//! Owns the canonical Rust `FMemCache` record and its allocations.

use std::alloc::{Layout, alloc_zeroed, dealloc};
use std::cell::RefCell;
use std::ffi::{CStr, c_void};
use std::ptr::{self, NonNull};

use hp2_abi::engine::generated::{FCacheItem, FMemCache};

use crate::ansi::Tchar;
use crate::cache::{self, CacheServices, StatusFigures};
use crate::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use crate::mem::MemServices;

const OWNER_FILE: &CStr = c"Core/Rust/cache_owner.rs";

/// Owns a stable compiler-probed cache record and runs the existing cache policy.
pub struct CacheOwner { cache: NonNull<FMemCache> }

impl CacheOwner {
    /// Materializes the native zero-initialized global cache before Init.
    /// Flush is valid in this state, matching FMemCache::Flush's initialized gate.
    pub fn uninitialized() -> RuntimeResult<Self> {
        let layout = Layout::new::<FMemCache>();
        // The native record contains a 16K hash table; allocate it directly in place.
        let cache = NonNull::new(unsafe { alloc_zeroed(layout).cast::<FMemCache>() })
            .ok_or_else(|| RuntimeError::new("Unable to allocate FMemCache record"))?;
        Ok(Self { cache })
    }

    /// Initializes an existing zero-state cache owner with the canonical cache policy.
    pub fn initialize_in_place(&mut self, access: CoreAccess<'_>, bytes: i32, max_items: i32, segment_size: i32) -> RuntimeResult<()> {
        let services = CacheServicesBinding::new(access);
        unsafe { cache::init(self.cache.as_mut(), bytes, max_items, ptr::null_mut(), segment_size, OWNER_FILE.as_ptr(), &services) }
            .map_err(|frame| services.error("initialization", frame))
    }

    /// Allocates and initializes a cache record using Core's system heap.
    pub fn initialize(access: CoreAccess<'_>, bytes: i32, max_items: i32, segment_size: i32) -> RuntimeResult<Self> {
        let mut owner = Self::uninitialized()?;
        owner.initialize_in_place(access, bytes, max_items, segment_size)?;
        Ok(owner)
    }

    /// Returns the stable native cache record for EngineState integration.
    pub fn as_ptr(&mut self) -> *mut FMemCache { self.cache.as_ptr() }

    /// Creates or replaces an item using the canonical cache policy.
    ///
    /// # Safety
    /// `alignment` must be a positive power of two; `safety_pad` must preserve
    /// the native create-size arithmetic bounds.
    pub unsafe fn create(&mut self, access: CoreAccess<'_>, id: u64, size: i32, alignment: i32, safety_pad: i32) -> RuntimeResult<(*mut u8, *mut FCacheItem)> {
        let services = CacheServicesBinding::new(access);
        unsafe { cache::create(self.cache.as_mut(), id, size, alignment, safety_pad, OWNER_FILE.as_ptr(), &services) }
            .map_err(|frame| services.error("create", frame))
    }

    /// Finds and locks an item; a miss leaves the caller without an item.
    ///
    /// # Safety
    /// `id` must be nonzero and `alignment` a positive power of two.
    pub unsafe fn get(&mut self, access: CoreAccess<'_>, id: u64, alignment: i32) -> Option<(*mut u8, *mut FCacheItem)> {
        let services = CacheServicesBinding::new(access);
        let mut item = ptr::null_mut();
        let data = unsafe { cache::get(self.cache.as_ptr(), id, &mut item, alignment, true, true, &services) };
        (!data.is_null()).then_some((data, item))
    }

    /// Releases a previously acquired item lock.
    ///
    /// # Safety
    /// `item` must be null or an item from this cache which has not been
    /// invalidated by a flush, replacement, or cache teardown.
    pub unsafe fn unlock(&mut self, access: CoreAccess<'_>, item: *mut FCacheItem) -> RuntimeResult<()> {
        let services = CacheServicesBinding::new(access);
        let code = unsafe { cache::item_unlock(item, true) };
        if code == 0 { return Ok(()); }
        unsafe { cache::raise_unlock_error(item, code, &services) };
        Err(services.error("unlock", code))
    }
    /// Flushes entries selected by the canonical cache policy.
    pub fn flush(&mut self, access: CoreAccess<'_>, id: u64, mask: u32, ignore_locked: i32) -> RuntimeResult<()> {
        let services = CacheServicesBinding::new(access);
        unsafe { cache::flush(self.cache.as_mut(), id, mask, ignore_locked, OWNER_FILE.as_ptr(), &services) }
            .map_err(|frame| services.error("flush", frame))
    }

    /// Advances the cache age and statistics.
    pub fn tick(&mut self, access: CoreAccess<'_>) {
        let services = CacheServicesBinding::new(access);
        unsafe { cache::tick(self.cache.as_mut(), &services) }
    }

    /// Formats status into caller-provided TCHAR storage and resets counters.
    ///
    /// # Safety
    /// `out` must point to writable storage for at least 1024 TCHAR values.
    pub unsafe fn status(&mut self, access: CoreAccess<'_>, out: *mut Tchar) {
        let services = CacheServicesBinding::new(access);
        unsafe { cache::status(self.cache.as_mut(), out.cast(), &services) }
    }

    /// Executes `DUMPCACHE` against this live shared cache and a borrowed output sink.
    ///
    /// # Safety
    /// `cmd` must be a NUL-terminated TCHAR command for the duration of the call.
    pub unsafe fn exec_with_output(
        &mut self,
        access: CoreAccess<'_>,
        cmd: *const c_void,
        output: &mut dyn FnMut(&[u32]) -> RuntimeResult<()>,
    ) -> RuntimeResult<bool> {
        let services = CacheServicesBinding::with_output(access, output);
        let output_handle = (&services as *const CacheServicesBinding<'_, '_>)
            .cast_mut()
            .cast();
        unsafe { cache::exec(self.cache.as_ref(), cmd, output_handle, &services) }
            .map(|handled| handled != 0)
            .map_err(|frame| services.error("command", frame))
    }

    /// Executes the existing `DUMPCACHE` command policy using Core's log output.
    ///
    /// # Safety
    /// `cmd` must be a NUL-terminated TCHAR command. Output routes through Core.
    pub unsafe fn exec(&mut self, access: CoreAccess<'_>, cmd: *const c_void) -> RuntimeResult<bool> {
        let services = CacheServicesBinding::new(access);
        // The adapter treats this live service address as its synchronous output handle.
        let output_handle = (&services as *const CacheServicesBinding<'_, '_>)
            .cast_mut()
            .cast();
        unsafe { cache::exec(self.cache.as_ref(), cmd, output_handle, &services) }
            .map(|handled| handled != 0)
            .map_err(|frame| services.error("command", frame))
    }

    /// Consumes the cache, running exit policy before releasing its buffers.
    pub fn exit(mut self, access: CoreAccess<'_>) -> RuntimeResult<()> {
        self.exit_in_place(access)
    }

    /// Runs global-cache Exit while retaining its valid uninitialized record.
    pub fn exit_in_place(&mut self, access: CoreAccess<'_>) -> RuntimeResult<()> {
        let services = CacheServicesBinding::new(access);
        unsafe { cache::exit(self.cache.as_mut(), 1, OWNER_FILE.as_ptr(), &services) }
            .map_err(|frame| services.error("exit", frame))?;
        let cache = unsafe { self.cache.as_mut() };
        cache.item_memory = ptr::null_mut();
        cache.cache_memory = ptr::null_mut();
        Ok(())
    }

    fn release_allocations(&mut self) {
        // Init may fail after either allocation; Drop also handles error teardown.
        let cache = unsafe { self.cache.as_mut() };
        if !cache.item_memory.is_null() {
            unsafe { release_block(cache.item_memory.cast()) };
            cache.item_memory = ptr::null_mut();
        }
        if !cache.cache_memory.is_null() {
            unsafe { release_block(cache.cache_memory.cast()) };
            cache.cache_memory = ptr::null_mut();
        }
    }
}

impl Drop for CacheOwner {
    fn drop(&mut self) {
        self.release_allocations();
        unsafe { dealloc(self.cache.as_ptr().cast(), Layout::new::<FMemCache>()) };
    }
}

unsafe fn allocate_block(size: u32, tag: *const c_void) -> Result<*mut c_void, i32> {
    let allocator = crate::array_storage::system_allocator();
    let mut block = ptr::null_mut();
    let count = 0;
    let status = unsafe {
        (allocator.realloc)(allocator.context, ptr::null_mut(), size as usize, &count, 1, tag, &mut block)
    };
    if status == 0 { Ok(block) } else { Err(status) }
}

unsafe fn release_block(block: *mut c_void) {
    let allocator = crate::array_storage::system_allocator();
    let status = unsafe { (allocator.free)(allocator.context, block) };
    debug_assert_eq!(status, 0, "Core system allocator release is infallible");
}

struct CacheServicesBinding<'access, 'output> {
    access: CoreAccess<'access>,
    failure: RefCell<Option<RuntimeError>>,
    output: RefCell<Option<&'output mut dyn FnMut(&[u32]) -> RuntimeResult<()>>>,
}

impl<'access, 'output> CacheServicesBinding<'access, 'output> {
    fn new(access: CoreAccess<'access>) -> Self {
        Self { access, failure: RefCell::new(None), output: RefCell::new(None) }
    }

    fn with_output(
        access: CoreAccess<'access>,
        output: &'output mut dyn FnMut(&[u32]) -> RuntimeResult<()>,
    ) -> Self {
        Self { access, failure: RefCell::new(None), output: RefCell::new(Some(output)) }
    }

    fn record(&self, error: RuntimeError) -> i32 {
        let mut failure = self.failure.borrow_mut();
        if failure.is_none() { *failure = Some(error); }
        1
    }

    fn error(&self, operation: &str, frame: i32) -> RuntimeError {
        self.failure.borrow_mut().take().unwrap_or_else(|| {
            RuntimeError::new(&format!("FMemCache {operation} failed in guard frame {frame}"))
        })
    }
}

impl MemServices for CacheServicesBinding<'_, '_> {
    unsafe fn alloc(&self, size: u32, tag: *const c_void) -> Result<*mut c_void, i32> {
        unsafe { allocate_block(size, tag) }.map_err(|_| {
            self.record(RuntimeError::new(&format!("Unable to allocate {size} FMemCache bytes")))
        })
    }
    unsafe fn release(&self, block: *mut c_void) { unsafe { release_block(block) } }
    fn raise_error(&self, msg: &[Tchar]) -> i32 {
        let text = msg.iter().copied().take_while(|unit| *unit != 0).map(|unit| unit as u32).collect();
        self.record(RuntimeError::new("FMemCache error").with_utf32_context(text))
    }
    fn fail_assert(&self, expr: &CStr, file: &CStr, line: i32) -> i32 {
        self.record(RuntimeError::new(&format!(
            "FMemCache assertion failed: {} ({}:{line})", expr.to_string_lossy(), file.to_string_lossy(),
        )))
    }
}
impl CacheServices for CacheServicesBinding<'_, '_> {
    unsafe fn logf(&self, _ar: *mut c_void, text: &[Tchar]) -> Result<(), i32> {
        let message: Vec<u32> = text.iter().copied().take_while(|unit| *unit != 0).map(|unit| unit as u32).collect();
        if let Some(output) = self.output.borrow_mut().as_deref_mut() {
            output(&message).map_err(|error| self.record(error))
        } else {
            self.access.emit_log(0, &message).map_err(|error| self.record(error))
        }
    }
    fn glog(&self) -> *mut c_void { ptr::from_ref(self).cast_mut().cast() }
    unsafe fn parse_command(&self, stream: *mut *const c_void, matched: &[Tchar]) -> bool {
        unsafe { crate::command::parse_command(stream.cast(), matched.as_ptr()) != 0 }
    }
    fn app_cycles(&self) -> u32 { crate::unix_clock::cycles() }
    fn seconds_per_cycle(&self) -> f32 { crate::unix_clock::seconds_per_cycle() }
    unsafe fn status_format(&self, out: *mut c_void, figures: &StatusFigures) {
        unsafe { cache::format_status(out, figures) }
    }
}
