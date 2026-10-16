//! Replaces the `vorbisfile`/`libvorbis` calls in
//! `HarryPotter2/Unreal/Core/Src/UnFileStream.cpp` (`ov_open`/`ov_read`/
//! `ov_pcm_seek`/`ov_clear`) and
//! `HarryPotter2/Unreal/ALAudio/Src/ALAudioSubsystem.cpp` (`ov_info`/
//! `ov_pcm_total`/`ov_time_total`) with the pure-Rust `lewton` decoder.
//!
//! [`Decoder`] keeps the exact two-phase lifecycle `OggVorbis_File` had:
//! callers allocate an empty decoder ([`Decoder::new`], replacing
//! `new OggVorbis_File`), then initialize it in place
//! ([`Decoder::open`], replacing `ov_open(File, Ogg, NULL, 0)`). This
//! preserves every existing call site's allocate-then-open-then-on-failure-
//! free pattern without redesigning `FFileStream`'s `CreateStream`
//! contract (`Stream.TDD` stays caller-allocated for the Ogg stream types,
//! exactly as it is today).
//!
//! PCM fidelity: `lewton`'s own `Sample for i16` impl truncates
//! (`fl as i16`) where `libvorbis`'s `vorbis_ftoi` rounds to nearest
//! (`floor(x + 0.5)` on the non-x86 fallback path `os.h` takes on
//! aarch64, which is what this engine ships on). This module bypasses
//! `lewton`'s i16 conversion, takes its raw `f32` PCM, and reconverts
//! with `libvorbis`'s exact rule (see [`vorbis_ftoi`]) so the two decoders
//! agree to within the ±1 LSB inherent in two independently implemented
//! inverse MDCTs (confirmed empirically: 491 of 882,688 samples differ by
//! exactly 1, on the stock fixture `sm_bur_PlayfulFail_01.ogg`, down from
//! 443,008 differing bytes before this fix). `Tests/AudioTests.cpp`'s
//! comparison against the real `libvorbisfile` tolerates that ±1.
//!
//! `pcm_total`/`time_total`: `lewton` has no direct equivalent of
//! `ov_pcm_total`'s seek-based lookup. [`Decoder::open`] decodes the whole
//! file once to count exact total samples, then reopens the file for
//! sequential streaming — simpler and exact, at the cost of decoding
//! streamed files twice at registration time (a one-time, non-per-frame
//! cost).

use std::fs::File;

use lewton::inside_ogg::OggStreamReader;

/// Matches `libvorbis`'s `vorbis_ftoi`, which is rounding-mode-specific in
/// `lib/os.h`: x86_64 (CI's `linux-x64`/`windows-x64`, both built with
/// SSE2 math) takes the `_mm_cvtsd_si32` path, round-to-nearest with ties
/// to even (MXCSR default). Every other architecture — including aarch64,
/// which is what this engine ships on for macOS/Linux — falls through to
/// the generic `(int)floor(f + .5)`, which rounds exact `.5` ties up
/// (toward +∞) rather than to even.
#[cfg(target_arch = "x86_64")]
fn vorbis_ftoi(f: f64) -> i32 {
    f.round_ties_even() as i32
}

#[cfg(not(target_arch = "x86_64"))]
fn vorbis_ftoi(f: f64) -> i32 {
    (f + 0.5).floor() as i32
}

/// Matches `vorbisfile.c`'s `val = vorbis_ftoi(src[j] * 32768.f)`: the
/// multiply happens in `f32` (`32768.f` is a float literal) before
/// promoting to `f64` for `vorbis_ftoi`, then clamps to `i16` range.
fn sample_to_i16(sample: f32) -> i16 {
    let scaled: f32 = sample * 32768.0f32;
    vorbis_ftoi(scaled as f64).clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

fn interleave(packet: &[Vec<f32>]) -> Vec<u8> {
    if packet.is_empty() {
        return Vec::new();
    }
    let frames = packet[0].len();
    let mut out = Vec::with_capacity(frames * packet.len() * 2);
    for frame in 0..frames {
        for channel in packet.iter() {
            out.extend_from_slice(&sample_to_i16(channel[frame]).to_le_bytes());
        }
    }
    out
}

/// Counts total PCM sample frames in the Ogg Vorbis file at `path` by
/// decoding it once, discarding the audio. `None` on open/decode failure.
fn count_pcm_total(path: &str) -> Option<i64> {
    let file = File::open(path).ok()?;
    let mut reader = OggStreamReader::new(file).ok()?;
    let mut total: i64 = 0;
    loop {
        match reader.read_dec_packet_generic::<Vec<Vec<f32>>>() {
            Ok(Some(packet)) => {
                if let Some(channel) = packet.first() {
                    total += channel.len() as i64;
                }
            }
            Ok(None) => break,
            Err(_) => return None,
        }
    }
    Some(total)
}

struct Opened {
    path: String,
    reader: OggStreamReader<File>,
    channels: i32,
    rate: i32,
    pcm_total: i64,
    residue: Vec<u8>,
    residue_offset: usize,
}

/// Replaces `OggVorbis_File`. `None` until [`Decoder::open`] succeeds,
/// matching the struct being uninitialized until `ov_open` fills it in.
pub struct Decoder {
    state: Option<Opened>,
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder {
    /// Replaces `new OggVorbis_File` (an empty, not-yet-opened decoder).
    pub fn new() -> Self {
        Decoder { state: None }
    }

    /// Replaces `ov_open(File, Ogg, NULL, 0)`: opens and parses headers in
    /// place, and (see the module doc) decodes the whole file once more to
    /// get an exact total sample count. `path` is a UTF-8 native path (the
    /// same conversion `appFopen`/`appToUtf8NativePath` produces). Returns
    /// whether it succeeded, matching `ov_open`'s `< 0` failure check at
    /// the call site (negated).
    pub fn open(&mut self, path: &str) -> bool {
        let Some(pcm_total) = count_pcm_total(path) else {
            return false;
        };
        let Ok(file) = File::open(path) else {
            return false;
        };
        let Ok(reader) = OggStreamReader::new(file) else {
            return false;
        };
        let channels = reader.ident_hdr.audio_channels as i32;
        let rate = reader.ident_hdr.audio_sample_rate as i32;
        self.state = Some(Opened {
            path: path.to_string(),
            reader,
            channels,
            rate,
            pcm_total,
            residue: Vec::new(),
            residue_offset: 0,
        });
        true
    }

    /// Replaces `ov_info(Ogg, -1)->channels`. 0 if not open.
    pub fn channels(&self) -> i32 {
        self.state.as_ref().map(|s| s.channels).unwrap_or(0)
    }

    /// Replaces `ov_info(Ogg, -1)->rate`. 0 if not open.
    pub fn rate(&self) -> i32 {
        self.state.as_ref().map(|s| s.rate).unwrap_or(0)
    }

    /// Replaces `ov_pcm_total(Ogg, -1)`. 0 if not open.
    pub fn pcm_total(&self) -> i64 {
        self.state.as_ref().map(|s| s.pcm_total).unwrap_or(0)
    }

    /// Replaces `ov_time_total(Ogg, -1)`. 0 if not open.
    pub fn time_total(&self) -> f64 {
        match &self.state {
            Some(s) if s.rate > 0 => s.pcm_total as f64 / s.rate as f64,
            _ => 0.0,
        }
    }

    /// Replaces one `ov_read(Ogg, dest, dest.len(), 0, 2, 1, &bitstream)`
    /// call: fills at most `dest.len()` bytes of interleaved 16-bit signed
    /// little-endian PCM. Returns the byte count written (may be less than
    /// `dest.len()`, matching `ov_read`'s packet-boundary granularity), 0
    /// on clean end of stream, or a negative value on a hard decode error
    /// (including "not open").
    pub fn read(&mut self, dest: &mut [u8]) -> i64 {
        let Some(state) = self.state.as_mut() else {
            return -1;
        };
        if dest.is_empty() {
            return 0;
        }
        loop {
            if state.residue_offset < state.residue.len() {
                let available = state.residue.len() - state.residue_offset;
                let n = available.min(dest.len());
                dest[..n].copy_from_slice(
                    &state.residue[state.residue_offset..state.residue_offset + n],
                );
                state.residue_offset += n;
                return n as i64;
            }
            match state.reader.read_dec_packet_generic::<Vec<Vec<f32>>>() {
                Ok(Some(packet)) => {
                    state.residue = interleave(&packet);
                    state.residue_offset = 0;
                    if state.residue.is_empty() {
                        // An empty packet produced no bytes; keep decoding
                        // rather than reporting a spurious EOF.
                        continue;
                    }
                }
                Ok(None) => return 0,
                Err(_) => return -1,
            }
        }
    }

    /// Replaces `ov_pcm_seek(Ogg, 0)`: this engine only ever seeks to the
    /// very start (looping music), so reopening the stream is sufficient
    /// and avoids needing arbitrary-position seek support. Returns whether
    /// it succeeded, matching `ov_pcm_seek`'s `== 0` success check
    /// (negated) at the call site. False if not open.
    pub fn seek_to_start(&mut self) -> bool {
        let Some(state) = self.state.as_mut() else {
            return false;
        };
        let Ok(file) = File::open(&state.path) else {
            return false;
        };
        let Ok(reader) = OggStreamReader::new(file) else {
            return false;
        };
        state.reader = reader;
        state.residue.clear();
        state.residue_offset = 0;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_to_i16_rounds_to_nearest_like_libvorbis() {
        // 0.5/32768 scales to exactly 0.5, an exact tie: aarch64's
        // floor(x+.5) rounds it up to 1, x86_64's round_ties_even rounds
        // it down to the nearest even value, 0. Either way it must not be
        // lewton's own truncating `as i16`, which would give 0 on aarch64.
        #[cfg(not(target_arch = "x86_64"))]
        assert_eq!(sample_to_i16(0.5 / 32768.0), 1);
        #[cfg(target_arch = "x86_64")]
        assert_eq!(sample_to_i16(0.5 / 32768.0), 0);
        assert_eq!(sample_to_i16(-0.5 / 32768.0), 0);
        // -2.5/32768: floor(-2.5+.5)=-2 and round_ties_even(-2.5)=-2 (-2 is
        // the even neighbor) agree on both rounding rules.
        assert_eq!(sample_to_i16(-2.5 / 32768.0), -2);
        assert_eq!(sample_to_i16(1.0), 32767); // clamps, does not wrap
        assert_eq!(sample_to_i16(-1.0), -32768);
    }

    #[test]
    fn interleave_orders_channels_within_each_frame() {
        let packet = vec![vec![0.0f32, 1.0 / 32768.0], vec![-1.0 / 32768.0, 0.0]];
        let bytes = interleave(&packet);
        // frame 0: ch0=0, ch1=-1 -> i16 0, -1
        // frame 1: ch0=1, ch1=0 -> i16 1, 0
        assert_eq!(bytes, vec![0, 0, 0xff, 0xff, 1, 0, 0, 0]);
    }

    #[test]
    fn unopened_decoder_reports_empty_metadata_and_read_error() {
        let mut decoder = Decoder::new();
        assert_eq!(decoder.channels(), 0);
        assert_eq!(decoder.rate(), 0);
        assert_eq!(decoder.pcm_total(), 0);
        assert_eq!(decoder.time_total(), 0.0);
        let mut buf = [0u8; 16];
        assert_eq!(decoder.read(&mut buf), -1);
        assert!(!decoder.seek_to_start());
    }

    #[test]
    fn open_rejects_missing_file() {
        let mut decoder = Decoder::new();
        assert!(!decoder.open("/nonexistent/hp2-vorbis-test-path.ogg"));
    }

    fn stock_fixture_path() -> String {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        format!("{manifest_dir}/../../HarryPotter2/Unreal/Music/sm_bur_PlayfulFail_01.ogg")
    }

    #[test]
    #[ignore = "needs data-prototype: HarryPotter2/Unreal/Music/sm_bur_PlayfulFail_01.ogg, untracked by git; run with --ignored locally"]
    fn decodes_stock_fixture_to_libvorbisfile_byte_count() {
        // Checks decode succeeds, metadata, and exact total byte count
        // against libvorbisfile's own output on this fixture. The ±1 LSB
        // sample-level comparison against the real libvorbisfile decoder
        // lives in Tests/AudioTests.cpp (PcmMatchesWithinOneLsb), which has
        // the independent oracle this crate does not link.
        let path = stock_fixture_path();
        assert!(
            std::path::Path::new(&path).is_file(),
            "stock fixture must exist to run this ignored test"
        );
        let mut decoder = Decoder::new();
        assert!(decoder.open(&path), "stock fixture opens");
        assert_eq!(decoder.channels(), 2);
        assert_eq!(decoder.rate(), 44100);
        assert!(decoder.pcm_total() > 0);
        let mut all = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            let n = decoder.read(&mut buf);
            if n <= 0 {
                break;
            }
            all.extend_from_slice(&buf[..n as usize]);
        }
        // libvorbisfile on this exact fixture produces 1_765_376 bytes
        // (measured directly against the pinned libvorbisfile source this
        // workspace builds; see this module's doc comment).
        assert_eq!(
            all.len(),
            1_765_376,
            "decoded byte count must match libvorbisfile"
        );

        // Looping: seek_to_start then a second pass must read-produce data
        // again from the beginning.
        assert!(decoder.seek_to_start());
        let mut second = [0u8; 4096];
        let n = decoder.read(&mut second);
        assert!(n > 0, "post-seek read must produce data");
        assert_eq!(
            &second[..n as usize],
            &all[..n as usize],
            "seek_to_start restarts from the beginning"
        );
    }
}
