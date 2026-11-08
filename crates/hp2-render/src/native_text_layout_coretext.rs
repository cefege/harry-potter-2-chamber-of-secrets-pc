//! CoreText layout policy. Native services are SDK calls and public-layout materialization only.
use crate::native_text_layout::{self as common, Hp2TextLayoutRange, Prepared, Range};
use core::ffi::c_void;
type Handle = *const c_void;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2TextLayoutCoreTextRequest {
    pub text: *const c_void,
    pub length: i32,
    pub unit_bytes: u32,
    pub name: *const c_void,
    pub name_length: usize,
    pub family: *const c_void,
    pub family_length: usize,
    pub font: *const c_void,
    pub font_height: i32,
    pub page_count: i32,
    pub scale: f32,
    pub space_x: f32,
    pub space_y: f32,
    pub clip_x: f32,
    pub clip_y: f32,
    pub color: [f32; 4],
    pub start_x: i32,
    pub start_y: i32,
    pub visible: i32,
    pub ampersand: u8,
    pub center: u8,
    pub wrapped: u8,
    pub poly_flags: u32,
    pub clip: u8,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Hp2TextLayoutCoreTextMetrics {
    pub advance: f64,
    pub ascent: f64,
    pub descent: f64,
    pub leading: f64,
    pub underline: f64,
    pub thickness: f64,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2TextLayoutCoreTextFontInfo {
    pub postscript: *const u8,
    pub postscript_len: usize,
    pub descriptor: *const u8,
    pub descriptor_len: usize,
    pub size: f64,
    pub matrix: [f64; 6],
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2TextLayoutCoreTextGlyph {
    pub glyph: u16,
    pub position: [f64; 2],
    pub advance: [f64; 2],
    pub bounds: [f64; 4],
    pub source: isize,
    pub null_bounds: u8,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2TextLayoutCoreTextRun {
    pub range: Hp2TextLayoutRange,
    pub font: *const c_void,
    pub glyphs: *const Hp2TextLayoutCoreTextGlyph,
    pub count: usize,
}
#[repr(C)]
pub struct Hp2TextLayoutCoreTextPlacement {
    pub font: *const c_void,
    pub glyph: u16,
    pub position: [f64; 2],
    pub advance: [f64; 2],
    pub bounds: [f64; 4],
    pub range: Hp2TextLayoutRange,
    pub identity: *const u8,
    pub identity_len: usize,
    pub point_size64: u32,
    pub drawable: u8,
    pub key_glyph: u16,
    pub antialias: u8,
    pub subpixel: u8,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2TextLayoutCoreTextCluster {
    pub range: Hp2TextLayoutRange,
    pub source_end: i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2TextLayoutCoreTextUnderline {
    pub start: f64,
    pub end: f64,
    pub baseline: f64,
    pub position: f64,
    pub thickness: f64,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Hp2TextLayoutCoreTextOutput {
    pub width: i32,
    pub height: i32,
    pub ascent: f64,
    pub descent: f64,
    pub glyphs: *const Hp2TextLayoutCoreTextPlacement,
    pub glyph_count: usize,
    pub clusters: *const Hp2TextLayoutCoreTextCluster,
    pub cluster_count: usize,
    pub underlines: *const Hp2TextLayoutCoreTextUnderline,
    pub underline_count: usize,
}
#[repr(C)]
pub struct Hp2TextLayoutCoreTextServices {
    pub release: unsafe extern "C" fn(*const c_void),
    pub retain: unsafe extern "C" fn(*const c_void),
    pub named_font: unsafe extern "C" fn(*mut c_void, *const u16, usize, f64) -> *const c_void,
    pub system_font: unsafe extern "C" fn(*mut c_void, u8, f64) -> *const c_void,
    pub font_info:
        unsafe extern "C" fn(*mut c_void, *const c_void, *mut Hp2TextLayoutCoreTextFontInfo) -> u8,
    pub source: unsafe extern "C" fn(*mut c_void, *const u16, usize) -> *const c_void,
    pub attributes: unsafe extern "C" fn(*mut c_void, *const c_void) -> *const c_void,
    pub number: unsafe extern "C" fn(*mut c_void, f64) -> *const c_void,
    pub set_font: unsafe extern "C" fn(*mut c_void, *const c_void, *const c_void, usize) -> u8,
    pub set_kern: unsafe extern "C" fn(*mut c_void, *const c_void, *const c_void, usize) -> u8,
    pub typesetter: unsafe extern "C" fn(*mut c_void, *const c_void) -> *const c_void,
    pub suggest: unsafe extern "C" fn(*mut c_void, *const c_void, isize, f64, u8) -> isize,
    pub line: unsafe extern "C" fn(*mut c_void, *const c_void, Hp2TextLayoutRange) -> *const c_void,
    pub metrics: unsafe extern "C" fn(
        *mut c_void,
        *const c_void,
        u8,
        *mut Hp2TextLayoutCoreTextMetrics,
    ) -> u8,
    pub component: unsafe extern "C" fn(*mut c_void, *const c_void, isize) -> Hp2TextLayoutRange,
    pub run_count: unsafe extern "C" fn(*mut c_void, *const c_void) -> usize,
    pub run: unsafe extern "C" fn(
        *mut c_void,
        *const c_void,
        usize,
        u8,
        *mut Hp2TextLayoutCoreTextRun,
    ) -> u8,
    pub offset: unsafe extern "C" fn(*mut c_void, *const c_void, isize) -> f64,
    pub raster: unsafe extern "C" fn(
        *mut c_void,
        *const c_void,
        u16,
        i32,
        i32,
        f64,
        f64,
        u8,
        u8,
        u8,
        u8,
        *mut u8,
    ) -> u8,
}
struct Host<'a> {
    api: &'a Hp2TextLayoutCoreTextServices,
    ctx: *mut c_void,
}
impl Host<'_> {
    fn owned(&self, raw: Handle) -> Option<Owned> {
        if raw.is_null() {
            None
        } else {
            Some(Owned {
                raw,
                release: self.api.release,
            })
        }
    }
    fn info(&self, font: Handle) -> Option<Hp2TextLayoutCoreTextFontInfo> {
        let mut info = core::mem::MaybeUninit::uninit();
        let ok = unsafe { (self.api.font_info)(self.ctx, font, info.as_mut_ptr()) };
        if ok == 0 {
            None
        } else {
            Some(unsafe { info.assume_init() })
        }
    }
    fn named(&self, name: &[u16], size: f64) -> Option<Owned> {
        if name.is_empty() || !common::valid_point_size(size) {
            return None;
        }
        let font = self
            .owned(unsafe { (self.api.named_font)(self.ctx, name.as_ptr(), name.len(), size) })?;
        let info = self.info(font.raw)?;
        let name = if info.postscript_len == 0 {
            &[][..]
        } else {
            unsafe { core::slice::from_raw_parts(info.postscript, info.postscript_len) }
        };
        if name == b".LastResort" {
            None
        } else {
            Some(font)
        }
    }
    fn role_font(&self, role: u32, size: f64) -> Option<Owned> {
        if !common::valid_point_size(size) {
            return None;
        }
        let bold = common::bold(role);
        for name in if bold {
            ["Times New Roman Bold", "Times-Bold"]
        } else {
            ["Times New Roman", "Times-Roman"]
        } {
            let name: Vec<u16> = name.encode_utf16().collect();
            if let Some(font) = self.named(&name, size) {
                return Some(font);
            }
        }
        self.owned(unsafe { (self.api.system_font)(self.ctx, bold as u8, size) })
    }
    fn component(&self, source: Handle, index: isize) -> Option<Range> {
        Some(unsafe { (self.api.component)(self.ctx, source, index) })
    }
    fn metrics(&self, handle: Handle, font: bool) -> Option<Hp2TextLayoutCoreTextMetrics> {
        let mut value = Hp2TextLayoutCoreTextMetrics::default();
        let ok = unsafe { (self.api.metrics)(self.ctx, handle, font as u8, &mut value) };
        (ok != 0).then_some(value)
    }
}
struct Owned {
    raw: Handle,
    release: unsafe extern "C" fn(*const c_void),
}
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe { (self.release)(self.raw) }
    }
}
struct Glyph {
    font: Owned,
    id: u16,
    position: [f64; 2],
    advance: [f64; 2],
    bounds: [f64; 4],
    range: Range,
    identity: Option<std::rc::Rc<String>>,
    point_size64: u32,
    drawable: bool,
}
#[derive(Default)]
pub struct Layout {
    width: i32,
    height: i32,
    ascent: f64,
    descent: f64,
    glyphs: Vec<Glyph>,
    clusters: Vec<Hp2TextLayoutCoreTextCluster>,
    underlines: Vec<Hp2TextLayoutCoreTextUnderline>,
    flat: Vec<Hp2TextLayoutCoreTextPlacement>,
    start_x: i32,
    start_y: i32,
    origin_x: f32,
    origin_y: f32,
    clip_x: f32,
    clip_y: f32,
    poly_flags: u32,
    color: [f32; 4],
    clip: bool,
    draw_bridge: *mut c_void,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Hp2TextLayoutCoreTextFields {
    pub start_x: i32,
    pub start_y: i32,
    pub origin_x: f32,
    pub origin_y: f32,
    pub clip_x: f32,
    pub clip_y: f32,
    pub clip: u8,
    pub poly_flags: u32,
    pub color: [f32; 4],
}
impl Layout {
    pub fn view(&self) -> Hp2TextLayoutCoreTextOutput {
        Hp2TextLayoutCoreTextOutput {
            width: self.width,
            height: self.height,
            ascent: self.ascent,
            descent: self.descent,
            glyphs: self.flat.as_ptr(),
            glyph_count: self.flat.len(),
            clusters: self.clusters.as_ptr(),
            cluster_count: self.clusters.len(),
            underlines: self.underlines.as_ptr(),
            underline_count: self.underlines.len(),
        }
    }
    pub fn fields(&self) -> Hp2TextLayoutCoreTextFields {
        Hp2TextLayoutCoreTextFields {
            start_x: self.start_x,
            start_y: self.start_y,
            origin_x: self.origin_x,
            origin_y: self.origin_y,
            clip_x: self.clip_x,
            clip_y: self.clip_y,
            clip: self.clip as u8,
            poly_flags: self.poly_flags,
            color: self.color,
        }
    }
    pub fn set_origin(&mut self, origin_x: f32, origin_y: f32) {
        self.origin_x = origin_x;
        self.origin_y = origin_y;
    }
    pub fn draw_bridge(&self) -> *mut c_void {
        self.draw_bridge
    }
    pub fn set_draw_bridge(&mut self, bridge: *mut c_void) {
        self.draw_bridge = bridge;
    }
}
fn end(range: Range) -> isize {
    range.location + range.length
}
fn identity(info: &Hp2TextLayoutCoreTextFontInfo) -> String {
    let bytes = |ptr, len| {
        if len == 0 {
            &[][..]
        } else {
            unsafe { core::slice::from_raw_parts(ptr, len) }
        }
    };
    format!(
        "{}|{}|{:.6}|{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}",
        String::from_utf8_lossy(bytes(info.postscript, info.postscript_len)),
        String::from_utf8_lossy(bytes(info.descriptor, info.descriptor_len)),
        info.size,
        info.matrix[0],
        info.matrix[1],
        info.matrix[2],
        info.matrix[3],
        info.matrix[4],
        info.matrix[5]
    )
}
fn add_cluster(layout: &mut Layout, range: Range, prepared: &Prepared) -> Option<()> {
    if range.length <= 0 || range.location < 0 || end(range) as usize > prepared.source_ends.len() {
        return None;
    }
    if layout.clusters.last().is_none_or(|c| c.range != range) {
        layout.clusters.push(Hp2TextLayoutCoreTextCluster {
            range,
            source_end: prepared.source_ends[end(range) as usize - 1],
        });
    }
    Some(())
}
fn line(
    host: &Host,
    layout: &mut Layout,
    typesetter: Handle,
    source: Handle,
    base: Handle,
    range: Range,
    prepared: &Prepared,
    request: &Hp2TextLayoutCoreTextRequest,
    top: &mut f64,
    capture: bool,
) -> Option<()> {
    let line = if range.length == 0 {
        None
    } else {
        Some(host.owned(unsafe { (host.api.line)(host.ctx, typesetter, range) })?)
    };
    let metrics = host.metrics(line.as_ref().map_or(base, |l| l.raw), line.is_none())?;
    let height = metrics.ascent + metrics.descent + metrics.leading + request.space_y as f64;
    let next = *top + height.max(0.0);
    if ![
        metrics.advance,
        metrics.ascent,
        metrics.descent,
        metrics.leading,
        height,
        next,
    ]
    .iter()
    .all(|x| x.is_finite())
        || metrics.advance > i32::MAX as f64
        || next > i32::MAX as f64
    {
        return None;
    }
    if let Some(line) = line {
        let mut ranges = Vec::new();
        let mut cursor = range.location;
        while cursor < end(range) {
            let component = host.component(source, cursor)?;
            if component.length <= 0
                || component.location < range.location
                || end(component) > end(range)
                || end(component) <= cursor
            {
                return None;
            }
            ranges.push(component);
            cursor = end(component);
        }
        // Capture a run once: SDK arrays are borrowed until the next run service call.
        let count = unsafe { (host.api.run_count)(host.ctx, line.raw) };
        let mut runs = Vec::with_capacity(count);
        for index in 0..count {
            let mut raw = core::mem::MaybeUninit::uninit();
            let ok = unsafe {
                (host.api.run)(host.ctx, line.raw, index, capture as u8, raw.as_mut_ptr())
            };
            if ok == 0 {
                return None;
            }
            let run: Hp2TextLayoutCoreTextRun = unsafe { raw.assume_init() };
            let glyphs = if run.count == 0 {
                &[][..]
            } else {
                unsafe { core::slice::from_raw_parts(run.glyphs, run.count) }
            };
            if run.range.length > 0
                && run.range.location >= range.location
                && end(run.range) <= end(range)
            {
                let mut indices = Vec::with_capacity(glyphs.len() + 2);
                indices.push(run.range.location);
                indices.extend(glyphs.iter().map(|g| g.source));
                indices.push(end(run.range));
                indices.sort_unstable();
                indices.dedup();
                for pair in indices.windows(2) {
                    let start = pair[0].max(range.location);
                    let stop = pair[1].min(end(range));
                    if start >= stop {
                        continue;
                    }
                    let first = host.component(source, start)?;
                    let last = host.component(source, stop - 1)?;
                    if first.location >= range.location && end(last) <= end(range) {
                        ranges.push(Range {
                            location: first.location,
                            length: end(last) - first.location,
                        });
                    }
                }
            }
            if capture {
                runs.push((run.font, glyphs.to_vec()));
            }
        }
        ranges.sort_unstable_by_key(|r| (r.location, r.length));
        let mut merged: Vec<Range> = Vec::with_capacity(ranges.len());
        for range in ranges {
            if let Some(previous) = merged.last_mut() {
                if range.location < end(*previous) {
                    previous.length = end(*previous).max(end(range)) - previous.location;
                    continue;
                }
            }
            merged.push(range);
        }
        let origin = if request.center != 0 {
            -metrics.advance * 0.5
        } else {
            0.0
        };
        let baseline = *top + metrics.ascent;
        if capture {
            for (font, glyphs) in runs {
                if font.is_null() || glyphs.is_empty() {
                    continue;
                }
                let mut font_key: Option<(std::rc::Rc<String>, u32)> = None;
                for glyph in glyphs {
                    let cluster = *merged
                        .iter()
                        .find(|c| glyph.source >= c.location && glyph.source < end(**c))?;
                    let position = [origin + glyph.position[0], baseline + glyph.position[1]];
                    let finite_bounds = glyph.null_bounds != 0
                        || (glyph.bounds.iter().all(|v| v.is_finite())
                            && glyph.bounds[2] >= 0.0
                            && glyph.bounds[3] >= 0.0);
                    if !position
                        .iter()
                        .chain(glyph.advance.iter())
                        .all(|v| v.is_finite())
                        || !finite_bounds
                    {
                        return None;
                    }
                    let drawable =
                        glyph.null_bounds == 0 && glyph.bounds[2] > 0.0 && glyph.bounds[3] > 0.0;
                    let (key, point_size64) = if drawable {
                        if font_key.is_none() {
                            let info = host.info(font)?;
                            font_key = Some((
                                std::rc::Rc::new(identity(&info)),
                                common::point_size64(info.size)?,
                            ));
                        }
                        let (key, size) = font_key.as_ref()?;
                        (Some(key.clone()), *size)
                    } else {
                        (None, 0)
                    };
                    unsafe { (host.api.retain)(font) };
                    layout.glyphs.push(Glyph {
                        font: Owned {
                            raw: font,
                            release: host.api.release,
                        },
                        id: glyph.glyph,
                        position,
                        advance: glyph.advance,
                        bounds: glyph.bounds,
                        range: cluster,
                        identity: key,
                        point_size64,
                        drawable,
                    });
                }
            }
        }
        let font_metrics = host.metrics(base, true)?;
        for underline in &prepared.underlines {
            let start = range.location.max(underline.location);
            let stop = end(range).min(end(*underline));
            if start >= stop {
                continue;
            }
            let a = unsafe { (host.api.offset)(host.ctx, line.raw, start) };
            let b = unsafe { (host.api.offset)(host.ctx, line.raw, stop) };
            if !a.is_finite() || !b.is_finite() || a == b {
                continue;
            }
            let thickness = font_metrics.thickness as f32;
            layout.underlines.push(Hp2TextLayoutCoreTextUnderline {
                start: origin + a.min(b),
                end: origin + a.max(b),
                baseline,
                position: font_metrics.underline,
                thickness: (if 1.0 >= thickness { 1.0 } else { thickness }) as f64,
            });
        }
        for range in merged {
            add_cluster(layout, range, prepared)?;
        }
        layout.width = layout.width.max(if metrics.advance > 0.0 {
            metrics.advance.ceil() as i32
        } else {
            0
        });
    }
    layout.ascent = layout.ascent.max(metrics.ascent);
    layout.descent = layout.descent.max(metrics.descent);
    *top = next;
    Some(())
}
fn build(
    host: &Host,
    request: &Hp2TextLayoutCoreTextRequest,
    prepared: &Prepared,
    base: Handle,
    capture: bool,
) -> Option<Layout> {
    let mut layout = Layout::default();
    if prepared.text.is_empty() {
        return Some(layout);
    }
    let source = host.owned(unsafe {
        (host.api.source)(host.ctx, prepared.text.as_ptr(), prepared.text.len())
    })?;
    let attributes = host.owned(unsafe { (host.api.attributes)(host.ctx, source.raw) })?;
    let ok = unsafe { (host.api.set_font)(host.ctx, attributes.raw, base, prepared.text.len()) };
    if ok == 0 {
        return None;
    }
    if request.space_x != 0.0 {
        let number = host.owned(unsafe { (host.api.number)(host.ctx, request.space_x as f64) });
        if let Some(number) = number {
            let ok = unsafe {
                (host.api.set_kern)(host.ctx, attributes.raw, number.raw, prepared.text.len())
            };
            if ok == 0 {
                return None;
            }
        }
    }
    let typesetter = host.owned(unsafe { (host.api.typesetter)(host.ctx, attributes.raw) })?;
    let width = request.clip_x as f64
        - if request.center != 0 {
            0.0
        } else {
            request.start_x as f64
        };
    let mut top = 0.0;
    let mut start = 0usize;
    let len = prepared.text.len();
    while start < len {
        let mut paragraph = start;
        while paragraph < len && prepared.text[paragraph] != 10 && prepared.text[paragraph] != 13 {
            paragraph += 1;
        }
        let mut cursor = start;
        if cursor == paragraph {
            line(
                host,
                &mut layout,
                typesetter.raw,
                source.raw,
                base,
                Range {
                    location: cursor as isize,
                    length: 0,
                },
                prepared,
                request,
                &mut top,
                capture,
            )?;
        }
        while cursor < paragraph {
            let mut length = paragraph - cursor;
            if request.wrapped != 0 {
                if width <= 0.0 {
                    return None;
                }
                let mut suggested = unsafe {
                    (host.api.suggest)(host.ctx, typesetter.raw, cursor as isize, width, 0)
                };
                if suggested <= 0 {
                    suggested = unsafe {
                        (host.api.suggest)(host.ctx, typesetter.raw, cursor as isize, width, 1)
                    };
                }
                if suggested <= 0 {
                    return None;
                }
                length = length.min(suggested as usize);
            }
            line(
                host,
                &mut layout,
                typesetter.raw,
                source.raw,
                base,
                Range {
                    location: cursor as isize,
                    length: length as isize,
                },
                prepared,
                request,
                &mut top,
                capture,
            )?;
            cursor += length;
        }
        start = paragraph;
        if paragraph < len {
            start += 1;
            if prepared.text[paragraph] == 13 && start < len && prepared.text[start] == 10 {
                start += 1;
            }
            add_cluster(
                &mut layout,
                Range {
                    location: paragraph as isize,
                    length: (start - paragraph) as isize,
                },
                prepared,
            )?;
            if start == len {
                line(
                    host,
                    &mut layout,
                    typesetter.raw,
                    source.raw,
                    base,
                    Range {
                        location: start as isize,
                        length: 0,
                    },
                    prepared,
                    request,
                    &mut top,
                    capture,
                )?;
            }
        }
    }
    layout.height = (top.ceil() as i32).max(0);
    Some(layout)
}
/// All pointers and services must remain valid for the synchronous call.
pub unsafe fn create(
    api: &Hp2TextLayoutCoreTextServices,
    ctx: *mut c_void,
    request: &Hp2TextLayoutCoreTextRequest,
    page_fn: unsafe extern "C" fn(*const c_void, i32) -> i32,
) -> Option<Layout> {
    let host = Host { api, ctx };
    if request.font.is_null()
        || ![
            request.scale,
            request.space_x,
            request.space_y,
            request.clip_x,
            request.clip_y,
        ]
        .iter()
        .chain(request.color.iter())
        .all(|v| common::safe_float(*v))
    {
        return None;
    }
    let mut prepared = unsafe {
        common::preprocess(
            request.text,
            request.length,
            request.unit_bytes,
            request.ampersand != 0,
        )
    }?;
    let name = unsafe { common::units(request.name, request.name_length, request.unit_bytes) };
    let role = common::role(&name, request.family_length != 0);
    let scale = if request.scale > 0.0 {
        request.scale as f64
    } else {
        1.0
    };
    let mut base = None;
    if request.family_length != 0 {
        let family = unsafe {
            common::preprocess(
                request.family,
                i32::try_from(request.family_length).ok()?,
                request.unit_bytes,
                false,
            )
        }?;
        let size = if request.font_height > 0 {
            request.font_height
        } else {
            common::role_size(8)
        };
        base = host.named(&family.text, size as f64 * scale);
    }
    if base.is_none() {
        let mut size = common::role_size(role);
        if request.family_length == 0 && !common::known_package(&name) {
            size = unsafe { common::legacy_bitmap_size(request.font, request.page_count, page_fn) };
        }
        base = host.role_font(role, size as f64 * scale);
    }
    let base = base?;
    if !prepared.text.is_empty()
        && request.visible > 0
        && prepared
            .source_ends
            .last()
            .is_some_and(|last| request.visible < *last)
    {
        let probe = build(&host, request, &prepared, base.raw, false)?;
        let visible = common::visible_end(
            probe.clusters.iter().map(|c| (c.range, c.source_end)),
            request.visible,
            prepared.text.len(),
        );
        prepared.text.truncate(visible);
        prepared.source_ends.truncate(visible);
        prepared.underlines.retain(|r| end(*r) <= visible as isize);
    }
    let mut layout = build(&host, request, &prepared, base.raw, true)?;
    layout.flat = layout
        .glyphs
        .iter()
        .map(|g| {
            let identity = g.identity.as_ref().map_or("", |s| s.as_str());
            Hp2TextLayoutCoreTextPlacement {
                font: g.font.raw,
                glyph: g.id,
                position: g.position,
                advance: g.advance,
                bounds: g.bounds,
                range: g.range,
                identity: identity.as_ptr(),
                identity_len: identity.len(),
                point_size64: g.point_size64,
                drawable: g.drawable as u8,
                key_glyph: if g.drawable { g.id } else { 0 },
                antialias: g.drawable as u8,
                subpixel: g.drawable as u8,
            }
        })
        .collect();
    layout.start_x = request.start_x;
    layout.start_y = request.start_y;
    layout.clip_x = request.clip_x;
    layout.clip_y = request.clip_y;
    layout.poly_flags = request.poly_flags;
    layout.color = request.color;
    layout.clip = request.clip != 0;
    layout.origin_x = 0.0;
    layout.origin_y = 0.0;
    layout.draw_bridge = core::ptr::null_mut();
    Some(layout)
}
pub unsafe fn role_font(
    api: &Hp2TextLayoutCoreTextServices,
    ctx: *mut c_void,
    role: u32,
    size: f64,
) -> Handle {
    let host = Host { api, ctx };
    match host.role_font(role, size) {
        Some(font) => {
            let raw = font.raw;
            core::mem::forget(font);
            raw
        }
        None => core::ptr::null(),
    }
}
pub fn rectangle(bounds: [f64; 4], drawable: bool, page: i32) -> Option<[f64; 4]> {
    if !bounds.iter().all(|v| v.is_finite()) || bounds[2] <= 0.0 || bounds[3] <= 0.0 {
        return None;
    }
    common::raster_rectangle(
        [
            bounds[0],
            bounds[1],
            bounds[0] + bounds[2],
            bounds[1] + bounds[3],
        ],
        drawable,
        page,
    )
}
pub fn offset(position: [f64; 2], bounds: [f64; 4]) -> [f64; 2] {
    let other = [bounds[0] + bounds[2], bounds[1] + bounds[3]];
    common::draw_offset(
        position,
        [
            bounds[0].min(other[0]),
            bounds[1].min(other[1]),
            bounds[0].max(other[0]),
            bounds[1].max(other[1]),
        ],
    )
}
pub unsafe fn raster(
    api: &Hp2TextLayoutCoreTextServices,
    ctx: *mut c_void,
    font: Handle,
    glyph: u16,
    bounds: [f64; 4],
    drawable: bool,
    page: i32,
) -> Option<(i32, i32, Vec<u8>)> {
    let rect = rectangle(bounds, drawable, page)?;
    let (width, height) = (rect[2] as i32, rect[3] as i32);
    let len = (width as usize).checked_mul(height as usize)?;
    let bytes = len.checked_mul(4)?;
    let mut rgba = vec![0u8; bytes];
    let ok = unsafe {
        (api.raster)(
            ctx,
            font,
            glyph,
            width,
            height,
            rect[0],
            rect[1],
            1,
            1,
            1,
            0,
            rgba.as_mut_ptr(),
        )
    };
    if ok == 0 {
        return None;
    }
    // Compact in place, avoiding a second pixel allocation.
    for index in 0..len {
        rgba[index] = rgba[index * 4 + 3];
    }
    rgba.truncate(len);
    Some((width, height, rgba))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raster_rejects_raw_empty_and_negative_sizes_before_rounding() {
        assert!(rectangle([0.2, 0.2, 0.0, 1.0], true, 1024).is_none());
        assert!(rectangle([0.2, 0.2, -0.1, 1.0], true, 1024).is_none());
        assert!(rectangle([0.2, 0.2, 1.0, -0.1], true, 1024).is_none());
        assert!(rectangle([0.0, 0.0, 1023.0, 1.0], true, 1024).is_none());
        assert_eq!(
            rectangle([-0.25, -1.5, 2.5, 3.0], true, 1024),
            Some([-1.0, -2.0, 4.0, 4.0])
        );
    }

    #[test]
    fn identity_preserves_native_six_decimal_key_format() {
        let info = Hp2TextLayoutCoreTextFontInfo {
            postscript: b"Times-Roman".as_ptr(),
            postscript_len: 11,
            descriptor: b"Times".as_ptr(),
            descriptor_len: 5,
            size: 14.0,
            matrix: [1.0, 0.0, -0.0, 1.0, 0.0, 0.0],
        };
        assert_eq!(
            identity(&info),
            "Times-Roman|Times|14.000000|1.000000,0.000000,-0.000000,1.000000,0.000000,0.000000"
        );
    }

    #[test]
    fn cluster_source_end_preserves_supplementary_scalar_mapping() {
        let prepared = Prepared {
            text: vec![0xd83d, 0xde00],
            source_ends: vec![2, 2],
            underlines: Vec::new(),
        };
        let range = Range {
            location: 0,
            length: 2,
        };
        let mut layout = Layout::default();
        assert_eq!(add_cluster(&mut layout, range, &prepared), Some(()));
        assert_eq!(add_cluster(&mut layout, range, &prepared), Some(()));
        assert_eq!(layout.clusters.len(), 1);
        assert_eq!(layout.clusters[0].source_end, 2);
        assert_eq!(
            common::visible_end(
                layout.clusters.iter().map(|c| (c.range, c.source_end)),
                1,
                2
            ),
            0
        );
    }
}
