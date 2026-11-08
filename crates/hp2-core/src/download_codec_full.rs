//! Full codec sequencing with native lexical archive/time temporary scopes.
use crate::download_codec::CodecResult;
use std::ffi::c_void;

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2CodecFullLeaf {
    Count = 0,
    SetNotify = 1,
    ProgressState = 2,
    WithStage = 4,
    BeginStage = 5,
    CodecAt = 6,
    InvokeCodec = 7,
    Elapsed = 8,
    LogStage = 9,
    AssignBuffers = 10,
    EmptyOutput = 11,
    LogTotal = 12,
    Notify = 13,
    DeleteCodec = 14,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2CodecFullIo {
    pub scope: *mut c_void,
    pub object: *mut c_void,
    pub continuation_context: *mut c_void,
    pub continuation: Option<unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32>,
    pub index: i32,
    pub value: i32,
    pub progress: f32,
}

impl Default for Hp2CodecFullIo {
    fn default() -> Self {
        Self {
            scope: std::ptr::null_mut(),
            object: std::ptr::null_mut(),
            continuation_context: std::ptr::null_mut(),
            continuation: None,
            index: 0,
            value: 0,
            progress: 0.0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2CodecFullServices {
    pub context: *mut c_void,
    pub invoke: unsafe extern "C" fn(*mut c_void, Hp2CodecFullLeaf, *mut Hp2CodecFullIo) -> i32,
}

struct FullIo {
    services: Hp2CodecFullServices,
}

impl FullIo {
    fn call(&self, leaf: Hp2CodecFullLeaf, io: &mut Hp2CodecFullIo) -> CodecResult<()> {
        // SAFETY: invocation contract; the table is immutable, owner stays raw.
        let status = unsafe { (self.services.invoke)(self.services.context, leaf, io) };
        if status == 0 { Ok(()) } else { Err(()) }
    }
    fn integer(&self, leaf: Hp2CodecFullLeaf) -> CodecResult<i32> {
        let mut io = Hp2CodecFullIo::default();
        self.call(leaf, &mut io)?;
        Ok(io.value)
    }
    fn count(&self) -> CodecResult<i32> {
        self.integer(Hp2CodecFullLeaf::Count)
    }
}

struct StageCall<'a> {
    io: &'a FullIo,
    index: i32,
    first: i32,
    step: i32,
    total_time: *mut f32,
    completed: bool,
}

unsafe fn stage(call: &mut StageCall<'_>, scope: *mut c_void) -> CodecResult<()> {
    let io = call.io;
    let mut value = Hp2CodecFullIo {
        scope,
        index: call.index,
        ..Default::default()
    };
    io.call(Hp2CodecFullLeaf::BeginStage, &mut value)?;
    value.index = call.first.wrapping_add(call.step.wrapping_mul(call.index));
    io.call(Hp2CodecFullLeaf::CodecAt, &mut value)?;
    let count = io.count()?;
    value.value = i32::from(call.index != 0) | (i32::from(call.index < count.wrapping_sub(1)) << 1);
    io.call(Hp2CodecFullLeaf::InvokeCodec, &mut value)?;
    if value.value == 0 {
        return Ok(());
    }
    io.call(Hp2CodecFullLeaf::Elapsed, &mut value)?;
    // SAFETY: the Rust invocation-local accumulator survives this continuation.
    unsafe {
        *call.total_time += value.progress;
    }
    value.index = call.index;
    io.call(Hp2CodecFullLeaf::LogStage, &mut value)?;
    if call.index < io.count()?.wrapping_sub(1) {
        io.call(Hp2CodecFullLeaf::AssignBuffers, &mut value)?;
        io.call(Hp2CodecFullLeaf::EmptyOutput, &mut value)?;
    }
    call.completed = true;
    Ok(())
}

// No allocation, indexing, unwrap or panic-producing arithmetic in this thunk.
// Each native service contains its own exceptions/signals and returns status.
unsafe extern "C" fn stage_continuation(context: *mut c_void, scope: *mut c_void) -> i32 {
    // SAFETY: WithStage calls once synchronously while both actual native buffer
    // archives and time temporaries, and this Rust invocation-local record, live.
    let call = unsafe { &mut *context.cast::<StageCall<'_>>() };
    match unsafe { stage(call, scope) } {
        Ok(()) => 0,
        Err(()) => 1,
    }
}

/// # Safety
/// The copied services retain live native owners and checked callback status.
/// WithStage invokes its continuation synchronously inside genuine typed scopes.
pub unsafe fn run(
    services: Hp2CodecFullServices,
    decode: bool,
    notify: *mut c_void,
) -> CodecResult<i32> {
    let io = FullIo { services };
    let (first, step) = if decode {
        (io.count()?.wrapping_sub(1), -1)
    } else {
        (0, 1)
    };
    let mut value = Hp2CodecFullIo {
        object: notify,
        ..Default::default()
    };
    io.call(Hp2CodecFullLeaf::SetNotify, &mut value)?;
    let mut total_time = 0.0f32;
    let mut index = 0i32;
    while index < io.count()? {
        let mut call = StageCall {
            io: &io,
            index,
            first,
            step,
            total_time: &raw mut total_time,
            completed: false,
        };
        value.continuation = Some(stage_continuation);
        value.continuation_context = (&raw mut call).cast();
        io.call(Hp2CodecFullLeaf::WithStage, &mut value)?;
        if !call.completed {
            return Ok(0);
        }
        index = index.wrapping_add(1);
    }
    value.progress = total_time;
    io.call(Hp2CodecFullLeaf::LogTotal, &mut value)?;
    Ok(1)
}

/// # Safety
/// Native stage, notifier and collection fields are accessed live by services.
pub unsafe fn progress(services: Hp2CodecFullServices, progress: f32) -> CodecResult<i32> {
    let io = FullIo { services };
    let mut value = Hp2CodecFullIo::default();
    io.call(Hp2CodecFullLeaf::ProgressState, &mut value)?;
    if value.object.is_null() {
        return Ok(1);
    }
    // No callback occurs between the original plain stage/count reads. One
    // native snapshot preserves live reentry while avoiding per-byte getters.
    let count = value.value as f32;
    value.progress = value.index as f32 / count + progress / count;
    io.call(Hp2CodecFullLeaf::Notify, &mut value)?;
    Ok(i32::from(value.value != 0))
}

/// # Safety
/// DeleteCodec performs the actual original typed native deletion. The native
/// collection stays alive until its C++ owner finishes destruction after return.
pub unsafe fn destroy(services: Hp2CodecFullServices) -> CodecResult<()> {
    let io = FullIo { services };
    let mut index = 0i32;
    while index < io.count()? {
        let mut value = Hp2CodecFullIo {
            index,
            ..Default::default()
        };
        io.call(Hp2CodecFullLeaf::DeleteCodec, &mut value)?;
        index = index.wrapping_add(1);
    }
    Ok(())
}
