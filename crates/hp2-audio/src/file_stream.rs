//! Owns the audio stream manager, its worker, locks, queues and readers.
//! The manager identity is the existing `Arc<Runtime>` allocation. Native code
//! supplies only synchronous USound/TLazyArray extraction; Rust converts paths.
//! Records remain at fixed addresses until shutdown joins the worker and drains
//! Enter/Leave leases. Request and completion polling allocate nothing.
//!
//! In deterministic verification mode (`HP2_DETERMINISTIC_VERIFICATION=1`, see
//! [`crate::verification`]) a request performs its read synchronously under the
//! requesting stream's lock through the same [`complete_read`] lifecycle the
//! worker uses, so refill readiness never depends on OS scheduling. The worker
//! and its queues remain in place (and unchanged) for production runs.
use hp2_ffi_support::guard;
use std::cell::{RefCell, UnsafeCell};
use std::ffi::{c_char, c_void};
use std::panic::AssertUnwindSafe;
use std::ptr;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};

pub const HP2_STREAM_CAPACITY: usize = 4096;
pub const HP2_STREAM_REGULAR: i32 = 0;
pub const HP2_STREAM_OGG: i32 = 1;
pub const HP2_STREAM_OGG_LOOPING: i32 = 2;
pub const HP2_STREAM_XA: i32 = 3;
pub const HP2_STREAM_XA_LOOPING: i32 = 4;

#[derive(Clone, Copy)]
struct Hp2StreamCompletion {
    destination: *mut c_void,
    terminal: i32,
    ready: i32,
}

struct Hp2Stream {
    handle: *mut c_void,
    tdd: *mut c_void,
    chunk_size: i32,
    pending_destinations: [*mut c_void; HP2_STREAM_CAPACITY],
    pending_head: usize,
    pending_count: usize,
    completed_chunks: [Hp2StreamCompletion; HP2_STREAM_CAPACITY],
    completion_head: usize,
    completion_count: usize,
    used: i32,
    end_of_file: i32,
    num_samples: i32,
    kind: i32,
}

/// Called synchronously under the lifecycle and selected stream locks. For XA,
/// `bytes`/`length` describe loaded data. Otherwise `bytes` points to a terminated
/// native TCHAR path and `length` is ignored. Storage remains valid until return.
pub type Hp2StreamSource = unsafe extern "C" fn(
    context: *mut c_void,
    bytes: *mut *const u8,
    length: *mut usize,
    valid: *mut i32,
) -> i32;

unsafe extern "C" {
    fn fopen(path: *const c_char, mode: *const c_char) -> *mut c_void;
    fn fread(dest: *mut c_void, size: usize, count: usize, file: *mut c_void) -> usize;
    fn ferror(file: *mut c_void) -> i32;
    fn fclose(file: *mut c_void) -> i32;
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}

/// A nonrecursive lock whose acquisition can span two native ABI calls. Ordinary
/// operations use an RAII permit; Enter keeps the same permit in thread-local
/// storage, so Leave needs neither the lifecycle lock nor the global runtime.
#[derive(Default)]
struct StreamLock {
    held: Mutex<bool>,
    available: Condvar,
}
impl StreamLock {
    fn acquire(&self) {
        let mut held = lock(&self.held);
        while *held {
            held = self.available.wait(held).unwrap_or_else(|e| e.into_inner());
        }
        *held = true;
    }
    fn release(&self) {
        *lock(&self.held) = false;
        self.available.notify_one();
    }
    fn permit(&self) -> Permit<'_> {
        self.acquire();
        Permit(self)
    }
}
struct Permit<'a>(&'a StreamLock);
impl Drop for Permit<'_> {
    fn drop(&mut self) {
        self.0.release();
    }
}

#[derive(Default)]
struct Work {
    pending: usize,
    stop: bool,
}
struct Runtime {
    streams: Box<[UnsafeCell<Hp2Stream>]>,
    locks: Box<[StreamLock]>,
    work: Mutex<Work>,
    ready: Condvar,
}
// Every record/resource is protected by its stable per-stream lock. Publication
// is under LIFECYCLE; destinations and adopted decoders satisfy create's lifetime
// contract. The Arc allocation itself supplies the opaque manager identity.
unsafe impl Send for Runtime {}
unsafe impl Sync for Runtime {}

static LIFECYCLE: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);
static RUNTIME: Mutex<Option<Arc<Runtime>>> = Mutex::new(None);

struct Lease {
    runtime: Arc<Runtime>,
    id: usize,
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.runtime.locks[self.id].release();
    }
}
thread_local! {
    static LEASES: RefCell<Vec<Lease>> = const { RefCell::new(Vec::new()) };
}

fn runtime(instance: *mut c_void) -> Option<Arc<Runtime>> {
    lock(&RUNTIME)
        .as_ref()
        .filter(|r| Arc::as_ptr(r).cast_mut().cast::<c_void>() == instance)
        .cloned()
}
fn valid_id(runtime: &Runtime, id: i32) -> Option<usize> {
    (id >= 0 && (id as usize) < runtime.streams.len()).then_some(id as usize)
}

fn reset(stream: &mut Hp2Stream) {
    stream.handle = ptr::null_mut();
    stream.tdd = ptr::null_mut();
    stream.chunk_size = 0;
    stream.pending_head = 0;
    stream.pending_count = 0;
    stream.completion_head = 0;
    stream.completion_count = 0;
    stream.used = 0;
    stream.end_of_file = 0;
    stream.num_samples = 0;
    stream.kind = 0;
}

fn cancel_pending(runtime: &Runtime, stream: &mut Hp2Stream) {
    let cancelled = stream.pending_count;
    if cancelled == 0 {
        return;
    }
    let mut work = lock(&runtime.work);
    // Pending reservations are always the unready tail of the completion ring,
    // including when several requests name the same destination.
    for _ in 0..cancelled {
        let tail = (stream.completion_head + stream.completion_count - 1) % HP2_STREAM_CAPACITY;
        debug_assert_eq!(stream.completed_chunks[tail].ready, 0);
        stream.completion_count -= 1;
    }
    stream.pending_head = 0;
    stream.pending_count = 0;
    if stream.completion_count == 0 {
        stream.completion_head = 0;
    }
    work.pending = work.pending.saturating_sub(cancelled);
}

struct XaState {
    encoded: Box<[u8]>,
    decoder: hp2_codecs::eaxa::Decoder,
}
impl XaState {
    unsafe fn refeed(&mut self, samples: i32) -> bool {
        unsafe {
            self.decoder
                .feed(self.encoded.as_ptr(), self.encoded.len() as i32, samples)
        }
    }
}

unsafe fn destroy_stream(runtime: &Runtime, stream: &mut Hp2Stream) -> bool {
    cancel_pending(runtime, stream);
    stream.completion_head = 0;
    stream.completion_count = 0;
    unsafe {
        match stream.kind {
            0 => {
                if !stream.handle.is_null() {
                    fclose(stream.handle);
                }
            }
            1 | 2 => {
                if !stream.tdd.is_null() {
                    drop(Box::from_raw(stream.tdd.cast::<hp2_codecs::vorbis::Decoder>()));
                }
            }
            3 | 4 => {
                if !stream.tdd.is_null() {
                    drop(Box::from_raw(stream.tdd.cast::<XaState>()));
                }
            }
            _ if !stream.handle.is_null() || !stream.tdd.is_null() || stream.used != 0 => {
                return false;
            }
            _ => {}
        }
    }
    reset(stream);
    true
}

unsafe fn read_stream(stream: &mut Hp2Stream, bytes: i32, destination: *mut c_void) -> bool {
    if bytes < 0 || destination.is_null() {
        return false;
    }
    if bytes == 0 {
        return true;
    }
    unsafe {
        match stream.kind {
            0 if !stream.handle.is_null() => {
                let count = fread(destination, 1, bytes as usize, stream.handle);
                if count != bytes as usize {
                    stream.end_of_file = 1;
                }
                // Regular files deliberately leave the unread destination suffix
                // untouched and return success on a short read without ferror.
                ferror(stream.handle) == 0
            }
            1 | 2 if !stream.handle.is_null() && !stream.tdd.is_null() => {
                let mut count = 0;
                let mut produced_since_seek = false;
                while count < bytes as usize {
                    let output = destination.cast::<u8>().add(count);
                    let remaining = bytes as usize - count;
                    let read = (&mut *stream.tdd.cast::<hp2_codecs::vorbis::Decoder>())
                        .read(std::slice::from_raw_parts_mut(output, remaining));
                    if read > 0 {
                        count += read as usize;
                        produced_since_seek = true;
                        continue;
                    }
                    if read < 0
                        || stream.kind != 2
                        || !produced_since_seek
                        || !(&mut *stream.tdd.cast::<hp2_codecs::vorbis::Decoder>()).seek_to_start()
                    {
                        ptr::write_bytes(
                            destination.cast::<u8>().add(count),
                            0,
                            bytes as usize - count,
                        );
                        stream.end_of_file = 1;
                        return false;
                    }
                    produced_since_seek = false;
                }
                true
            }
            3 | 4 if !stream.tdd.is_null() => {
                if bytes & 1 != 0 {
                    return false;
                }
                let state = &mut *stream.tdd.cast::<XaState>();
                // Stream destinations are writable byte buffers, not necessarily i16-aligned.
                let dest = destination.cast::<u8>();
                let samples = bytes / 2;
                let mut count = 0;
                let mut refeed_without_progress = false;
                while count < samples {
                    let requested = samples - count;
                    let output = dest.add(count as usize * 2);
                    // SAFETY: the remaining destination bytes are writable; the XA
                    // decoder accepts unaligned sample destinations.
                    let read = state.decoder.decode(output.cast::<i16>(), requested);
                    if read < 0 || read > requested {
                        ptr::write_bytes(output, 0, requested as usize * 2);
                        stream.end_of_file = 1;
                        return false;
                    }
                    if read > 0 {
                        count += read;
                        refeed_without_progress = false;
                        continue;
                    }
                    if stream.kind != 4 {
                        ptr::write_bytes(output, 0, (samples - count) as usize * 2);
                        stream.end_of_file = 1;
                        return true;
                    }
                    if refeed_without_progress || !state.refeed(stream.num_samples) {
                        ptr::write_bytes(output, 0, (samples - count) as usize * 2);
                        stream.end_of_file = 1;
                        return false;
                    }
                    refeed_without_progress = true;
                }
                true
            }
            _ => false,
        }
    }
}

fn take_pending(runtime: &Runtime, stream: &mut Hp2Stream) -> *mut c_void {
    let destination = stream.pending_destinations[stream.pending_head];
    stream.pending_head = (stream.pending_head + 1) % HP2_STREAM_CAPACITY;
    stream.pending_count -= 1;
    if stream.pending_count == 0 {
        stream.pending_head = 0;
    }
    let mut work = lock(&runtime.work);
    work.pending = work.pending.saturating_sub(1);
    destination
}

/// Fills `destination` with the next chunk and publishes the completion
/// reserved at `index`. Shared by the asynchronous worker and the synchronous
/// deterministic verification path so both use the same decoder, queue and
/// cancellation lifecycle.
fn complete_read(
    runtime: &Runtime,
    stream: &mut Hp2Stream,
    index: usize,
    destination: *mut c_void,
) {
    let succeeded = guard(
        "file_stream_read",
        false,
        AssertUnwindSafe(|| unsafe { read_stream(stream, stream.chunk_size, destination) }),
    );
    if !succeeded {
        stream.end_of_file = 1;
    }
    let completion = &mut stream.completed_chunks[index];
    debug_assert_eq!(completion.ready, 0);
    debug_assert_eq!(completion.destination, destination);
    completion.terminal = (stream.end_of_file != 0) as i32;
    completion.ready = 1;
    if stream.end_of_file != 0 {
        cancel_pending(runtime, stream);
    }
}

fn worker(runtime: Arc<Runtime>) {
    loop {
        {
            let mut work = lock(&runtime.work);
            while !work.stop && work.pending == 0 {
                work = runtime.ready.wait(work).unwrap_or_else(|e| e.into_inner());
            }
            if work.stop {
                return;
            }
        }
        for id in 0..runtime.streams.len() {
            let _permit = runtime.locks[id].permit();
            let stream = unsafe { &mut *runtime.streams[id].get() };
            if stream.used == 0 || stream.pending_count == 0 {
                continue;
            }
            let destination = take_pending(&runtime, stream);
            // The pending requests are the unready completion tail. The read just
            // removed its first request, so locate its reservation directly.
            let index =
                (stream.completion_head + stream.completion_count - stream.pending_count - 1)
                    % HP2_STREAM_CAPACITY;
            complete_read(&runtime, stream, index, destination);
        }
    }
}

/// # Safety
/// `output` is writable. The returned identity is opaque and must not be freed.
pub unsafe extern "C" fn hp2_rs_stream_init(capacity: i32, output: *mut *mut c_void) -> i32 {
    guard("hp2_rs_stream_init", 1, || unsafe {
        *output = ptr::null_mut();
        let mut lifecycle = lock(&LIFECYCLE);
        if let Some(runtime) = lock(&RUNTIME).as_ref() {
            *output = Arc::as_ptr(runtime).cast_mut().cast();
            return 0;
        }
        if capacity <= 0 {
            return 0;
        }
        // Zero is a valid representation for every field, including pointers.
        // Allocate directly on the heap: a 4096-entry record must not be copied
        // through a worker-sized stack frame for each stream.
        let mut records = Box::<[UnsafeCell<Hp2Stream>]>::new_uninit_slice(capacity as usize);
        ptr::write_bytes(records.as_mut_ptr(), 0, capacity as usize);
        let records = records.assume_init();
        let locks = (0..capacity)
            .map(|_| StreamLock::default())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let runtime = Arc::new(Runtime {
            streams: records,
            locks,
            work: Mutex::new(Work::default()),
            ready: Condvar::new(),
        });
        let worker_runtime = Arc::clone(&runtime);
        let started = thread::Builder::new()
            .name("hp2-file-stream".into())
            .spawn(move || {
                guard(
                    "file_stream_worker",
                    (),
                    AssertUnwindSafe(|| worker(worker_runtime)),
                );
            });
        let Ok(worker) = started else {
            return 0;
        };
        *output = Arc::as_ptr(&runtime).cast_mut().cast();
        *lock(&RUNTIME) = Some(runtime);
        *lifecycle = Some(worker);
        0
    })
}

/// Returns the current opaque manager identity, or null before init/after shutdown.
pub extern "C" fn hp2_rs_stream_manager() -> *mut c_void {
    guard("hp2_rs_stream_manager", ptr::null_mut(), || {
        lock(&RUNTIME)
            .as_ref()
            .map_or(ptr::null_mut(), |runtime| Arc::as_ptr(runtime).cast_mut().cast())
    })
}

/// Joins the worker and drains outstanding Enter/Leave leases before releasing
/// resources. Queued destinations may be written until this call returns.
/// # Safety
/// Do not call while this thread holds an Enter lease.
pub unsafe extern "C" fn hp2_rs_stream_shutdown() -> i32 {
    guard("hp2_rs_stream_shutdown", 1, || unsafe {
        let mut lifecycle = lock(&LIFECYCLE);
        let Some(runtime) = lock(&RUNTIME).clone() else {
            return 0;
        };
        lock(&runtime.work).stop = true;
        runtime.ready.notify_all();
        if let Some(worker) = lifecycle.take() {
            let _ = worker.join();
        }
        for id in 0..runtime.streams.len() {
            let _permit = runtime.locks[id].permit();
            let stream = &mut *runtime.streams[id].get();
            if stream.used != 0 || !stream.handle.is_null() || !stream.tdd.is_null() {
                destroy_stream(&runtime, stream);
            }
        }
        *lock(&RUNTIME) = None;
        0
    })
}

unsafe fn create_resource(
    stream: &mut Hp2Stream,
    source: Hp2StreamSource,
    context: *mut c_void,
    xa: bool,
    samples: i32,
) -> Result<bool, i32> {
    if (xa && (!(3..=4).contains(&stream.kind) || samples < 0))
        || (!xa && !(0..=2).contains(&stream.kind))
    {
        return Ok(false);
    }
    if !xa && stream.kind != 0 && stream.tdd.is_null() {
        return Ok(false);
    }
    let mut data = ptr::null();
    let mut length = 0;
    let mut valid = 0;
    let status = unsafe { source(context, &mut data, &mut length, &mut valid) };
    if status != 0 {
        return Err(status);
    }
    if valid == 0 || ((!xa || length > 0) && data.is_null()) {
        return Ok(false);
    }
    unsafe {
        if xa {
            if length > i32::MAX as usize {
                return Ok(false);
            }
            let bytes = if length == 0 {
                &[]
            } else {
                std::slice::from_raw_parts(data, length)
            };
            let mut state = Box::new(XaState {
                encoded: bytes.into(),
                decoder: hp2_codecs::eaxa::Decoder::new(),
            });
            if !state.refeed(samples) {
                return Ok(false);
            }
            stream.handle = ptr::null_mut();
            stream.tdd = Box::into_raw(state).cast();
            stream.num_samples = samples;
        } else {
            let input = data.cast::<hp2_core::ansi::Tchar>();
            let units = hp2_core::unix_text::tchar_length(input);
            let Some(capacity) = units.checked_mul(4).and_then(|size| size.checked_add(1)) else {
                return Ok(false);
            };
            let mut path = Vec::with_capacity(capacity);
            let Some(length) = hp2_core::path::native_path_to_utf8(
                input, path.as_mut_ptr(), capacity - 1,
            ) else {
                return Ok(false);
            };
            path.set_len(length);
            path.push(0);
            if stream.kind == HP2_STREAM_REGULAR {
                let file = fopen(path.as_ptr().cast(), c"rb".as_ptr());
                if file.is_null() {
                    return Ok(false);
                }
                stream.handle = file;
                stream.tdd = ptr::null_mut();
            } else {
                let Some(path) = std::str::from_utf8(&path[..length]).ok() else {
                    return Ok(false);
                };
                if !(&mut *stream.tdd.cast::<hp2_codecs::vorbis::Decoder>()).open(path) {
                    return Ok(false);
                }
                stream.handle = stream.tdd;
            }
            stream.num_samples = -1;
        }
    }
    Ok(true)
}

/// # Safety
/// Source/context must remain valid synchronously. Destination holds
/// chunk_size*initial_chunks writable bytes, with no sample-alignment requirement;
/// Ogg TDD is caller-owned until success.
pub unsafe extern "C" fn hp2_rs_stream_create(
    instance: *mut c_void,
    source: Hp2StreamSource,
    context: *mut c_void,
    xa: i32,
    samples: i32,
    chunk_size: i32,
    initial_chunks: i32,
    destination: *mut c_void,
    kind: i32,
    tdd: *mut c_void,
    output: *mut i32,
) -> i32 {
    guard("hp2_rs_stream_create", 1, || unsafe {
        *output = -1;
        let _lifecycle = lock(&LIFECYCLE);
        let Some(runtime) = runtime(instance) else {
            return 0;
        };
        if chunk_size <= 0
            || initial_chunks < 0
            || initial_chunks > i32::MAX / chunk_size
            || (initial_chunks > 0 && destination.is_null())
        {
            return 0;
        }
        for id in 0..runtime.streams.len() {
            let _permit = runtime.locks[id].permit();
            let stream = &mut *runtime.streams[id].get();
            if stream.used != 0 {
                continue;
            }
            stream.kind = kind;
            if xa == 0 {
                stream.tdd = tdd;
            }
            match create_resource(stream, source, context, xa != 0, samples) {
                Ok(true) => {}
                result => {
                    reset(stream); // failed Ogg creation does not adopt TDD
                    return result.err().unwrap_or(0);
                }
            }
            stream.chunk_size = chunk_size;
            stream.end_of_file = 0;
            stream.used = 1;
            if initial_chunks > 0 {
                read_stream(stream, chunk_size * initial_chunks, destination);
            }
            *output = id as i32;
            return 0;
        }
        0
    })
}


/// # Safety
/// A queued destination provides `chunk_size` writable bytes, with no sample-alignment
/// requirement, until completed, cancelled, or destroy returns. Capacity counts
/// outstanding completions, not only pending reads.
/// In deterministic verification mode the read completes before this returns.
pub unsafe extern "C" fn hp2_rs_stream_request(
    instance: *mut c_void,
    id: i32,
    destination: *mut c_void,
) -> i32 {
    guard("hp2_rs_stream_request", 0, || unsafe {
        let _lifecycle = lock(&LIFECYCLE);
        let Some(runtime) = runtime(instance) else {
            return 0;
        };
        let Some(id) = valid_id(&runtime, id) else {
            return 0;
        };
        if destination.is_null() {
            return 0;
        }
        let _permit = runtime.locks[id].permit();
        let stream = &mut *runtime.streams[id].get();
        if stream.used == 0
            || stream.end_of_file != 0
            || stream.pending_count >= HP2_STREAM_CAPACITY
            || stream.completion_count >= HP2_STREAM_CAPACITY
        {
            return 0;
        }
        if crate::verification::enabled() {
            // Deterministic verification: complete the refill under the same
            // per-stream lock and with the same decoder/queue/cancellation
            // lifecycle the worker uses, but before returning, so a pending
            // chunk never waits on OS scheduling. The pending queue stays empty
            // and the completion ring keeps the production capacity bound.
            let index =
                (stream.completion_head + stream.completion_count) % HP2_STREAM_CAPACITY;
            stream.completed_chunks[index] = Hp2StreamCompletion {
                destination,
                terminal: 0,
                ready: 0,
            };
            stream.completion_count += 1;
            complete_read(&runtime, stream, index, destination);
            return 1;
        }
        let mut work = lock(&runtime.work);
        if work.pending >= i32::MAX as usize {
            return 0;
        }
        let tail = (stream.completion_head + stream.completion_count) % HP2_STREAM_CAPACITY;
        stream.completed_chunks[tail] = Hp2StreamCompletion {
            destination,
            terminal: 0,
            ready: 0,
        };
        stream.completion_count += 1;
        let tail = (stream.pending_head + stream.pending_count) % HP2_STREAM_CAPACITY;
        stream.pending_destinations[tail] = destination;
        stream.pending_count += 1;
        work.pending += 1;
        drop(work);
        runtime.ready.notify_one();
        1
    })
}

/// # Safety
/// Output pointers are writable; outputs remain unchanged when no completion is ready.
pub unsafe extern "C" fn hp2_rs_stream_pop(
    instance: *mut c_void,
    id: i32,
    destination: *mut *mut c_void,
    terminal: *mut i32,
) -> i32 {
    guard("hp2_rs_stream_pop", 0, || unsafe {
        let _lifecycle = lock(&LIFECYCLE);
        let Some(runtime) = runtime(instance) else {
            return 0;
        };
        let Some(id) = valid_id(&runtime, id) else {
            return 0;
        };
        let _permit = runtime.locks[id].permit();
        let stream = &mut *runtime.streams[id].get();
        if stream.completion_count == 0 {
            return 0;
        }
        let completion = stream.completed_chunks[stream.completion_head];
        if completion.ready == 0 {
            return 0;
        }
        stream.completion_head = (stream.completion_head + 1) % HP2_STREAM_CAPACITY;
        stream.completion_count -= 1;
        if stream.completion_count == 0 {
            stream.completion_head = 0;
        }
        *destination = completion.destination;
        *terminal = completion.terminal;
        1
    })
}

/// # Safety
/// Instance is the opaque Arc allocation identity, never dereferenced here.
pub unsafe extern "C" fn hp2_rs_stream_alive(instance: *mut c_void, id: i32) -> i32 {
    guard("hp2_rs_stream_alive", 0, || unsafe {
        let _lifecycle = lock(&LIFECYCLE);
        let Some(runtime) = runtime(instance) else {
            return 0;
        };
        let Some(id) = valid_id(&runtime, id) else {
            return 0;
        };
        let _permit = runtime.locks[id].permit();
        let stream = &*runtime.streams[id].get();
        (stream.used != 0 && stream.end_of_file == 0) as i32
    })
}

/// # Safety
/// Queued destinations remain writable through this call when drain is nonzero.
pub unsafe extern "C" fn hp2_rs_stream_destroy(
    instance: *mut c_void,
    id: i32,
    drain: i32,
) -> i32 {
    guard("hp2_rs_stream_destroy", 0, || unsafe {
        let _lifecycle = lock(&LIFECYCLE);
        let Some(runtime) = runtime(instance) else {
            return 0;
        };
        let Some(id) = valid_id(&runtime, id) else {
            return 0;
        };
        let _permit = runtime.locks[id].permit();
        let stream = &mut *runtime.streams[id].get();
        if stream.used == 0 {
            return 0;
        }
        if drain != 0 {
            while stream.pending_count > 0 {
                let destination = take_pending(&runtime, stream);
                read_stream(stream, stream.chunk_size, destination);
            }
        }
        destroy_stream(&runtime, stream) as i32
    })
}

/// # Safety
/// Enter/Leave must pair on the same thread. Do not call another operation on
/// the held stream, or Init/Destroy, while holding its nonrecursive lock.
pub unsafe extern "C" fn hp2_rs_stream_enter(instance: *mut c_void, id: i32) {
    guard("hp2_rs_stream_enter", (), || {
        let Some(runtime) = runtime(instance) else {
            return;
        };
        let Some(id) = valid_id(&runtime, id) else {
            return;
        };
        runtime.locks[id].acquire();
        let lease = Lease { runtime, id };
        // Shutdown may have started after the registry lookup. A late entrant
        // must not acquire a lease on an already-reset or unpublished record.
        if lock(&lease.runtime.work).stop {
            return;
        }
        LEASES.with(|leases| leases.borrow_mut().push(lease));
    })
}

/// # Safety
/// Must match this thread's Enter; it remains valid while another thread is
/// joining the worker in Destroy (which must never block Leave's lookup).
pub unsafe extern "C" fn hp2_rs_stream_leave(instance: *mut c_void, id: i32) {
    guard("hp2_rs_stream_leave", (), || {
        LEASES.with(|leases| {
            let mut leases = leases.borrow_mut();
            if let Some(index) = leases
                .iter()
                .rposition(|lease| Arc::as_ptr(&lease.runtime).cast_mut().cast::<c_void>() == instance && lease.id as i32 == id)
            {
                drop(leases.remove(index));
            }
        });
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    // These tests use the real singleton/worker/decoder, serialized because the
    // production ABI intentionally has one manager. No filesystem artifacts.
    static TEST_SERIAL: Mutex<()> = Mutex::new(());

    struct Manager {
        instance: *mut c_void,
    }
    impl Manager {
        fn new(capacity: i32) -> Self {
            let mut instance = ptr::null_mut();
            assert_eq!(unsafe { hp2_rs_stream_init(capacity, &mut instance) }, 0);
            assert!(!instance.is_null());
            Self { instance }
        }
        fn instance(&self) -> *mut c_void {
            self.instance
        }
        fn xa(&self, kind: i32, source: &mut Source) -> i32 {
            let mut id = -1;
            assert_eq!(
                unsafe {
                    hp2_rs_stream_create(
                        self.instance(),
                        xa_source,
                        (source as *mut Source).cast(),
                        1,
                        28,
                        2,
                        0,
                        ptr::null_mut(),
                        kind,
                        ptr::null_mut(),
                        &mut id,
                    )
                },
                0
            );
            assert!(id >= 0);
            id
        }
    }
    impl Drop for Manager {
        fn drop(&mut self) {
            // Ensure a failing assertion cannot strand an explicit lease and
            // deadlock the test runner's shutdown.
            LEASES.with(|leases| leases.borrow_mut().clear());
            unsafe {
                hp2_rs_stream_shutdown();
            }
        }
    }
    struct Source {
        bytes: [u8; 15],
        status: i32,
    }
    unsafe extern "C" fn xa_source(
        context: *mut c_void,
        bytes: *mut *const u8,
        length: *mut usize,
        valid: *mut i32,
    ) -> i32 {
        unsafe {
            let source = &*context.cast::<Source>();
            *bytes = source.bytes.as_ptr();
            *length = source.bytes.len();
            *valid = 1;
            source.status
        }
    }
    struct OneShotSource {
        bytes: [u8; 15],
        calls: usize,
    }

    unsafe extern "C" fn one_shot_xa_source(
        context: *mut c_void,
        bytes: *mut *const u8,
        length: *mut usize,
        valid: *mut i32,
    ) -> i32 {
        // SAFETY: the test passes a live OneShotSource and writable outputs.
        unsafe {
            let source = &mut *context.cast::<OneShotSource>();
            source.calls += 1;
            if source.calls != 1 {
                return 37;
            }
            *bytes = source.bytes.as_ptr();
            *length = source.bytes.len();
            *valid = 1;
            0
        }
    }

    fn pop(instance: *mut c_void, id: i32) -> (*mut c_void, i32) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let mut destination = ptr::null_mut();
            let mut terminal = -1;
            if unsafe { hp2_rs_stream_pop(instance, id, &mut destination, &mut terminal) } != 0 {
                return (destination, terminal);
            }
            assert!(Instant::now() < deadline, "stream completion timed out");
            thread::yield_now();
        }
    }

    #[test]
    fn xa_odd_address_short_read_zero_fills_tail_with_canaries() {
        let _serial = lock(&TEST_SERIAL);
        let manager = Manager::new(1);
        let mut source = Source { bytes: [0x33; 15], status: 0 };
        source.bytes[0] = 12;
        let mut backing = [0xa5a5u16; 6];
        let bytes = unsafe { std::slice::from_raw_parts_mut(backing.as_mut_ptr().cast::<u8>(), 12) };
        let destination = unsafe { bytes.as_mut_ptr().add(1) }.cast::<c_void>();
        let mut id = -1;
        // Initial read uses the production stream path: 2 samples, then EOF.
        assert_eq!(unsafe {
            hp2_rs_stream_create(manager.instance(), xa_source, (&mut source as *mut Source).cast(),
                1, 2, 8, 1, destination, HP2_STREAM_XA, ptr::null_mut(), &mut id)
        }, 0);
        assert!(id >= 0);
        let mut expected = Vec::new();
        expected.extend_from_slice(&3i16.to_ne_bytes());
        expected.extend_from_slice(&3i16.to_ne_bytes());
        expected.extend_from_slice(&[0; 4]);
        assert_eq!(&bytes[1..9], expected);
        assert_eq!(bytes[0], 0xa5);
        assert_eq!(&bytes[9..], &[0xa5; 3]);
        assert_eq!(unsafe { hp2_rs_stream_alive(manager.instance(), id) }, 0);
    }

    #[test]
    fn xa_odd_address_looping_refeed_failure_preserves_partial_pcm_and_tail() {
        let _serial = lock(&TEST_SERIAL);
        let manager = Manager::new(1);
        let mut source = OneShotSource { bytes: [0x33; 15], calls: 0 };
        source.bytes[0] = 12;
        let mut id = -1;
        // SAFETY: the source and output remain live for the synchronous create call.
        let status = unsafe {
            hp2_rs_stream_create(
                manager.instance(), one_shot_xa_source, (&mut source as *mut OneShotSource).cast(),
                1, 2, 8, 0, ptr::null_mut(), HP2_STREAM_XA_LOOPING, ptr::null_mut(), &mut id,
            )
        };
        assert_eq!((status, id, source.calls), (0, 0, 1));

        let mut backing = [0xa5a5u16; 6];
        // SAFETY: the backing array contains 12 initialized bytes. Offset 1 is odd.
        let bytes = unsafe { std::slice::from_raw_parts_mut(backing.as_mut_ptr().cast::<u8>(), 12) };
        let destination = unsafe { bytes.as_mut_ptr().add(1) }.cast::<c_void>();
        let runtime = runtime(manager.instance()).unwrap();
        let _permit = runtime.locks[id as usize].permit();
        // The source callback is never called on XA refeed: the encoded copy is
        // retained. Inject an invalid refeed count to exercise that failure path.
        // SAFETY: the stream lock is held and the destination has 8 writable bytes.
        let stream = unsafe { &mut *runtime.streams[id as usize].get() };
        stream.num_samples = -1;
        assert!(!unsafe { read_stream(stream, 8, destination) });
        assert_eq!(stream.end_of_file, 1);
        assert_eq!(source.calls, 1);
        let sample = 3i16.to_ne_bytes();
        assert_eq!(&bytes[1..9], &[sample[0], sample[1], sample[0], sample[1], 0, 0, 0, 0]);
        assert_eq!(bytes[0], 0xa5);
        assert_eq!(&bytes[9..], &[0xa5; 3]);
    }

    #[test]
    fn xa_odd_address_empty_loop_stops_without_progress() {
        let _serial = lock(&TEST_SERIAL);
        let manager = Manager::new(1);
        let mut source = OneShotSource { bytes: [0; 15], calls: 0 };
        let mut backing = [0xa5a5u16; 4];
        // SAFETY: the backing array contains 8 initialized bytes. Offset 1 is odd.
        let bytes = unsafe { std::slice::from_raw_parts_mut(backing.as_mut_ptr().cast::<u8>(), 8) };
        let destination = unsafe { bytes.as_mut_ptr().add(1) }.cast::<c_void>();
        let mut id = -1;
        // SAFETY: the source and output are live and the destination holds 4 bytes.
        let status = unsafe {
            hp2_rs_stream_create(
                manager.instance(), one_shot_xa_source, (&mut source as *mut OneShotSource).cast(),
                1, 0, 4, 1, destination, HP2_STREAM_XA_LOOPING, ptr::null_mut(), &mut id,
            )
        };
        assert_eq!((status, id, source.calls), (0, 0, 1));
        assert_eq!(&bytes[1..5], &[0; 4]);
        assert_eq!(bytes[0], 0xa5);
        assert_eq!(&bytes[5..], &[0xa5; 3]);
        assert_eq!(unsafe { hp2_rs_stream_alive(manager.instance(), id) }, 0);
    }

    #[test]
    fn full_ring_duplicate_destinations_and_wraparound_preserve_reservations() {
        let _serial = lock(&TEST_SERIAL);
        let manager = Manager::new(2);
        let mut source = Source {
            bytes: [0; 15],
            status: 0,
        };
        let blocker = manager.xa(4, &mut source);
        let stream_id = manager.xa(4, &mut source);
        let instance = manager.instance();
        let mut sample = 0i16;
        let destination = (&mut sample as *mut i16).cast::<c_void>();
        unsafe {
            hp2_rs_stream_enter(instance, blocker);
        }
        for _ in 0..HP2_STREAM_CAPACITY {
            assert_eq!(
                unsafe { hp2_rs_stream_request(instance, stream_id, destination) },
                1
            );
        }
        assert_eq!(
            unsafe { hp2_rs_stream_request(instance, stream_id, destination) },
            0
        );
        unsafe {
            hp2_rs_stream_leave(instance, blocker);
        }
        // Keep the ring full while both heads wrap. A duplicate address must
        // retain one completion per accepted request, not collapse into a map.
        for _ in 0..HP2_STREAM_CAPACITY + 7 {
            assert_eq!(pop(instance, stream_id), (destination, 0));
            assert_eq!(
                unsafe { hp2_rs_stream_request(instance, stream_id, destination) },
                1
            );
        }
        for _ in 0..HP2_STREAM_CAPACITY {
            assert_eq!(pop(instance, stream_id), (destination, 0));
        }
        let mut untouched_destination = destination;
        let mut untouched_terminal = 77;
        assert_eq!(
            unsafe {
                hp2_rs_stream_pop(
                    instance,
                    stream_id,
                    &mut untouched_destination,
                    &mut untouched_terminal,
                )
            },
            0
        );
        assert_eq!(untouched_destination, destination);
        assert_eq!(untouched_terminal, 77);
    }

    #[test]
    fn leave_remains_available_while_shutdown_holds_lifecycle() {
        let _serial = lock(&TEST_SERIAL);
        let manager = Manager::new(1);
        let mut source = Source {
            bytes: [0; 15],
            status: 0,
        };
        let stream_id = manager.xa(4, &mut source);
        let instance = manager.instance();
        let live = runtime(instance).unwrap();
        unsafe {
            hp2_rs_stream_enter(instance, stream_id);
        }
        let (done, finished) = std::sync::mpsc::channel();
        let shutdown = thread::spawn(move || {
            let status = unsafe { hp2_rs_stream_shutdown() };
            done.send(status).unwrap();
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        while !lock(&live.work).stop {
            assert!(
                Instant::now() < deadline,
                "shutdown did not request worker stop"
            );
            thread::yield_now();
        }
        // Destroy holds LIFECYCLE and cannot finish while the lease is held.
        assert!(finished.try_recv().is_err());
        unsafe {
            hp2_rs_stream_leave(instance, stream_id);
        }
        assert_eq!(finished.recv_timeout(Duration::from_secs(5)).unwrap(), 0);
        shutdown.join().unwrap();
    }

    #[test]
    fn callback_failure_resets_reservation_and_propagates_status() {
        let _serial = lock(&TEST_SERIAL);
        let manager = Manager::new(1);
        let mut source = Source {
            bytes: [0; 15],
            status: 37,
        };
        let mut id = 123;
        assert_eq!(
            unsafe {
                hp2_rs_stream_create(
                    manager.instance(),
                    xa_source,
                    (&mut source as *mut Source).cast(),
                    1,
                    28,
                    2,
                    0,
                    ptr::null_mut(),
                    4,
                    ptr::null_mut(),
                    &mut id,
                )
            },
            37
        );
        assert_eq!(id, -1);
        source.status = 0;
        assert!(manager.xa(4, &mut source) >= 0);
    }

    // The mode is a process-start choice, so the enabled half runs in a child
    // re-exec of this test binary. This exercises the public stream ABI only.
    #[test]
    fn deterministic_request_publishes_completion_before_returning() {
        if std::env::var_os(crate::verification::ENV).as_deref()
            != Some(std::ffi::OsStr::new("1"))
        {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "file_stream::tests::deterministic_request_publishes_completion_before_returning",
                    "--nocapture",
                ])
                .env(crate::verification::ENV, "1")
                .output()
                .unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                output.status.success() && stdout.contains("1 passed"),
                "deterministic child did not run the enabled test: {}\n{stdout}\n{stderr}",
                output.status,
            );
            return;
        }
        let _serial = lock(&TEST_SERIAL);
        let manager = Manager::new(1);
        let mut source = Source {
            bytes: [0; 15],
            status: 0,
        };
        let stream_id = manager.xa(4, &mut source);
        let instance = manager.instance();
        let mut samples = [0i16; 1];
        let destination = samples.as_mut_ptr().cast::<c_void>();
        assert_eq!(
            unsafe { hp2_rs_stream_request(instance, stream_id, destination) },
            1
        );
        // The work queue was never armed: the completion was produced inline.
        assert_eq!(lock(&runtime(instance).unwrap().work).pending, 0);
        // No worker turn has been requested or awaited; the completion must
        // already be visible.
        let mut reported = ptr::null_mut();
        let mut terminal = -1;
        assert_eq!(
            unsafe { hp2_rs_stream_pop(instance, stream_id, &mut reported, &mut terminal) },
            1
        );
        assert_eq!(reported, destination);
        assert_eq!(terminal, 0);
        // The completion ring still bounds outstanding requests exactly like
        // the asynchronous path.
        for _ in 0..HP2_STREAM_CAPACITY {
            assert_eq!(
                unsafe { hp2_rs_stream_request(instance, stream_id, destination) },
                1
            );
        }
        assert_eq!(
            unsafe { hp2_rs_stream_request(instance, stream_id, destination) },
            0
        );
    }
}
