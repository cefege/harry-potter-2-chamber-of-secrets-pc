//! Phase 5 audio-lifecycle gate: Rust port of `Tests/AudioTests.cpp`
//! (`audio_lifecycle`) scoped to the crate's Ogg/XA stream types.
//!
//! Gate: open/play/stop/shutdown completes cleanly and deterministically —
//! the same event sequence and state transitions every run, no leaked
//! streams or sinks. Determinism is structural (single-threaded pull model),
//! and this suite proves it by running each scenario twice and comparing
//! full lifecycle traces.
//!
//! Headless reality: CI has no speakers, so the output graph runs on
//! counting sinks; the real rodio sink is exercised only for construction
//! and teardown (lifecycle determinism is the gate, not audibility).

use hp_audio::graph::{LifecycleEvent, OutputGraph, RodioSink};
use hp_audio::ogg::OggSource;
use hp_audio::source::{BUFFERS_PER_STREAM, OGG_CHUNK_BYTES, PcmSource, StreamKind};
use hp_audio::stream::StreamManager;
use hp_audio::xa::XaSource;

const STOCK_OGG: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../HarryPotter2/Unreal/Music/sm_bur_PlayfulFail_01.ogg"
);

/// FNV-1a 64-bit over decoded PCM bytes; dependency-free regression tripwire
/// pinning symphonia's Vorbis output for the stock fixture.
fn fnv64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

struct StockOgg {
    bytes: Vec<u8>,
}

impl StockOgg {
    /// `None` when the game data root is absent (fresh clone without the
    /// copyrighted soundtrack): callers print a blocked note and pass.
    fn load() -> Option<StockOgg> {
        match std::fs::read(STOCK_OGG) {
            Ok(bytes) => Some(StockOgg { bytes }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                println!("blocked: stock Ogg fixture absent (no game data root)");
                None
            }
            Err(error) => panic!("stock Ogg fixture unreadable: {error}"),
        }
    }
}

#[test]
fn stock_ogg_decodes_pcm_identically_across_runs() {
    let Some(ogg) = StockOgg::load() else { return; };

    let decode_all = || {
        let mut source =
            OggSource::from_bytes(ogg.bytes.clone(), StreamKind::Ogg).expect("stock Ogg opens");
        let mut chunk = vec![0x7fi16; OGG_CHUNK_BYTES / 2];
        let mut all: Vec<i16> = Vec::new();
        loop {
            let fill = source.fill(&mut chunk).expect("decode pass succeeds");
            all.extend_from_slice(&chunk);
            if fill.eof {
                break;
            }
        }
        all
    };

    // Two independent decode passes must be bit-identical (PCM-identical per
    // the Vorbis spec, quantized with libvorbis ov_read conversion).
    let pass_bytes =
        |samples: &[i16]| -> Vec<u8> { samples.iter().flat_map(|s| s.to_le_bytes()).collect() };
    let first = decode_all();
    let second = decode_all();
    assert!(!first.is_empty(), "stock Ogg decodes non-empty PCM");
    assert_eq!(
        first.len(),
        second.len(),
        "decode length stable across passes"
    );
    assert_eq!(
        pass_bytes(&first),
        pass_bytes(&second),
        "decode bit-identical across passes"
    );
}

/// Golden regression pin for symphonia's Vorbis output on the stock fixture:
/// FNV-1a 64 over all decoded i16 LE bytes. Regenerate only alongside a
/// documented dependency upgrade.
const GOLDEN_PCM_FNV64: u64 = 6060863096668405065;

#[test]
fn stock_ogg_pcm_matches_golden_hash() {
    let Some(ogg) = StockOgg::load() else { return; };
    let mut source = OggSource::from_bytes(ogg.bytes.clone(), StreamKind::Ogg).expect("opens");
    let mut chunk = vec![0i16; OGG_CHUNK_BYTES / 2];
    let mut pcm: Vec<u8> = Vec::new();
    loop {
        let fill = source.fill(&mut chunk).expect("decodes");
        pcm.extend(chunk.iter().flat_map(|s| s.to_le_bytes()));
        if fill.eof {
            break;
        }
    }
    assert_eq!(
        fnv64(&pcm),
        GOLDEN_PCM_FNV64,
        "{} total PCM bytes",
        pcm.len()
    );
}

/// Full deterministic lifecycle scenario over both stream families.
/// Returns the complete graph trace; the M3 gate compares two runs.
fn lifecycle_scenario(ogg: &StockOgg) -> Vec<LifecycleEvent> {
    let mut graph = OutputGraph::new();
    let mut manager = StreamManager::new(2);

    // Non-looping music-style Ogg: open, play, stream to EOF, stop, destroy.
    let non_loop = OggSource::from_bytes(ogg.bytes.clone(), StreamKind::Ogg).expect("ogg opens");
    let id_a = manager
        .open(&mut graph, Box::new(non_loop), BUFFERS_PER_STREAM)
        .unwrap();
    manager.start_playback(&mut graph, id_a).unwrap();
    while manager.request_chunk(&mut graph, id_a).unwrap().is_some() {}
    manager.drain(&mut graph, id_a).unwrap();
    manager.stop_playback(&mut graph, id_a).unwrap();

    // Looping XA: synthetic blocks, streamed well past one pass.
    let encoded: Vec<u8> = (0..4u32)
        .flat_map(|block| {
            let mut raw = vec![0u8; 15];
            raw[0] = 0x04 | ((block * 7) as u8 & 0xf0); // predictor varies per block
            for (index, byte) in raw.iter_mut().enumerate().skip(1) {
                *byte = ((index << 4) | (15 - index)) as u8;
            }
            raw
        })
        .collect();
    let xa_loop = XaSource::new(encoded, 112, 22050, StreamKind::XaLooping).unwrap();
    let id_b = manager
        .open(&mut graph, Box::new(xa_loop), BUFFERS_PER_STREAM)
        .unwrap();
    manager.start_playback(&mut graph, id_b).unwrap();
    for _ in 0..6 {
        manager.request_chunk(&mut graph, id_b).unwrap();
    }
    manager.drain(&mut graph, id_b).unwrap();
    manager.stop_playback(&mut graph, id_b).unwrap();

    let destroyed = manager.shutdown(&mut graph);
    assert_eq!(
        graph.trace().last(),
        Some(&LifecycleEvent::ShutdownCompleted {
            streams_destroyed: destroyed
        })
    );
    graph.trace().to_vec()
}

#[test]
fn lifecycle_double_run_produces_identical_traces() {
    let Some(ogg) = StockOgg::load() else { return; };
    let first = lifecycle_scenario(&ogg);
    let second = lifecycle_scenario(&ogg);
    assert!(!first.is_empty(), "scenario records events");
    assert_eq!(
        first, second,
        "same event sequence and state transitions twice"
    );
}

#[test]
fn lifecycle_shutdown_is_clean_and_leak_free() {
    let Some(ogg) = StockOgg::load() else { return; };
    let trace = lifecycle_scenario(&ogg);
    let opens = trace
        .iter()
        .filter(|e| matches!(e, LifecycleEvent::StreamOpened { .. }))
        .count();
    let destroys = trace
        .iter()
        .filter(|e| matches!(e, LifecycleEvent::StreamDestroyed { .. }))
        .count();
    assert_eq!(opens, destroys, "every opened stream is destroyed");
    assert_eq!(
        trace
            .iter()
            .filter(|e| matches!(e, LifecycleEvent::EndOfStream { .. }))
            .count(),
        1,
        "exactly one terminal EOF (non-looping Ogg); the XA loop never ends"
    );
    assert!(
        matches!(
            trace.last(),
            Some(LifecycleEvent::ShutdownCompleted {
                streams_destroyed: 2
            })
        ),
        "shutdown completes with both streams torn down (the looping XA survives until shutdown)"
    );
}

#[test]
fn repeated_teardown_cycles_are_stable() {
    let Some(ogg) = StockOgg::load() else { return; };
    let mut reference_frames: Option<u64> = None;
    for _ in 0..20 {
        let mut graph = OutputGraph::new();
        let mut manager = StreamManager::new(1);
        let source =
            OggSource::from_bytes(ogg.bytes.clone(), StreamKind::OggLooping).expect("ogg opens");
        let id = manager
            .open(&mut graph, Box::new(source), BUFFERS_PER_STREAM)
            .unwrap();
        manager.start_playback(&mut graph, id).unwrap();
        for _ in 0..4 {
            manager.request_chunk(&mut graph, id).unwrap();
        }
        manager.drain(&mut graph, id).unwrap();
        let frames = graph.frames_written(id);
        match reference_frames {
            None => reference_frames = Some(frames),
            Some(expected) => assert_eq!(frames, expected, "cycle frame count deterministic"),
        }
        let destroyed = manager.shutdown(&mut graph);
        assert_eq!(destroyed, 1, "shutdown tears down the one active stream");
    }
}

#[test]
fn invalid_ogg_header_is_rejected_loudly() {
    let err = OggSource::from_bytes(b"notogg".to_vec(), StreamKind::Ogg).unwrap_err();
    assert_eq!(err.reason_code(), "audio.open_failed");
}

#[test]
fn truncated_ogg_reaches_deterministic_eof_or_loud_decode_failure() {
    let Some(mut encoded) = StockOgg::load().map(|ogg| ogg.bytes) else { return; };
    encoded.truncate(encoded.len() / 2);
    let mut graph = OutputGraph::new();
    let mut manager = StreamManager::new(1);
    match OggSource::from_bytes(encoded, StreamKind::Ogg) {
        Err(err) => assert_eq!(err.reason_code(), "audio.open_failed"),
        Ok(source) => {
            let id = manager
                .open(&mut graph, Box::new(source), BUFFERS_PER_STREAM)
                .unwrap();
            // The truncated stream ends in exactly one of two deterministic
            // ways, mirroring first-party ov_read handling: a clean terminal
            // EOF (exactly one EndOfStream event) or a loud decode failure.
            enum Outcome {
                Eof(usize),
                Failed(String),
            }
            let outcome = loop {
                match manager.request_chunk(&mut graph, id) {
                    Ok(Some(_)) => continue,
                    Ok(None) => {
                        break Outcome::Eof(
                            graph
                                .trace()
                                .iter()
                                .filter(|e| matches!(e, LifecycleEvent::EndOfStream { .. }))
                                .count(),
                        );
                    }
                    Err(err) => break Outcome::Failed(err.reason_code().to_string()),
                }
            };
            match outcome {
                Outcome::Eof(eofs) => {
                    assert_eq!(eofs, 1, "exactly one deterministic EOF event");
                    manager.drain(&mut graph, id).unwrap();
                    assert!(!manager.is_alive(id));
                }
                Outcome::Failed(code) => assert_eq!(code, "audio.decode_failed"),
            }
        }
    }
}

#[test]
fn xa_non_looping_stream_ends_with_single_eof() {
    let encoded: Vec<u8> = vec![0x04; 15 * 8]; // eight valid silent-ish blocks
    let mut graph = OutputGraph::new();
    let mut manager = StreamManager::new(1);
    let source = XaSource::new(encoded, 224, 22050, StreamKind::Xa).unwrap();
    let id = manager
        .open(&mut graph, Box::new(source), BUFFERS_PER_STREAM)
        .unwrap();
    while manager.request_chunk(&mut graph, id).unwrap().is_some() {}
    manager.drain(&mut graph, id).unwrap();
    assert_eq!(
        graph
            .trace()
            .iter()
            .filter(|e| matches!(e, LifecycleEvent::EndOfStream { .. }))
            .count(),
        1
    );
    assert!(!manager.is_alive(id));
}

#[test]
fn rodio_sink_constructs_and_tears_down_headlessly() {
    // No speakers required: Player::new() wires no device until connected.
    let mut graph = OutputGraph::with_sink_factory(std::sync::Arc::new(|_info| {
        Box::new(RodioSink::new()) as Box<dyn hp_audio::graph::AudioSink>
    }));
    let mut manager = StreamManager::new(1);
    let source = XaSource::new(vec![0x04; 15 * 2], 56, 22050, StreamKind::Xa).unwrap();
    let id = manager.open(&mut graph, Box::new(source), 1).unwrap();
    manager.drain(&mut graph, id).unwrap();
    let destroyed = manager.shutdown(&mut graph);
    assert_eq!(destroyed, 1);
}
