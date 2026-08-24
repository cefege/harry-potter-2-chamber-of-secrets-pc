//! Slot/ring/completion bookkeeping mirroring `FFileStream` streaming.
//!
//! Single-threaded deterministic port of the first-party stream manager
//! (`UnFileStream.cpp`: `ReadStreamLocked` dispatch, three ring buffers per
//! stream per `ALAudioSubsystem.h MAX_BUFFERS_PER_STREAM`). The async decode
//! worker of the original becomes an explicit synchronous pull: every chunk
//! "request" completes immediately in FIFO order, so determinism is
//! structural rather than scheduler-dependent.
//!
//! Geometry and conventions:
//! - One [`StreamManager`] owns `capacity` stream slots; ids are dense
//!   (`lowest free index`, mirroring first-free-slot reuse after destroy).
//! - Each stream owns [`BUFFERS_PER_STREAM`] ring slots reused round-robin;
//!   `Completion.slot` identifies which ring slot landed/completed.
//! - Open primes up to `min(initial_chunks, BUFFERS_PER_STREAM)` chunks and
//!   feeds them to the sink immediately (device pre-buffer); those chunks are
//!   marked fed so a later drain completes them without re-feeding PCM.
//! - Chunks queued via [`StreamManager::request_chunk`] reach the sink only
//!   through [`StreamManager::drain`], which walks the FIFO up to and
//!   including the terminal chunk.
//! - Terminal EOF: the first fill reporting `eof` produces a terminal
//!   completion; the NEXT request observes it and returns `Ok(None)`,
//!   emitting [`LifecycleEvent::EndOfStream`] exactly once. Looping sources
//!   never report terminal EOF, so they simply keep cycling ring slots.
//!
//! Failures are loud [`AudioError`]s carrying `audio.*` reason codes;
//! operations on destroyed or unknown ids reject with
//! `AudioError::StreamNotAlive`, exhaustion of the slot table with
//! `AudioError::SlotExhausted`. Nothing here spawns threads or reads clocks.

use crate::error::AudioError;
use crate::graph::{LifecycleEvent, OutputGraph};
use crate::source::PcmSource;

/// Dense stream identifier handed out by [`StreamManager::open`].
pub type StreamId = usize;

/// One landed chunk: which ring slot it occupies, how many interleaved
/// samples it holds, and whether it ends the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Completion {
    /// Ring slot index (`0..BUFFERS_PER_STREAM`) the chunk landed in.
    pub slot: usize,
    /// Interleaved i16 samples actually filled.
    pub samples: usize,
    /// True once this chunk's fill reported terminal EOF.
    pub terminal: bool,
}

/// One reusable ring slot of a stream.
struct RingSlot {
    /// Sample buffer, sized to the stream kind's chunk sample count; reused
    /// across fills (round-robin overwrite mirrors the C++ ring).
    samples: Vec<i16>,
    /// Valid interleaved samples in `samples`.
    filled: usize,
    /// This chunk's fill reported terminal EOF.
    terminal: bool,
    /// Sitting in the FIFO awaiting drain.
    queued: bool,
    /// PCM already handed to the sink (primed chunks are pre-buffered).
    fed_to_sink: bool,
}

impl RingSlot {
    fn new(capacity_samples: usize) -> Self {
        Self {
            samples: vec![0; capacity_samples],
            filled: 0,
            terminal: false,
            queued: false,
            fed_to_sink: false,
        }
    }
}

/// Per-id state of one opened stream.
struct StreamSlot {
    source: Box<dyn PcmSource>,
    ring: Vec<RingSlot>,
    /// Monotonic fill cursor; ring index is `next_fill % ring.len()`.
    next_fill: usize,
    /// Monotonic drain/playback cursor (FIFO behind `next_fill`).
    next_play: usize,
    /// A terminal completion was already returned to the caller.
    terminal_landed: bool,
    /// `EndOfStream` was emitted (once per stream).
    eos_emitted: bool,
    /// Lifetime count of chunks completed through drain (fed + counted),
    /// reported by `StreamDestroyed`.
    drained_total: usize,
}

impl StreamSlot {
    fn new(source: Box<dyn PcmSource>) -> Self {
        let capacity_samples = source.kind().chunk_samples();
        Self {
            source,
            ring: (0..crate::source::BUFFERS_PER_STREAM)
                .map(|_| RingSlot::new(capacity_samples))
                .collect(),
            next_fill: 0,
            next_play: 0,
            terminal_landed: false,
            eos_emitted: false,
            drained_total: 0,
        }
    }

    /// First-party `IsStreamAlive`: a slot counts as alive while it is open
    /// AND has not landed its terminal chunk (`EndOfFile` clears liveness).
    fn alive(slots: &[Option<StreamSlot>], id: StreamId) -> bool {
        matches!(slots.get(id), Some(Some(s)) if !s.terminal_landed)
    }

    fn borrow(
        slots: &mut [Option<StreamSlot>],
        id: StreamId,
    ) -> Result<&mut StreamSlot, AudioError> {
        slots
            .get_mut(id)
            .and_then(|s| s.as_mut())
            .ok_or(AudioError::StreamNotAlive { id })
    }
}

/// Deterministic single-threaded port of the FFileStream stream table.
pub struct StreamManager {
    slots: Vec<Option<StreamSlot>>,
}

impl StreamManager {
    /// Creates a manager with room for `capacity` streams. A sub-one
    /// capacity is clamped to 1 (with a debug assertion) so headless callers
    /// can pass a config value unchecked without a silent zero-capacity trap.
    pub fn new(capacity: usize) -> Self {
        debug_assert!(capacity >= 1, "StreamManager capacity must be >= 1");
        let capacity = capacity.max(1);
        Self {
            slots: (0..capacity).map(|_| None).collect(),
        }
    }

    /// Opens `source`, primes `min(initial_chunks, BUFFERS_PER_STREAM)`
    /// chunks synchronously through `source.fill`, attaches its sink, and
    /// emits the open event sequence. Slot selection is the lowest free
    /// index (first-free-slot reuse). Primed chunks are pre-buffered straight
    /// to the sink and marked fed; a later drain completes them without
    /// re-feeding. A priming failure tears the half-open stream back down
    /// (sink detached) and surfaces the decoder error loudly; the partial
    /// open history stays in the trace.
    pub fn open(
        &mut self,
        graph: &mut OutputGraph,
        source: Box<dyn PcmSource>,
        initial_chunks: usize,
    ) -> Result<StreamId, AudioError> {
        let id = self
            .slots
            .iter()
            .position(|s| s.is_none())
            .ok_or(AudioError::SlotExhausted)?;
        let kind = source.kind();
        let info = source.info();

        let mut slot = StreamSlot::new(source);
        graph.record(LifecycleEvent::StreamOpened { id, kind, info });
        graph.attach_sink(id, info);

        for _ in 0..initial_chunks.min(crate::source::BUFFERS_PER_STREAM) {
            match Self::prime(&mut slot, graph, id) {
                Ok(true) => break, // terminal chunk landed; stop priming
                Ok(false) => {}
                Err(err) => {
                    graph.detach_sink(id);
                    return Err(err);
                }
            }
        }

        self.slots[id] = Some(slot);
        Ok(id)
    }

    /// Primes one chunk into the next ring slot, feeding it to the sink
    /// immediately. Returns `Ok(true)` when the chunk was terminal.
    fn prime(
        slot: &mut StreamSlot,
        graph: &mut OutputGraph,
        id: StreamId,
    ) -> Result<bool, AudioError> {
        let s = slot.next_fill % slot.ring.len();
        let fill = {
            let buf = &mut slot.ring[s].samples;
            slot.source.fill(buf)?
        };
        let entry = &mut slot.ring[s];
        entry.filled = fill.written;
        entry.terminal = fill.eof;
        entry.queued = true;
        entry.fed_to_sink = false;
        graph.feed_chunk(id, &entry.samples[..fill.written]);
        entry.fed_to_sink = true;
        graph.record(LifecycleEvent::ChunkPrimed {
            id,
            slot: s,
            samples: fill.written,
        });
        slot.next_fill += 1;
        Ok(fill.eof)
    }

    /// Fills the next ring slot from the source (FIFO round-robin order,
    /// buffers reused). Returns `Ok(Some(completion))` when a chunk landed
    /// and `Ok(None)` once the stream hit terminal EOF — the first such call
    /// emits [`LifecycleEvent::EndOfStream`] exactly once, later calls keep
    /// returning `Ok(None)` quietly. Unknown or destroyed ids reject with
    /// `AudioError::StreamNotAlive`.
    pub fn request_chunk(
        &mut self,
        graph: &mut OutputGraph,
        id: StreamId,
    ) -> Result<Option<Completion>, AudioError> {
        let slot = StreamSlot::borrow(&mut self.slots, id)?;
        if slot.terminal_landed {
            if !slot.eos_emitted {
                graph.record(LifecycleEvent::EndOfStream { id });
                slot.eos_emitted = true;
            }
            return Ok(None);
        }

        let s = slot.next_fill % slot.ring.len();
        let fill = {
            let buf = &mut slot.ring[s].samples;
            slot.source.fill(buf)?
        };
        let entry = &mut slot.ring[s];
        entry.filled = fill.written;
        entry.terminal = fill.eof;
        entry.queued = true;
        entry.fed_to_sink = false;
        graph.record(LifecycleEvent::ChunkQueued { id, slot: s });
        slot.next_fill += 1;
        if fill.eof {
            slot.terminal_landed = true;
        }
        Ok(Some(Completion {
            slot: s,
            samples: fill.written,
            terminal: fill.eof,
        }))
    }

    /// Drains queued chunks in FIFO order up to and including the terminal
    /// one, feeding each not-yet-fed chunk's PCM to the sink and recording
    /// [`LifecycleEvent::ChunkCompleted`] for every drained chunk. Returns
    /// the number of chunks drained (primed chunks count here too — they are
    /// completed even though their PCM was pre-buffered at open).
    pub fn drain(&mut self, graph: &mut OutputGraph, id: StreamId) -> Result<usize, AudioError> {
        let slot = StreamSlot::borrow(&mut self.slots, id)?;
        let ring_len = slot.ring.len();
        let mut drained = 0usize;

        loop {
            let p = slot.next_play % ring_len;
            if !slot.ring[p].queued {
                break;
            }
            if !slot.ring[p].fed_to_sink {
                let samples = &slot.ring[p].samples[..slot.ring[p].filled];
                graph.feed_chunk(id, samples);
            }
            let (samples, terminal) = (slot.ring[p].filled, slot.ring[p].terminal);
            slot.ring[p].fed_to_sink = true;
            slot.ring[p].queued = false;
            slot.next_play += 1;
            drained += 1;
            slot.drained_total += 1;
            graph.record(LifecycleEvent::ChunkCompleted {
                id,
                slot: p,
                samples,
                terminal,
            });
            if terminal {
                break;
            }
        }
        Ok(drained)
    }

    /// Records [`LifecycleEvent::PlaybackStarted`]. Rejects dead ids.
    pub fn start_playback(
        &mut self,
        graph: &mut OutputGraph,
        id: StreamId,
    ) -> Result<(), AudioError> {
        StreamSlot::borrow(&mut self.slots, id)?;
        graph.record(LifecycleEvent::PlaybackStarted { id });
        Ok(())
    }

    /// Records [`LifecycleEvent::PlaybackStopped`]. Rejects dead ids.
    pub fn stop_playback(
        &mut self,
        graph: &mut OutputGraph,
        id: StreamId,
    ) -> Result<(), AudioError> {
        StreamSlot::borrow(&mut self.slots, id)?;
        graph.record(LifecycleEvent::PlaybackStopped { id });
        Ok(())
    }

    /// Whether stream `id` is live: opened, not destroyed, and not yet at
    /// terminal EOF (looping streams stay alive; a landed terminal chunk
    /// mirrors the first-party `EndOfFile` clearing `IsStreamAlive`).
    pub fn is_alive(&self, id: StreamId) -> bool {
        StreamSlot::alive(&self.slots, id)
    }

    /// Number of occupied slots.
    pub fn active(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }

    /// Destroys stream `id`: drops it WITHOUT draining (mirroring
    /// `DestroyStream`), detaches its sink, and emits
    /// [`LifecycleEvent::StreamDestroyed`] carrying the lifetime drained
    /// chunk count. The freed slot is immediately reusable. Unknown or
    /// already-destroyed ids are a documented no-op (idempotent destroy).
    pub fn destroy(&mut self, graph: &mut OutputGraph, id: StreamId) {
        if let Some(slot) = self.slots.get_mut(id).and_then(|s| s.take()) {
            let drained_chunks = slot.drained_total;
            graph.detach_sink(id);
            graph.record(LifecycleEvent::StreamDestroyed { id, drained_chunks });
        }
    }

    /// Destroys every active stream (sink detached, `StreamDestroyed`
    /// emitted per stream, ascending id order) then records
    /// [`LifecycleEvent::ShutdownCompleted`]. Returns the number of streams
    /// destroyed.
    pub fn shutdown(self, graph: &mut OutputGraph) -> usize {
        // Consuming `self` drops every source when the manager goes out of
        // scope; here we only need ids and drained counts to emit events.
        let live: Vec<(StreamId, usize)> = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(id, slot)| slot.as_ref().map(|s| (id, s.drained_total)))
            .collect();
        for (id, drained_chunks) in &live {
            graph.detach_sink(*id);
            graph.record(LifecycleEvent::StreamDestroyed {
                id: *id,
                drained_chunks: *drained_chunks,
            });
        }
        graph.record(LifecycleEvent::ShutdownCompleted {
            streams_destroyed: live.len(),
        });
        live.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::AudioSink;
    use crate::source::{Fill, StreamInfo, StreamKind};
    use std::sync::atomic::{AtomicUsize, Ordering};

    const INFO: StreamInfo = StreamInfo {
        channels: 2,
        sample_rate: 22050,
    };

    /// Samples per chunk for the manager-sized ring of an `ST_XA` stream.
    const CHUNK_SAMPLES: usize = crate::source::XA_CHUNK_BYTES / 2;

    /// Deterministic fake source: yields `total_chunks` full patterned
    /// chunks, then zero-fills with `eof = true` forever.
    #[derive(Debug)]
    struct FakeSource {
        kind: StreamKind,
        total_chunks: usize,
        produced: usize,
    }

    impl FakeSource {
        fn finite(total_chunks: usize) -> Box<Self> {
            Box::new(Self {
                kind: StreamKind::Xa,
                total_chunks,
                produced: 0,
            })
        }

        fn endless() -> Box<Self> {
            Self::finite(usize::MAX)
        }
    }

    impl PcmSource for FakeSource {
        fn kind(&self) -> StreamKind {
            self.kind
        }

        fn info(&self) -> StreamInfo {
            INFO
        }

        fn fill(&mut self, out: &mut [i16]) -> Result<Fill, AudioError> {
            if self.produced >= self.total_chunks {
                out.fill(0);
                return Ok(Fill {
                    written: out.len(),
                    eof: true,
                });
            }
            self.produced += 1;
            let chunk = self.produced as i32;
            for (i, s) in out.iter_mut().enumerate() {
                *s = ((chunk * 1009 + i as i32) % 29_998 - 14_999) as i16;
            }
            Ok(Fill {
                written: out.len(),
                eof: false,
            })
        }
    }

    /// Per-test sink counters: each counting graph owns its own atomics, so
    /// tests stay independent and deterministic under parallel execution.
    struct CountingSink {
        channels: u16,
        writes: std::sync::Arc<AtomicUsize>,
        frames: std::sync::Arc<AtomicUsize>,
    }

    impl AudioSink for CountingSink {
        fn attach(&mut self, _id: usize, info: StreamInfo) {
            self.channels = info.channels.max(1);
        }
        fn write(&mut self, _id: usize, samples: &[i16]) {
            self.writes.fetch_add(1, Ordering::SeqCst);
            self.frames
                .fetch_add(samples.len() / self.channels as usize, Ordering::SeqCst);
        }
        fn detach(&mut self, _id: usize) {}
    }

    /// Builds a counting-sink graph plus handles to its shared tallies.
    fn counting_graph() -> (
        OutputGraph,
        std::sync::Arc<AtomicUsize>,
        std::sync::Arc<AtomicUsize>,
    ) {
        let writes = std::sync::Arc::new(AtomicUsize::new(0));
        let frames = std::sync::Arc::new(AtomicUsize::new(0));
        let (w, f) = (writes.clone(), frames.clone());
        let graph = OutputGraph::with_sink_factory(std::sync::Arc::new(move |_info| {
            Box::new(CountingSink {
                channels: 2,
                writes: w.clone(),
                frames: f.clone(),
            })
        }));
        (graph, writes, frames)
    }

    #[test]
    fn full_lifecycle_matches_expected_trace_verbatim() {
        let mut graph = OutputGraph::new();
        let mut mgr = StreamManager::new(4);

        let id = mgr
            .open(&mut graph, FakeSource::finite(2), 2)
            .expect("open");
        assert_eq!(id, 0);
        assert_eq!(mgr.active(), 1);

        let c1 = mgr.request_chunk(&mut graph, id).expect("chunk 3");
        assert_eq!(
            c1,
            Some(Completion {
                slot: 2,
                samples: CHUNK_SAMPLES,
                terminal: true
            })
        );
        let c2 = mgr.request_chunk(&mut graph, id).expect("post-eof");
        assert_eq!(c2, None);
        let c3 = mgr.request_chunk(&mut graph, id).expect("still quiet");
        assert_eq!(c3, None); // EndOfStream emitted exactly once

        mgr.start_playback(&mut graph, id).expect("play");
        assert_eq!(mgr.drain(&mut graph, id).expect("drain"), 3);
        mgr.stop_playback(&mut graph, id).expect("stop");
        mgr.destroy(&mut graph, id);
        assert!(!mgr.is_alive(id));
        assert_eq!(mgr.active(), 0);
        assert_eq!(
            graph.trace(),
            &[
                LifecycleEvent::GraphOpened,
                LifecycleEvent::StreamOpened {
                    id: 0,
                    kind: StreamKind::Xa,
                    info: INFO
                },
                LifecycleEvent::SinkAttached { id: 0 },
                LifecycleEvent::ChunkPrimed {
                    id: 0,
                    slot: 0,
                    samples: CHUNK_SAMPLES
                },
                LifecycleEvent::ChunkPrimed {
                    id: 0,
                    slot: 1,
                    samples: CHUNK_SAMPLES
                },
                LifecycleEvent::ChunkQueued { id: 0, slot: 2 },
                LifecycleEvent::EndOfStream { id: 0 },
                LifecycleEvent::PlaybackStarted { id: 0 },
                LifecycleEvent::ChunkCompleted {
                    id: 0,
                    slot: 0,
                    samples: CHUNK_SAMPLES,
                    terminal: false
                },
                LifecycleEvent::ChunkCompleted {
                    id: 0,
                    slot: 1,
                    samples: CHUNK_SAMPLES,
                    terminal: false
                },
                LifecycleEvent::ChunkCompleted {
                    id: 0,
                    slot: 2,
                    samples: CHUNK_SAMPLES,
                    terminal: true
                },
                LifecycleEvent::PlaybackStopped { id: 0 },
                LifecycleEvent::SinkDetached { id: 0 },
                LifecycleEvent::StreamDestroyed {
                    id: 0,
                    drained_chunks: 3
                },
            ]
        );
    }

    #[test]
    fn drain_feeds_sink_and_frames_written_tracks_pcm() {
        let (mut graph, writes, frames) = counting_graph();
        let mut mgr = StreamManager::new(2);

        let id = mgr
            .open(&mut graph, FakeSource::finite(4), 0)
            .expect("open");
        assert_eq!(id, 0);
        mgr.request_chunk(&mut graph, id).expect("chunk 0");
        mgr.request_chunk(&mut graph, id).expect("chunk 1");
        let drained = mgr.drain(&mut graph, id).expect("drain");
        assert_eq!(drained, 2);
        assert_eq!(writes.load(Ordering::SeqCst), 2);
        assert_eq!(frames.load(Ordering::SeqCst), 2 * CHUNK_SAMPLES / 2);
        assert_eq!(graph.frames_written(0), (2 * CHUNK_SAMPLES / 2) as u64);

        assert_eq!(
            graph.trace(),
            &[
                LifecycleEvent::GraphOpened,
                LifecycleEvent::StreamOpened {
                    id: 0,
                    kind: StreamKind::Xa,
                    info: INFO
                },
                LifecycleEvent::SinkAttached { id: 0 },
                LifecycleEvent::ChunkQueued { id: 0, slot: 0 },
                LifecycleEvent::ChunkQueued { id: 0, slot: 1 },
                LifecycleEvent::ChunkCompleted {
                    id: 0,
                    slot: 0,
                    samples: CHUNK_SAMPLES,
                    terminal: false
                },
                LifecycleEvent::ChunkCompleted {
                    id: 0,
                    slot: 1,
                    samples: CHUNK_SAMPLES,
                    terminal: false
                },
            ]
        );
    }
    #[test]
    fn priming_clamps_to_ring_buffers_and_feeds_sink_once() {
        let (mut graph, writes, _frames) = counting_graph();
        let mut mgr = StreamManager::new(1);

        mgr.open(&mut graph, FakeSource::endless(), 10)
            .expect("open");
        let primed = graph
            .trace()
            .iter()
            .filter(|e| matches!(e, LifecycleEvent::ChunkPrimed { .. }))
            .count();
        assert_eq!(primed, crate::source::BUFFERS_PER_STREAM);
        // Pre-buffered exactly once: three primed chunks, three writes.
        assert_eq!(writes.load(Ordering::SeqCst), 3);
        // Draining the primed chunks completes them without re-feeding.
        assert_eq!(mgr.drain(&mut graph, 0).expect("drain"), 3);
        assert_eq!(writes.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn destroyed_slot_is_reusable_via_first_free_index() {
        let mut graph = OutputGraph::new();
        let mut mgr = StreamManager::new(3);

        assert_eq!(
            mgr.open(&mut graph, FakeSource::endless(), 0).expect("a"),
            0
        );
        assert_eq!(
            mgr.open(&mut graph, FakeSource::endless(), 0).expect("b"),
            1
        );
        assert!(mgr.is_alive(1));
        mgr.destroy(&mut graph, 1);
        assert!(!mgr.is_alive(1));
        assert_eq!(
            mgr.open(&mut graph, FakeSource::endless(), 0)
                .expect("reuse"),
            1,
            "freed lowest slot must be handed out again"
        );
        assert_eq!(
            mgr.open(&mut graph, FakeSource::endless(), 0)
                .expect("last slot"),
            2
        );
        assert_eq!(mgr.active(), 3);
    }

    #[test]
    fn loud_errors_for_dead_ids_and_exhaustion() {
        let mut graph = OutputGraph::new();
        let mut mgr = StreamManager::new(1);

        // Unknown id before any open.
        assert_eq!(
            mgr.request_chunk(&mut graph, 0),
            Err(AudioError::StreamNotAlive { id: 0 })
        );

        mgr.open(&mut graph, FakeSource::endless(), 0)
            .expect("open");
        assert_eq!(
            mgr.open(&mut graph, FakeSource::endless(), 0),
            Err(AudioError::SlotExhausted)
        );
        assert_eq!(
            mgr.request_chunk(&mut graph, 5),
            Err(AudioError::StreamNotAlive { id: 5 })
        );
        assert_eq!(mgr.drain(&mut graph, 0).err(), None); // alive stream drains fine

        mgr.destroy(&mut graph, 0);
        assert_eq!(
            mgr.start_playback(&mut graph, 0),
            Err(AudioError::StreamNotAlive { id: 0 })
        );
        assert_eq!(
            mgr.stop_playback(&mut graph, 0),
            Err(AudioError::StreamNotAlive { id: 0 })
        );
        assert_eq!(
            mgr.request_chunk(&mut graph, 0),
            Err(AudioError::StreamNotAlive { id: 0 })
        );
        assert_eq!(
            mgr.request_chunk(&mut graph, 0).unwrap_err().reason_code(),
            "audio.stream_not_alive"
        );
        // Destroy freed the only slot, so a reopen reuses id 0 and only the
        // NEXT open exhausts.
        assert_eq!(
            mgr.open(&mut graph, FakeSource::endless(), 0)
                .expect("slot was freed"),
            0
        );
        let err = mgr
            .open(&mut graph, FakeSource::endless(), 0)
            .expect_err("capacity 1 is full again");
        assert_eq!(err.reason_code(), "audio.slot_exhausted");
        // Idempotent destroy of a dead id stays silent.
        mgr.destroy(&mut graph, 0);
        assert_eq!(mgr.active(), 0);
    }

    #[test]
    fn endless_source_wraps_ring_without_end_of_stream() {
        let mut graph = OutputGraph::new();
        let mut mgr = StreamManager::new(2);

        let id = mgr
            .open(&mut graph, FakeSource::endless(), 0)
            .expect("open");
        let mut slots = Vec::new();
        for _ in 0..5 {
            let completion = mgr
                .request_chunk(&mut graph, id)
                .expect("chunk")
                .expect("never terminal");
            slots.push(completion.slot);
        }
        assert_eq!(slots, vec![0, 1, 2, 0, 1]);
        assert!(mgr.is_alive(id));
        assert!(
            !graph
                .trace()
                .iter()
                .any(|e| matches!(e, LifecycleEvent::EndOfStream { .. }))
        );
    }

    #[test]
    fn shutdown_destroys_all_streams_in_id_order() {
        let mut graph = OutputGraph::new();
        let mut mgr = StreamManager::new(4);

        mgr.open(&mut graph, FakeSource::endless(), 0).expect("a");
        mgr.open(&mut graph, FakeSource::endless(), 0).expect("b");

        let destroyed = mgr.shutdown(&mut graph);
        assert_eq!(destroyed, 2);
        assert_eq!(
            &graph.trace()[graph.trace().len() - 6..],
            &[
                LifecycleEvent::SinkAttached { id: 1 },
                LifecycleEvent::SinkDetached { id: 0 },
                LifecycleEvent::StreamDestroyed {
                    id: 0,
                    drained_chunks: 0
                },
                LifecycleEvent::SinkDetached { id: 1 },
                LifecycleEvent::StreamDestroyed {
                    id: 1,
                    drained_chunks: 0
                },
                LifecycleEvent::ShutdownCompleted {
                    streams_destroyed: 2
                },
            ]
        );
    }
}
