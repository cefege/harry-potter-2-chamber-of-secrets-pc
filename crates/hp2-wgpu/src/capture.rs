//! End-of-frame readback for HP2_CAPTURE_FRAMES (mirrors XOpenGLDrv's capture hook, XOpenGL.cpp ~2614).

/// Reads mip 0 of `texture` back as tightly packed RGBA8 bytes, row 0 first
/// (top row, wgpu's texture origin). `texture` must have
/// `TextureUsages::COPY_SRC`. Supported formats: Rgba8Unorm, Rgba8UnormSrgb
/// (returned as-is), Bgra8Unorm, Bgra8UnormSrgb (B and R swapped to RGBA).
/// Returns `None` for any other format. Blocks until the GPU finishes.
pub fn read_texture_rgba8(
    created: &crate::CreatedDevice,
    texture: &wgpu::Texture,
) -> Option<Vec<u8>> {
    let swap_rb = match texture.format() {
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => false,
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => true,
        _ => return None,
    };
    let width = texture.width();
    let height = texture.height();
    // copy_texture_to_buffer requires bytes_per_row to be a multiple of
    // COPY_BYTES_PER_ROW_ALIGNMENT (256), so rows are padded on the GPU
    // side and unpadded on the CPU side below.
    let unpadded = width * 4;
    let padded =
        unpadded.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

    let buffer = created.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("hp2 texture readback buffer"),
        size: padded as u64 * height as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = created
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("hp2 texture readback encoder"),
        });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    created.queue.submit(std::iter::once(encoder.finish()));

    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |result| {
        result.expect("texture readback buffer map_async should succeed on a working device");
    });
    created
        .device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("device poll should succeed on a working device");
    let mapped = slice
        .get_mapped_range()
        .expect("buffer should be mapped after a successful map_async + device.poll");

    let unpadded = unpadded as usize;
    let mut out = Vec::with_capacity(unpadded * height as usize);
    for row in 0..height as usize {
        let start = row * padded as usize;
        out.extend_from_slice(&mapped[start..start + unpadded]);
    }
    if swap_rb {
        for pixel in out.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
    }
    drop(mapped);
    buffer.unmap();
    Some(out)
}

/// HP2_CAPTURE_FRAMES state, mirroring `GFrameCapture` in XOpenGL.cpp.
/// Each `write_frame` writes `frame_<6d>.png` and rewrites `frame_meta.json`.
pub struct FrameCapture {
    directory: std::path::PathBuf,
    map_name: String,
    requested_ticks: Option<i32>,
    frame_index: u32,
    meta_entries: Vec<String>,
    enabled: bool,
}

impl FrameCapture {
    /// Reads `HP2_CAPTURE_FRAMES`, `HP2_CAPTURE_MAP` and `HP2_CAPTURE_TICKS`.
    /// Unset or empty values count as absent; returns `None` when capture is off.
    pub fn from_env() -> Option<FrameCapture> {
        let directory = env_value("HP2_CAPTURE_FRAMES")?;
        let map_name = env_value("HP2_CAPTURE_MAP").unwrap_or_default();
        let requested_ticks = env_value("HP2_CAPTURE_TICKS").map(|value| parse_ticks(&value));
        // Best effort, like the C++ mkdir: the harness normally pre-creates it.
        let _ = std::fs::create_dir_all(&directory);
        Some(FrameCapture::new(
            std::path::PathBuf::from(directory),
            map_name,
            requested_ticks,
        ))
    }

    pub fn new(
        directory: std::path::PathBuf,
        map_name: String,
        requested_ticks: Option<i32>,
    ) -> FrameCapture {
        FrameCapture {
            directory,
            map_name,
            requested_ticks,
            frame_index: 0,
            meta_entries: Vec::new(),
            enabled: true,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Writes one presented frame. `rgba` is tightly packed RGBA8, top row first.
    pub fn write_frame(
        &mut self,
        rgba: &[u8],
        width: u32,
        height: u32,
        backing_scale: f64,
        renderer: &str,
    ) -> std::io::Result<()> {
        if !self.enabled || width == 0 || height == 0 {
            return Ok(());
        }
        assert_eq!(rgba.len(), width as usize * height as usize * 4);
        // The C++ writer emits 3-channel PNGs, so alpha is dropped here.
        let rgb: Vec<u8> = rgba
            .chunks_exact(4)
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2]])
            .collect();

        let path = self
            .directory
            .join(format!("frame_{:06}.png", self.frame_index));
        if let Err(error) = write_png(&path, &rgb, width, height) {
            self.enabled = false;
            return Err(error);
        }

        let mut entry = format!(
            "{{\"width_px\":{width},\"height_px\":{height},\"backing_scale_factor\":{backing_scale:.6},\"renderer\":"
        );
        append_json_string(&mut entry, renderer);
        entry.push_str(",\"ticks\":");
        // Matches XOpenGL.cpp: a negative or absent tick count is written as null.
        match self.requested_ticks.filter(|ticks| *ticks >= 0) {
            Some(ticks) => entry.push_str(&ticks.to_string()),
            None => entry.push_str("null"),
        }
        entry.push_str(",\"map\":");
        append_json_string(&mut entry, &self.map_name);
        entry.push('}');
        self.meta_entries.push(entry);

        if let Err(error) = std::fs::write(self.directory.join("frame_meta.json"), self.meta_json())
        {
            self.enabled = false;
            return Err(error);
        }
        self.frame_index += 1;
        Ok(())
    }

    fn meta_json(&self) -> String {
        format!("{{\"captures\":[{}]}}\n", self.meta_entries.join(","))
    }
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn write_png(path: &std::path::Path, rgb: &[u8], width: u32, height: u32) -> std::io::Result<()> {
    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(std::io::Error::other)?;
    writer
        .write_image_data(rgb)
        .map_err(std::io::Error::other)?;
    writer.finish().map_err(std::io::Error::other)
}

/// Appends `text` as a JSON string, escaping like `HPAppendJsonString`.
fn append_json_string(out: &mut String, text: &str) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// C `atoi` semantics, saturating instead of overflowing.
fn parse_ticks(text: &str) -> i32 {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len()
        && matches!(
            bytes[index],
            b' ' | b'\t' | b'\n' | b'\x0b' | b'\x0c' | b'\r'
        )
    {
        index += 1;
    }
    let mut negative = false;
    if index < bytes.len() && (bytes[index] == b'+' || bytes[index] == b'-') {
        negative = bytes[index] == b'-';
        index += 1;
    }
    let mut value: i32 = 0;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        let digit = i32::from(bytes[index] - b'0');
        value = if negative {
            value.saturating_mul(10).saturating_sub(digit)
        } else {
            value.saturating_mul(10).saturating_add(digit)
        };
        index += 1;
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn read_texture_rgba8_unpads_rows_for_a_non_aligned_width() {
        let created = crate::create_device_sync(
            crate::BackendChoice::Auto,
            None::<crate::HeadlessSurfaceCreator>,
        )
        .expect("expected a real device to be created");
        // 3 texels wide = 12 bytes per row, far from the 256-byte GPU
        // row alignment, so the padding path is actually exercised.
        let texture = created.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("capture unpad test texture"),
            size: wgpu::Extent3d {
                width: 3,
                height: 2,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let mut bytes = Vec::with_capacity(24);
        for i in 0u8..6 {
            bytes.extend_from_slice(&[i * 10, i * 10 + 1, i * 10 + 2, 255]);
        }
        created.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(12),
                rows_per_image: Some(2),
            },
            wgpu::Extent3d {
                width: 3,
                height: 2,
                depth_or_array_layers: 1,
            },
        );
        let out = read_texture_rgba8(&created, &texture)
            .expect("Rgba8Unorm with COPY_SRC should read back");
        assert_eq!(out, bytes);
        println!(
            "read_texture_rgba8 unpadded a 3x2 Rgba8Unorm readback on adapter {:?} backend {}",
            created.adapter_name,
            crate::backend_name(created.backend)
        );
    }

    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn read_texture_rgba8_swaps_bgra_to_rgba() {
        let created = crate::create_device_sync(
            crate::BackendChoice::Auto,
            None::<crate::HeadlessSurfaceCreator>,
        )
        .expect("expected a real device to be created");
        let texture = created.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("capture bgra swap test texture"),
            size: wgpu::Extent3d {
                width: 3,
                height: 2,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        // Each pixel stored in BGRA order: B=1, G=2, R=3, A=255.
        let bytes: Vec<u8> = [1u8, 2, 3, 255].repeat(6);
        created.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(12),
                rows_per_image: Some(2),
            },
            wgpu::Extent3d {
                width: 3,
                height: 2,
                depth_or_array_layers: 1,
            },
        );
        let out = read_texture_rgba8(&created, &texture)
            .expect("Bgra8Unorm with COPY_SRC should read back");
        assert_eq!(out.len(), 24);
        for pixel in out.chunks_exact(4) {
            assert_eq!(pixel, &[3u8, 2, 1, 255]);
        }
    }

    #[test]
    #[ignore = "needs a real GPU adapter; run explicitly with `cargo test -p hp2-wgpu -- --ignored`"]
    fn read_texture_rgba8_rejects_unsupported_formats() {
        let created = crate::create_device_sync(
            crate::BackendChoice::Auto,
            None::<crate::HeadlessSurfaceCreator>,
        )
        .expect("expected a real device to be created");
        let texture = created.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("capture unsupported format test texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        assert!(read_texture_rgba8(&created, &texture).is_none());
    }

    fn test_dir(suffix: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "hp2-capture-test-{}-{}",
            std::process::id(),
            suffix
        ))
    }

    #[test]
    fn parse_ticks_matches_c_atoi() {
        assert_eq!(parse_ticks("120"), 120);
        assert_eq!(parse_ticks("  42"), 42);
        assert_eq!(parse_ticks("-7"), -7);
        assert_eq!(parse_ticks("+5"), 5);
        assert_eq!(parse_ticks("12abc"), 12);
        assert_eq!(parse_ticks("abc"), 0);
        assert_eq!(parse_ticks(""), 0);
        assert_eq!(parse_ticks("99999999999999"), i32::MAX);
        assert_eq!(parse_ticks("-99999999999999"), i32::MIN);
    }

    #[test]
    fn append_json_string_escapes_like_the_cpp_writer() {
        let mut out = String::new();
        append_json_string(&mut out, "Apple \"M5\"\\\n");
        assert_eq!(out, "\"Apple \\\"M5\\\"\\\\\\u000a\"");
    }

    #[test]
    fn write_frame_writes_pngs_and_the_exact_meta_json() {
        let dir = test_dir("pngs-and-meta");
        std::fs::create_dir_all(&dir).unwrap();
        let pixels: [u8; 8] = [255, 0, 0, 9, 0, 255, 0, 9];
        let mut capture = FrameCapture::new(dir.clone(), "Entry".into(), Some(120));
        capture.write_frame(&pixels, 2, 1, 2.0, "Apple M5").unwrap();
        capture.write_frame(&pixels, 2, 1, 1.0, "Apple M5").unwrap();
        assert!(dir.join("frame_000000.png").exists());
        assert!(dir.join("frame_000001.png").exists());

        let mut reader = png::Decoder::new(std::io::BufReader::new(
            std::fs::File::open(dir.join("frame_000000.png")).unwrap(),
        ))
        .read_info()
        .unwrap();
        assert_eq!(reader.info().color_type, png::ColorType::Rgb);
        assert_eq!((reader.info().width, reader.info().height), (2, 1));
        let mut buffer = vec![0; reader.output_buffer_size().unwrap()];
        reader.next_frame(&mut buffer).unwrap();
        assert_eq!(buffer, [255u8, 0, 0, 0, 255, 0]);

        let meta = std::fs::read_to_string(dir.join("frame_meta.json")).unwrap();
        assert_eq!(
            meta,
            "{\"captures\":[{\"width_px\":2,\"height_px\":1,\"backing_scale_factor\":2.000000,\"renderer\":\"Apple M5\",\"ticks\":120,\"map\":\"Entry\"},{\"width_px\":2,\"height_px\":1,\"backing_scale_factor\":1.000000,\"renderer\":\"Apple M5\",\"ticks\":120,\"map\":\"Entry\"}]}\n"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn write_frame_uses_null_ticks_when_unset_or_negative() {
        let dir = test_dir("null-ticks");
        std::fs::create_dir_all(&dir).unwrap();
        let mut unset = FrameCapture::new(dir.clone(), "Map".into(), None);
        unset.write_frame(&[0, 0, 0, 255], 1, 1, 1.0, "R").unwrap();
        let meta = std::fs::read_to_string(dir.join("frame_meta.json")).unwrap();
        assert!(meta.contains("\"ticks\":null"), "meta was {meta}");

        let dir_negative = test_dir("null-ticks-negative");
        std::fs::create_dir_all(&dir_negative).unwrap();
        let mut negative = FrameCapture::new(dir_negative.clone(), "Map".into(), Some(-1));
        negative
            .write_frame(&[0, 0, 0, 255], 1, 1, 1.0, "R")
            .unwrap();
        let meta = std::fs::read_to_string(dir_negative.join("frame_meta.json")).unwrap();
        assert!(meta.contains("\"ticks\":null"), "meta was {meta}");

        std::fs::remove_dir_all(&dir).unwrap();
        std::fs::remove_dir_all(&dir_negative).unwrap();
    }

    #[test]
    fn disabled_capture_writes_nothing() {
        let dir = test_dir("disabled");
        std::fs::create_dir_all(&dir).unwrap();
        let mut capture = FrameCapture::new(dir.clone(), "Map".into(), None);
        capture.enabled = false;
        capture
            .write_frame(&[0, 0, 0, 255], 1, 1, 1.0, "R")
            .unwrap();
        assert!(!capture.is_enabled());
        assert!(!dir.join("frame_000000.png").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
