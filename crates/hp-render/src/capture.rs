//! Frame capture hook (Milestone M6).
//!
//! Writes `frame_%06d.png` plus a `frame_meta.json` sidecar into the
//! directory named by `HP2_CAPTURE_FRAMES`. The JSON schema matches
//! `Build/smoke_maps.py::_capture_record` and
//! `Tools/baseline_compare.py` field names EXACTLY:
//!
//! ```json
//! {"captures": [
//!   {"width_px": 1024, "height_px": 768, "backing_scale_factor": 1.0,
//!    "renderer": "Apple M5", "ticks": 120, "map": "PrivetDr"}
//! ]}
//! ```
//!
//! PNGs are 8-bit RGB (non-interlaced), decodable by baseline_compare's
//! minimal reader. Frame numbering is deterministic: zero-padded, in
//! submission order.

use crate::device::{GpuContext, OffscreenTarget};
use crate::error::RenderError;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

/// One entry of the `captures` array; field names are protocol-frozen.
#[derive(Debug, Clone, PartialEq)]
pub struct CaptureMetaEntry {
    pub width_px: u32,
    pub height_px: u32,
    pub backing_scale_factor: f32,
    /// GPU adapter name, e.g. "Apple M5".
    pub renderer: String,
    pub ticks: u32,
    pub map: String,
}

/// Sequential frame writer bound to one map run.
pub struct FrameCapture {
    dir: PathBuf,
    frames_written: u32,
    captures: Vec<CaptureMetaEntry>,
}

impl FrameCapture {
    /// Bind capture output to a directory (created eagerly so a bad path
    /// fails loudly at startup, not after a full smoke run).
    pub fn new(dir: impl Into<PathBuf>) -> Result<FrameCapture, RenderError> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir).map_err(|e| RenderError::CaptureWriteFailed {
            path: dir.display().to_string(),
            detail: format!("create_dir failed: {e}"),
        })?;
        Ok(FrameCapture {
            dir,
            frames_written: 0,
            captures: Vec::new(),
        })
    }

    pub fn frames_written(&self) -> u32 {
        self.frames_written
    }

    /// Read the current target back, encode it as `frame_%06d.png`, record
    /// its metadata. Deterministic ordering by construction.
    pub fn capture_frame(
        &mut self,
        ctx: &GpuContext,
        target: &OffscreenTarget,
        backing_scale_factor: f32,
        ticks: u32,
        map: &str,
    ) -> Result<PathBuf, RenderError> {
        let bgra = target.read_pixels_bgra(ctx);
        let file_name = format!("frame_{:06}.png", self.frames_written);
        let path = self.dir.join(file_name);
        write_png_rgb(&path, target.width, target.height, &bgra)?;

        self.captures.push(CaptureMetaEntry {
            width_px: target.width,
            height_px: target.height,
            backing_scale_factor,
            renderer: ctx.adapter_name.clone(),
            ticks,
            map: map.to_string(),
        });
        self.frames_written += 1;
        Ok(path)
    }

    /// Write `frame_meta.json` summarizing every captured frame.
    pub fn finish(self) -> Result<PathBuf, RenderError> {
        let meta_path = self.dir.join("frame_meta.json");
        write_meta_json(&meta_path, &self.captures)?;
        Ok(meta_path)
    }
}

/// Encode tightly packed BGRA8 rows as an 8-bit RGB PNG (channel-swapping
/// during encoding).
fn write_png_rgb(path: &Path, width: u32, height: u32, bgra: &[u8]) -> Result<(), RenderError> {
    let fail = |detail: String| RenderError::CaptureWriteFailed {
        path: path.display().to_string(),
        detail,
    };
    let file = std::fs::File::create(path).map_err(|e| fail(format!("open failed: {e}")))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut png_writer = encoder
        .write_header()
        .map_err(|e| fail(format!("png header failed: {e}")))?;

    // BGRA rows -> one tightly packed RGB frame.
    let mut rgb = vec![0u8; (width * height * 3) as usize];
    for index in 0..(width * height) as usize {
        rgb[index * 3] = bgra[index * 4 + 2]; // R
        rgb[index * 3 + 1] = bgra[index * 4 + 1]; // G
        rgb[index * 3 + 2] = bgra[index * 4]; // B
    }
    png_writer
        .write_image_data(&rgb)
        .map_err(|e| fail(format!("png encode failed: {e}")))?;
    png_writer
        .finish()
        .map_err(|e| fail(format!("png finish failed: {e}")))?;
    Ok(())
}

/// Serialize the frozen schema by hand — serde-free keeps dependencies at
/// `png` only, and the field set is fixed by protocol anyway.
fn write_meta_json(path: &Path, captures: &[CaptureMetaEntry]) -> Result<(), RenderError> {
    use std::fmt::Write as _;
    let mut out = String::from("{\"captures\":[");
    for (index, entry) in captures.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        let _ = write!(
            out,
            "{{\"width_px\":{},\"height_px\":{},\"backing_scale_factor\":{:.6},\
              \"renderer\":\"{}\",\"ticks\":{},\"map\":\"{}\"}}",
            entry.width_px,
            entry.height_px,
            entry.backing_scale_factor,
            json_escape(&entry.renderer),
            entry.ticks,
            json_escape(&entry.map),
        );
    }
    out.push_str("]}");
    std::fs::write(path, out).map_err(|e| RenderError::CaptureWriteFailed {
        path: path.display().to_string(),
        detail: format!("meta write failed: {e}"),
    })
}

fn json_escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            c if (c as u32) < 0x20 => escaped.push_str(&format!("\\u{:04x}", c as u32)),
            c => escaped.push(c),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captures_frames_and_writes_schema_compatible_meta() {
        let ctx = GpuContext::headless().expect("headless Metal device");
        let target = OffscreenTarget::new(&ctx, 8, 8);
        target.clear_to(&ctx, [1.0, 1.0, 1.0, 1.0]);

        let dir =
            std::env::temp_dir().join(format!("hp-render-capture-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut capture = FrameCapture::new(&dir).expect("capture dir");
        let path = capture
            .capture_frame(&ctx, &target, 1.0, 120, "PrivetDr")
            .expect("frame encode");
        assert!(path.ends_with("frame_000000.png"));
        let second = capture
            .capture_frame(&ctx, &target, 2.0, 121, "PrivetDr")
            .expect("frame 2");
        assert!(second.ends_with("frame_000001.png"));

        let meta_path = capture.finish().expect("meta");
        let text = std::fs::read_to_string(&meta_path).expect("meta readable");
        assert!(text.starts_with("{\"captures\":["));
        assert!(text.contains("\"width_px\":8"));
        assert!(text.contains("\"height_px\":8"));
        assert!(text.contains("\"backing_scale_factor\":1.000000"));
        assert!(text.contains("\"renderer\":\""));
        assert!(text.contains("\"ticks\":120"));
        assert!(text.contains("\"map\":\"PrivetDr\""));

        // PNG must decode with the same expectations as
        // Tools/baseline_compare.py: 8-bit RGB, correct dimensions.
        let decoder = png::Decoder::new(std::io::BufReader::new(
            std::fs::File::open(&path).expect("png open"),
        ));
        let mut reader = decoder.read_info().expect("png info");
        assert_eq!(reader.info().width, 8);
        assert_eq!(reader.info().height, 8);
        assert_eq!(reader.info().color_type, png::ColorType::Rgb);
        let mut buf = vec![0u8; reader.output_buffer_size().expect("buffer size known")];
        let frame = reader.next_frame(&mut buf).expect("png decode");
        assert_eq!(frame.width, 8);
        // Cleared white must survive the BGRA->RGB swap.
        assert!(buf[..(8 * 8 * 3)].iter().all(|&b| b == 255));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn json_escape_handles_quotes_and_control_chars() {
        assert_eq!(json_escape("plain"), "plain");
        assert_eq!(json_escape("a\"b\\c\u{1}"), "a\\\"b\\\\c\\u0001");
    }
}
