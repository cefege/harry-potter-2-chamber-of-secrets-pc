//! Ordered launcher display-mode collection. Native services retain SDL and
//! request ownership; this policy never borrows native objects across a call.

use std::fmt::{self, Write};

pub type DisplayModeResult<T> = Result<T, i32>;

pub trait DisplayModeHost {
    fn initialize_video(&self) -> DisplayModeResult<()>;
    fn platform_count(&self) -> DisplayModeResult<i32>;
    fn platform_dimensions(&self, index: i32) -> DisplayModeResult<Option<(i32, i32)>>;
    fn request_count(&self) -> DisplayModeResult<usize>;
    fn request_dimensions(&self, index: usize) -> DisplayModeResult<(i32, i32)>;
    fn append_mode(&self, width: i32, height: i32, label: &[u8]) -> DisplayModeResult<()>;
}

const STANDARD_MODES: [(i32, i32); 14] = [
    (640, 480),
    (800, 600),
    (1024, 768),
    (1280, 720),
    (1280, 800),
    (1366, 768),
    (1440, 900),
    (1600, 900),
    (1680, 1050),
    (1920, 1080),
    (1920, 1200),
    (2560, 1440),
    (2560, 1600),
    (3456, 2234),
];

struct Label {
    bytes: [u8; 64],
    len: usize,
}

impl Write for Label {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let end = self.len + value.len();
        let target = self.bytes.get_mut(self.len..end).ok_or(fmt::Error)?;
        target.copy_from_slice(value.as_bytes());
        self.len = end;
        Ok(())
    }
}

fn add_mode(host: &impl DisplayModeHost, width: i32, height: i32) -> DisplayModeResult<()> {
    if width <= 0 || height <= 0 {
        return Ok(());
    }
    let count = host.request_count()?;
    for index in 0..count {
        if host.request_dimensions(index)? == (width, height) {
            return Ok(());
        }
    }
    let mut label = Label {
        bytes: [0; 64],
        len: 0,
    };
    // Two positive i32 values plus the UTF-8 separator need at most 24 bytes.
    write!(label, "{width} × {height}").map_err(|_| 1)?;
    host.append_mode(width, height, &label.bytes[..label.len])
}

/// Preserve existing entries and append first-seen positive dimension pairs:
/// native index order, then standards, then the configured current resolution.
/// Callback failures stop collection immediately without reporting success.
pub fn collect_display_modes(
    host: &impl DisplayModeHost,
    resolutions: bool,
    configured_width: i32,
    configured_height: i32,
) -> DisplayModeResult<()> {
    if resolutions {
        host.initialize_video()?;
        let count = host.platform_count()?;
        for index in 0..count {
            if let Some((width, height)) = host.platform_dimensions(index)? {
                add_mode(host, width, height)?;
            }
        }
    }
    for (width, height) in STANDARD_MODES {
        add_mode(host, width, height)?;
    }
    add_mode(host, configured_width, configured_height)
}
