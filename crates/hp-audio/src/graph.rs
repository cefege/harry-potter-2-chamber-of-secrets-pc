//! Output graph with injectable sinks and a deterministic lifecycle trace.
//!
//! Mirrors the first-party ALAudio output path (`ALAudioSubsystem.h`): every
//! stream lifecycle transition is recorded as a [`LifecycleEvent`] so the
//! whole pipeline can be replayed headlessly and compared trace-for-trace
//! between two runs (the determinism gate for the Phase 5 slice). Sinks are
//! injectable through a factory: [`NullSink`] counts frames and discards PCM
//! (headless default), [`RodioSink`] appends converted chunks to a real
//! `rodio::Player`.
//!
//! The manager in [`crate::stream`] drives sink attachment, chunk feeding,
//! and detachment; the graph itself only records events and forwards PCM.
//! Nothing here allocates audio threads or depends on wall-clock time.

use std::sync::Arc;

use crate::source::StreamInfo;

/// Deterministic lifecycle event log entry. `PartialEq` + `Clone` + `Debug`
/// derives are REQUIRED: double-run trace comparison is the M3 gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleEvent {
    /// Graph constructed.
    GraphOpened,
    /// Stream slot occupied; sink attached.
    StreamOpened {
        id: usize,
        kind: crate::source::StreamKind,
        info: StreamInfo,
    },
    /// Chunk primed into ring `slot` during open; fed to the sink immediately
    /// (pre-buffer), samples = actual filled count.
    ChunkPrimed {
        id: usize,
        slot: usize,
        samples: usize,
    },
    /// Playback started on this stream.
    PlaybackStarted { id: usize },
    /// Requested chunk landed in ring `slot` (round-robin reuse).
    ChunkQueued { id: usize, slot: usize },
    /// Queued chunk handed to the sink during drain.
    ChunkCompleted {
        id: usize,
        slot: usize,
        samples: usize,
        terminal: bool,
    },
    /// Terminal EOF observed (emitted exactly once per stream).
    EndOfStream { id: usize },
    /// Playback stopped on this stream.
    PlaybackStopped { id: usize },
    /// Stream destroyed; `drained_chunks` = chunks fed to the sink over its
    /// lifetime.
    StreamDestroyed { id: usize, drained_chunks: usize },
    /// Sink instance attached for this stream id.
    SinkAttached { id: usize },
    /// Sink instance detached for this stream id.
    SinkDetached { id: usize },
    /// Manager shut down after destroying all active streams.
    ShutdownCompleted { streams_destroyed: usize },
}

/// Injectable audio consumer. Implementations: [`NullSink`] (headless
/// default), [`RodioSink`] (real device, best-effort).
pub trait AudioSink: Send {
    /// A stream with this id and metadata begins feeding.
    fn attach(&mut self, id: usize, info: StreamInfo);
    /// Receives every completed chunk's PCM (interleaved i16).
    fn write(&mut self, id: usize, samples: &[i16]);
    /// The stream with this id stops feeding.
    fn detach(&mut self, id: usize);
}

/// Headless sink: counts interleaved frames and drops them. Default for
/// [`OutputGraph::new`].
#[derive(Debug, Clone)]
pub struct NullSink {
    frames: u64,
    channels: u16,
}

impl NullSink {
    pub fn new() -> Self {
        Self {
            frames: 0,
            channels: 1,
        }
    }

    /// Frames observed so far (samples.len() / channels per write).
    pub fn frames(&self) -> u64 {
        self.frames
    }
}

impl Default for NullSink {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioSink for NullSink {
    fn attach(&mut self, _id: usize, info: StreamInfo) {
        self.channels = info.channels.max(1);
    }

    fn write(&mut self, _id: usize, samples: &[i16]) {
        self.frames += (samples.len() / self.channels as usize) as u64;
    }

    fn detach(&mut self, _id: usize) {}
}

/// Real-device sink: converts i16 -> f32 (/32768) and appends each completed
/// chunk to a `rodio::Player`. Construction never fails — `Player::new()`
/// needs no audio device and nothing pulls until the player is connected to
/// an output stream by the real app wiring.
pub struct RodioSink {
    player: rodio::Player,
    channels: u16,
    sample_rate: u32,
}

impl RodioSink {
    pub fn new() -> Self {
        // Player::new hands back the queue consumer side; nothing is connected
        // so dropping it is silent and safe.
        let (player, _queue_output) = rodio::Player::new();
        Self {
            player,
            channels: 1,
            sample_rate: 44100,
        }
    }
}

impl Default for RodioSink {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioSink for RodioSink {
    fn attach(&mut self, _id: usize, info: StreamInfo) {
        self.channels = info.channels.max(1);
        self.sample_rate = info.sample_rate.max(1);
    }

    fn write(&mut self, _id: usize, samples: &[i16]) {
        // libvorbis ov_read(word=2) parity: signed 16-bit -> normalized float.
        let data: Vec<f32> = samples.iter().map(|&s| f32::from(s) / 32768.0).collect();
        let channels =
            rodio::ChannelCount::try_from(self.channels).unwrap_or(rodio::ChannelCount::MIN);
        let sample_rate = rodio::SampleRate::try_from(self.sample_rate)
            .unwrap_or_else(|_| rodio::SampleRate::new(44100).expect("nonzero"));
        self.player.append(rodio::buffer::SamplesBuffer::new(
            channels,
            sample_rate,
            data,
        ));
    }

    fn detach(&mut self, _id: usize) {}
}

struct GraphSlot {
    info: Option<StreamInfo>,
    attached: bool,
    frames: u64,
    sink: Option<Box<dyn AudioSink>>,
}

/// Injectable sink constructor: one sink per opened stream, keyed by its
/// decoder metadata.
pub type SinkFactory = Arc<dyn Fn(StreamInfo) -> Box<dyn AudioSink> + Send + Sync>;

/// Trace-keeping output graph holding one sink slot per stream id. Stream ids
/// assigned by [`crate::stream::StreamManager`] index the slot vector
/// directly (ids are dense, first-free-slot).
pub struct OutputGraph {
    trace: Vec<LifecycleEvent>,
    factory: Option<SinkFactory>,
    slots: Vec<GraphSlot>,
}

impl std::fmt::Debug for OutputGraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OutputGraph")
            .field("trace", &self.trace)
            .field("slots", &self.slots.len())
            .finish()
    }
}

impl OutputGraph {
    /// Headless graph: one fresh [`NullSink`] per attached stream.
    pub fn new() -> Self {
        let mut graph = Self {
            trace: Vec::new(),
            factory: None,
            slots: Vec::new(),
        };
        graph.trace.push(LifecycleEvent::GraphOpened);
        graph
    }

    /// Graph whose streams receive sinks built by `factory(info)` at attach
    /// time.
    pub fn with_sink_factory(
        factory: Arc<dyn Fn(StreamInfo) -> Box<dyn AudioSink> + Send + Sync>,
    ) -> Self {
        let mut graph = Self {
            trace: Vec::new(),
            factory: Some(factory),
            slots: Vec::new(),
        };
        graph.trace.push(LifecycleEvent::GraphOpened);
        graph
    }

    /// Recorded lifecycle events, in emission order.
    pub fn trace(&self) -> &[LifecycleEvent] {
        &self.trace
    }

    /// Interleaved frames fed through the sink of stream `id`; 0 if the id
    /// never attached. Mirrors what a [`NullSink`] at that slot would count.
    pub fn frames_written(&self, id: usize) -> u64 {
        self.slots.get(id).map_or(0, |slot| slot.frames)
    }

    /// Records a lifecycle event emitted by the stream manager.
    pub(crate) fn record(&mut self, event: LifecycleEvent) {
        self.trace.push(event);
    }

    /// Attaches (or replaces) the sink for stream `id`, growing the slot
    /// vector as needed. Emits [`LifecycleEvent::SinkAttached`].
    pub(crate) fn attach_sink(&mut self, id: usize, info: StreamInfo) {
        if self.slots.len() <= id {
            self.slots.resize_with(id + 1, || GraphSlot {
                info: None,
                attached: false,
                frames: 0,
                sink: None,
            });
        }
        let mut sink = match &self.factory {
            Some(factory) => factory(info),
            None => Box::new(NullSink::new()),
        };
        sink.attach(id, info);
        let slot = &mut self.slots[id];
        if slot.attached
            && let Some(mut old) = slot.sink.take()
        {
            old.detach(id);
            self.trace.push(LifecycleEvent::SinkDetached { id });
        }
        slot.info = Some(info);
        slot.attached = true;
        slot.sink = Some(sink);
        self.trace.push(LifecycleEvent::SinkAttached { id });
    }

    /// Feeds one chunk of interleaved i16 PCM to the sink of stream `id`;
    /// no-op if the id has no attached sink. Updates [`Self::frames_written`].
    pub(crate) fn feed_chunk(&mut self, id: usize, samples: &[i16]) {
        let channels = match self.slots.get(id).and_then(|s| s.info) {
            Some(info) => info.channels.max(1) as usize,
            None => return,
        };
        self.slots[id].frames += (samples.len() / channels) as u64;
        if let Some(sink) = self.slots[id].sink.as_mut() {
            sink.write(id, samples);
        }
    }

    /// Detaches the sink of stream `id` (kept frame counter survives). Emits
    /// [`LifecycleEvent::SinkDetached`]; no-op if already detached.
    pub(crate) fn detach_sink(&mut self, id: usize) {
        let Some(slot) = self.slots.get_mut(id) else {
            return;
        };
        if !slot.attached {
            return;
        }
        if let Some(mut sink) = slot.sink.take() {
            sink.detach(id);
        }
        slot.attached = false;
        self.trace.push(LifecycleEvent::SinkDetached { id });
    }
}

impl Default for OutputGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const INFO: StreamInfo = StreamInfo {
        channels: 2,
        sample_rate: 22050,
    };

    /// Counting sink built through the factory path; shared tallies live in
    /// atomics so the factory closure stays `Fn`.
    struct CountingSink {
        writes: &'static AtomicUsize,
        samples_seen: &'static AtomicUsize,
        channels: u16,
    }

    impl AudioSink for CountingSink {
        fn attach(&mut self, _id: usize, info: StreamInfo) {
            self.channels = info.channels.max(1);
        }
        fn write(&mut self, _id: usize, samples: &[i16]) {
            self.writes.fetch_add(1, Ordering::SeqCst);
            self.samples_seen.fetch_add(
                samples.len() / self.channels.max(1) as usize,
                Ordering::SeqCst,
            );
        }
        fn detach(&mut self, _id: usize) {}
    }

    static SINK_WRITES: AtomicUsize = AtomicUsize::new(0);
    static SINK_SAMPLES: AtomicUsize = AtomicUsize::new(0);

    fn counting_factory(info: StreamInfo) -> Box<dyn AudioSink> {
        Box::new(CountingSink {
            writes: &SINK_WRITES,
            samples_seen: &SINK_SAMPLES,
            channels: info.channels,
        })
    }

    #[test]
    fn new_graph_opens_with_graph_opened_event() {
        let graph = OutputGraph::new();
        assert_eq!(graph.trace(), &[LifecycleEvent::GraphOpened]);
        assert_eq!(graph.frames_written(0), 0);
    }

    #[test]
    fn attach_feed_detach_records_and_counts_frames() {
        let mut graph = OutputGraph::with_sink_factory(Arc::new(counting_factory));
        assert_eq!(graph.trace(), &[LifecycleEvent::GraphOpened]);

        graph.attach_sink(0, INFO);
        graph.feed_chunk(0, &[1, 2, 3, 4, 5, 6]);
        graph.feed_chunk(0, &[1, 2]);
        graph.detach_sink(0);

        assert_eq!(
            graph.trace(),
            &[
                LifecycleEvent::GraphOpened,
                LifecycleEvent::SinkAttached { id: 0 },
                LifecycleEvent::SinkDetached { id: 0 },
            ]
        );
        // 6/2 + 2/2 = 4 interleaved stereo frames.
        assert_eq!(graph.frames_written(0), 4);
        assert_eq!(SINK_WRITES.load(Ordering::SeqCst), 2);
        assert_eq!(SINK_SAMPLES.load(Ordering::SeqCst), 4);

        // Feeding a detached / unknown id is a silent no-op for the graph
        // surface (manager guards loudness); counters stay put.
        graph.feed_chunk(9, &[1, 2, 3, 4]);
        assert_eq!(graph.frames_written(9), 0);
        assert_eq!(graph.frames_written(0), 4);
    }

    #[test]
    fn reattach_replaces_sink_and_double_run_traces_match() {
        let build = || {
            let mut graph = OutputGraph::new();
            graph.attach_sink(0, INFO);
            graph.attach_sink(0, INFO); // replace
            graph.detach_sink(0);
            graph.detach_sink(0); // idempotent, no second event
            graph.trace().to_vec()
        };
        let first = build();
        let second = build();
        assert_eq!(first, second);
        assert_eq!(
            first,
            vec![
                LifecycleEvent::GraphOpened,
                LifecycleEvent::SinkAttached { id: 0 },
                LifecycleEvent::SinkDetached { id: 0 },
                LifecycleEvent::SinkAttached { id: 0 },
                LifecycleEvent::SinkDetached { id: 0 },
            ]
        );
    }

    #[test]
    fn nullsink_counts_frames_per_channel_layout() {
        let mut sink = NullSink::new();
        sink.attach(0, INFO);
        sink.write(0, &[0; 8]);
        assert_eq!(sink.frames(), 4);
        sink.attach(
            0,
            StreamInfo {
                channels: 1,
                sample_rate: 8000,
            },
        );
        sink.write(0, &[0; 8]);
        assert_eq!(sink.frames(), 12);
    }

    #[test]
    fn rodiosink_converts_and_appends_without_device() {
        // Construction must never fail without an audio device.
        let mut sink = RodioSink::new();
        sink.attach(0, INFO);
        // Full-scale conversion sanity happens inside append; here we just
        // exercise the write path (nothing pulls, so it never plays).
        sink.write(0, &[32767, -32768, 0, 16384]);
        sink.write(0, &[]);
        sink.detach(0);
    }
}
