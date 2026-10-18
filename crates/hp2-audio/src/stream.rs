//! Port of the streaming-buffer state machine inside
//! `UALAudioSubsystem::Update` (`ALAudioSubsystem.cpp:1495-1597`,
//! `UpdateStreamingSounds`) and the `EALStreamBufferState` enum
//! (`ALAudioSubsystem.h`).
//!
//! The OpenAL and `GFileStream` calls are made by the caller through `Host`
//! (`host.rs`) inside the closures passed to `tick`. Each closure is one C++
//! call site, invoked in the same order the C++ loop makes them, so the
//! sequence of side effects is identical. A `check(...)` that fires in C++
//! becomes `TickError::Stream`; an engine call that throws becomes
//! `TickError::Host`.

use crate::host::{HostFailed, HostResult};

/// Matches `EALStreamBufferState` (`ALAudioSubsystem.h`): the C++ enum's
/// implicit 0..4 values, so the byte form is the C++ `(BYTE)` cast.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferState {
    QueuedPending = 0,
    QueuedReady = 1,
    FreePending = 2,
    FreeReady = 3,
    QueuedNoPrefetch = 4,
}

impl BufferState {
    /// Inverse of `self as u8`; `None` for a byte no C++ enum value has.
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::QueuedPending),
            1 => Some(Self::QueuedReady),
            2 => Some(Self::FreePending),
            3 => Some(Self::FreeReady),
            4 => Some(Self::QueuedNoPrefetch),
            _ => None,
        }
    }
}

/// Maximum buffers per stream (`MAX_BUFFERS_PER_STREAM` in
/// `ALAudioSubsystem.h`).
pub const MAX_BUFFERS_PER_STREAM: usize = 3;

/// The C++ `check(...)` failures this state machine can hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamError {
    /// `check(BufferIndex != INDEX_NONE)` after an unqueue (`:1521`).
    UnqueuedBufferNotFound,
    /// Unqueued buffer in a state that has no transition (`:1528`).
    UnqueueFromBadState,
    /// Completed chunk matched `!= 1` refill destinations (`:1545`).
    CompletionMatchCount(usize),
    /// Completed chunk in a state that has no transition (`:1552`).
    CompletionFromBadState,
    /// `check(Stream.RefillData[BufferIndex] != NULL)` (`:1566`).
    NullRefillData,
}

/// Transition for a buffer OpenAL reports as processed (`:1523-1528`).
/// `QueuedNoPrefetch` stays put.
pub fn on_unqueued(state: BufferState) -> Result<BufferState, StreamError> {
    match state {
        BufferState::QueuedPending => Ok(BufferState::FreePending),
        BufferState::QueuedReady => Ok(BufferState::FreeReady),
        BufferState::QueuedNoPrefetch => Ok(BufferState::QueuedNoPrefetch),
        _ => Err(StreamError::UnqueueFromBadState),
    }
}

/// Transition for a chunk `GFileStream` reports as completed
/// (`:1547-1552`).
pub fn on_completed(state: BufferState) -> Result<BufferState, StreamError> {
    match state {
        BufferState::QueuedPending => Ok(BufferState::QueuedReady),
        BufferState::FreePending => Ok(BufferState::FreeReady),
        _ => Err(StreamError::CompletionFromBadState),
    }
}

/// The post-refill state for a buffer that was just queued
/// (`:1577-1585`). A prefetch request that is accepted, while no
/// terminal chunk has been observed, leaves it pending; otherwise it
/// stops prefetching.
pub fn after_refill_queue(request_accepted: bool, terminal_observed: bool) -> BufferState {
    if !terminal_observed && request_accepted {
        BufferState::QueuedPending
    } else {
        BufferState::QueuedNoPrefetch
    }
}

/// Per-stream state the C++ keeps in `ALStream`
/// (`ALAudioSubsystem.h`), restricted to the fields this loop reads.
/// `buffers` holds the OpenAL buffer names and `refill` the
/// destination pointers (as opaque `usize`); only the first
/// `num_buffers` entries are live.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamState {
    pub buffers: [u32; MAX_BUFFERS_PER_STREAM],
    pub refill: [usize; MAX_BUFFERS_PER_STREAM],
    pub state: [BufferState; MAX_BUFFERS_PER_STREAM],
    pub num_buffers: usize,
    pub alive: bool,
    pub reset: bool,
}

/// What one `UpdateStreamingSounds` pass did, for the caller.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TickOutcome {
    /// `alSourcePlay` was issued (the C++ `Reset` branch).
    pub played: bool,
}

/// Failure of one `tick` pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickError {
    /// A `check(...)` in the original loop fired.
    Stream(StreamError),
    /// An engine call made through `Host` threw.
    Host(HostFailed),
}

impl From<StreamError> for TickError {
    fn from(e: StreamError) -> Self {
        TickError::Stream(e)
    }
}

impl From<HostFailed> for TickError {
    fn from(e: HostFailed) -> Self {
        TickError::Host(e)
    }
}

/// One `UpdateStreamingSounds` pass (`23ac187:HarryPotter2/Unreal/ALAudio/Src/ALAudioSubsystem.cpp:1497-1596`).
///
/// - `processed`: the value of `alGetSourcei(AL_BUFFERS_PROCESSED)`.
/// - `unqueue()`: `alSourceUnqueueBuffers(Source, 1, &b)`, returns `b`.
///   Called `processed` times, in order.
/// - `pop_completed()`: `GFileStream->PopCompletedChunk(Id-1, dest,
///   terminal)`. Returns `Some((dest, terminal))` until it returns
///   `None`.
/// - `queue(buffer_index)`: `alBufferData` followed by
///   `alSourceQueueBuffers` for `Buffer[buffer_index]`.
/// - `request(dest)`: `GFileStream->RequestChunk(Id-1, dest)`.
///
/// Stream-level `Alive`, `Reset`, and per-buffer `BufferState` are
/// updated in place, exactly as the C++ updates them.
pub fn tick<U, P, Q, R>(
    stream: &mut StreamState,
    processed: i32,
    mut unqueue: U,
    mut pop_completed: P,
    mut queue: Q,
    mut request: R,
) -> Result<TickOutcome, TickError>
where
    U: FnMut() -> HostResult<u32>,
    P: FnMut() -> HostResult<Option<(usize, bool)>>,
    Q: FnMut(usize) -> HostResult<()>,
    R: FnMut(usize) -> HostResult<bool>,
{
    let n = stream.num_buffers;
    // `UBOOL TerminalObserved = !Stream.Alive;` (`:1503`)
    let mut terminal_observed = !stream.alive;

    // Unqueue every processed buffer (`:1506-1529`).
    for _ in 0..processed.max(0) {
        let name = unqueue()?;
        let index = (0..n)
            .find(|&c| stream.buffers[c] == name)
            .ok_or(StreamError::UnqueuedBufferNotFound)?;
        stream.state[index] = on_unqueued(stream.state[index])?;
    }

    // Drain completed chunks (`:1533-1559`).
    while let Some((dest, terminal)) = pop_completed()? {
        let mut matches = 0usize;
        let mut index = None;
        for c in 0..n {
            if stream.refill[c] == dest {
                index = Some(c);
                matches += 1;
            }
        }
        if matches != 1 {
            return Err(StreamError::CompletionMatchCount(matches).into());
        }
        let index = index.expect("matches == 1 implies an index");
        stream.state[index] = on_completed(stream.state[index])?;
        if terminal {
            terminal_observed = true;
            stream.alive = false;
        }
    }

    // Refill every buffer that is free and ready (`:1561-1586`).
    for index in 0..n {
        if stream.state[index] != BufferState::FreeReady {
            continue;
        }
        if stream.refill[index] == 0 {
            return Err(StreamError::NullRefillData.into());
        }
        queue(index)?;
        let accepted = if !terminal_observed {
            request(stream.refill[index])?
        } else {
            false
        };
        stream.state[index] = after_refill_queue(accepted, terminal_observed);
    }

    // Restart after running out of buffers (`:1588-1593`).
    let mut outcome = TickOutcome::default();
    if stream.reset {
        outcome.played = true;
        stream.reset = false;
    }
    Ok(outcome)
}

// Transition and call-sequence expectations below are pinned to the
// original loop: `/tmp/hp2-m2/audio/stream/stream_oracle.cpp` includes the
// verbatim text (`orig_stream.inc`) and is built with
// `clang++ -std=c++17 -w -O0 -ffp-contract=off stream_oracle.cpp`; its output
// is `stream_oracle.out`. `host_failure_during_tick_is_propagated` has no C++
// counterpart (`alError` only logs) and is Rust-only.
#[cfg(test)]
mod tests {
    use super::*;

    /// The C++ side copies `EALStreamBufferState` as a byte (`(BYTE)`), so
    /// each value must round-trip to the C++ enum's implicit 0..4 and
    /// nothing else may decode.
    #[test]
    fn buffer_state_byte_values_match_cpp_enum() {
        for (byte, state) in [
            (0u8, BufferState::QueuedPending),
            (1, BufferState::QueuedReady),
            (2, BufferState::FreePending),
            (3, BufferState::FreeReady),
            (4, BufferState::QueuedNoPrefetch),
        ] {
            assert_eq!(BufferState::from_u8(byte), Some(state));
            assert_eq!(state as u8, byte);
        }
        assert_eq!(BufferState::from_u8(5), None);
        assert_eq!(BufferState::from_u8(u8::MAX), None);
    }

    fn stream(state: [BufferState; 3]) -> StreamState {
        StreamState {
            buffers: [11, 22, 33],
            refill: [0x100, 0x200, 0x300],
            state,
            num_buffers: 3,
            alive: true,
            reset: false,
        }
    }

    #[test]
    fn unqueue_transitions_match_cpp() {
        assert_eq!(
            on_unqueued(BufferState::QueuedPending),
            Ok(BufferState::FreePending)
        );
        assert_eq!(
            on_unqueued(BufferState::QueuedReady),
            Ok(BufferState::FreeReady)
        );
        assert_eq!(
            on_unqueued(BufferState::QueuedNoPrefetch),
            Ok(BufferState::QueuedNoPrefetch)
        );
        assert_eq!(
            on_unqueued(BufferState::FreeReady),
            Err(StreamError::UnqueueFromBadState)
        );
        assert_eq!(
            on_unqueued(BufferState::FreePending),
            Err(StreamError::UnqueueFromBadState)
        );
    }

    #[test]
    fn completion_transitions_match_cpp() {
        assert_eq!(
            on_completed(BufferState::QueuedPending),
            Ok(BufferState::QueuedReady)
        );
        assert_eq!(
            on_completed(BufferState::FreePending),
            Ok(BufferState::FreeReady)
        );
        assert_eq!(
            on_completed(BufferState::QueuedReady),
            Err(StreamError::CompletionFromBadState)
        );
        assert_eq!(
            on_completed(BufferState::QueuedNoPrefetch),
            Err(StreamError::CompletionFromBadState)
        );
    }

    #[test]
    fn refill_state_respects_terminal_and_request_result() {
        assert_eq!(after_refill_queue(true, false), BufferState::QueuedPending);
        assert_eq!(
            after_refill_queue(false, false),
            BufferState::QueuedNoPrefetch
        );
        assert_eq!(
            after_refill_queue(true, true),
            BufferState::QueuedNoPrefetch
        );
        assert_eq!(
            after_refill_queue(false, true),
            BufferState::QueuedNoPrefetch
        );
    }

    #[test]
    fn full_pass_refills_free_ready_and_issues_requests() {
        let mut s = stream([
            BufferState::FreeReady,
            BufferState::QueuedNoPrefetch,
            BufferState::QueuedNoPrefetch,
        ]);
        let mut queued = Vec::new();
        let mut requested = Vec::new();
        tick(
            &mut s,
            0,
            || unreachable!(),
            || Ok(None),
            |i| {
                queued.push(i);
                Ok(())
            },
            |d| {
                requested.push(d);
                Ok(true)
            },
        )
        .unwrap();
        assert_eq!(queued, vec![0]);
        assert_eq!(requested, vec![0x100]);
        assert_eq!(s.state[0], BufferState::QueuedPending);
    }

    #[test]
    fn terminal_completion_stops_prefetch_and_clears_alive() {
        let mut s = stream([
            BufferState::FreePending,
            BufferState::QueuedNoPrefetch,
            BufferState::QueuedNoPrefetch,
        ]);
        let mut done = vec![(0x100usize, true)].into_iter();
        let mut queued = Vec::new();
        let mut requests = 0;
        tick(
            &mut s,
            0,
            || unreachable!(),
            || Ok(done.next()),
            |i| {
                queued.push(i);
                Ok(())
            },
            |_| {
                requests += 1;
                Ok(true)
            },
        )
        .unwrap();
        assert!(!s.alive, "terminal completion clears Alive");
        assert_eq!(requests, 0, "no prefetch once terminal is observed");
        assert_eq!(s.state[0], BufferState::QueuedNoPrefetch);
        assert_eq!(queued, vec![0]);
    }

    #[test]
    fn dead_stream_treated_as_terminal_from_start() {
        let mut s = stream([
            BufferState::FreeReady,
            BufferState::QueuedNoPrefetch,
            BufferState::QueuedNoPrefetch,
        ]);
        s.alive = false;
        let mut requests = 0;
        tick(
            &mut s,
            0,
            || unreachable!(),
            || Ok(None),
            |_| Ok(()),
            |_| {
                requests += 1;
                Ok(true)
            },
        )
        .unwrap();
        assert_eq!(
            requests, 0,
            "!Alive at entry means TerminalObserved is true"
        );
    }

    #[test]
    fn reset_plays_once_then_clears() {
        let mut s = stream([
            BufferState::QueuedNoPrefetch,
            BufferState::QueuedNoPrefetch,
            BufferState::QueuedNoPrefetch,
        ]);
        s.reset = true;
        let first = tick(
            &mut s,
            0,
            || unreachable!(),
            || Ok(None),
            |_| Ok(()),
            |_| Ok(true),
        )
        .unwrap();
        assert!(first.played);
        assert!(!s.reset);
        let second = tick(
            &mut s,
            0,
            || unreachable!(),
            || Ok(None),
            |_| Ok(()),
            |_| Ok(true),
        )
        .unwrap();
        assert!(!second.played);
    }

    #[test]
    fn unknown_unqueued_buffer_is_an_error() {
        let mut s = stream([
            BufferState::QueuedPending,
            BufferState::QueuedPending,
            BufferState::QueuedPending,
        ]);
        let err = tick(&mut s, 1, || Ok(999), || Ok(None), |_| Ok(()), |_| Ok(true)).unwrap_err();
        assert_eq!(err, TickError::Stream(StreamError::UnqueuedBufferNotFound));
    }

    #[test]
    fn ambiguous_completion_destination_is_an_error() {
        let mut s = stream([
            BufferState::FreePending,
            BufferState::FreePending,
            BufferState::FreePending,
        ]);
        s.refill = [0x100, 0x100, 0x300];
        let mut done = vec![(0x100usize, false)].into_iter();
        let err = tick(
            &mut s,
            0,
            || unreachable!(),
            || Ok(done.next()),
            |_| Ok(()),
            |_| Ok(true),
        )
        .unwrap_err();
        assert_eq!(err, TickError::Stream(StreamError::CompletionMatchCount(2)));
    }

    #[test]
    fn host_failure_during_tick_is_propagated() {
        let mut s = stream([
            BufferState::FreeReady,
            BufferState::QueuedNoPrefetch,
            BufferState::QueuedNoPrefetch,
        ]);
        let err = tick(
            &mut s,
            0,
            || unreachable!(),
            || Ok(None),
            |_| Err(HostFailed),
            |_| Ok(true),
        )
        .unwrap_err();
        assert_eq!(err, TickError::Host(HostFailed));
    }
}
