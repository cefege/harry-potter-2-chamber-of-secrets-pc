//! FreeType provider policy. Native callbacks are SDK reads/operations only;
//! face identity, lifetime, fallback, segmentation and all layout stay here.
use crate::native_text_layout::{self as common, Prepared, Range};
use core::ffi::c_void;
use std::collections::{BTreeMap, HashMap};
use std::io::Write as _;
use std::sync::{LazyLock, Mutex};

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Hp2TextLayoutFreeTypeMetrics {
    pub ascent: f64,
    pub descent: f64,
    pub height: f64,
    pub underline_position: f64,
    pub underline_thickness: f64,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Hp2TextLayoutFreeTypeGlyph {
    pub glyph: u32,
    pub cluster: u32,
    pub x_advance: i32,
    pub y_advance: i32,
    pub x_offset: i32,
    pub y_offset: i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2TextLayoutFreeTypeBitmap {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub rows: u32,
    pub pitch: i32,
    pub gray: u32,
    pub data: *const u8,
}
impl Default for Hp2TextLayoutFreeTypeBitmap {
    fn default() -> Self {
        Self {
            left: 0,
            top: 0,
            width: 0,
            rows: 0,
            pitch: 0,
            gray: 0,
            data: core::ptr::null(),
        }
    }
}
#[repr(C)]
pub struct Hp2TextLayoutFreeTypeServices {
    pub context: *mut c_void,
    pub init: unsafe extern "C" fn(*mut c_void) -> i32,
    pub resolve: unsafe extern "C" fn(
        *mut c_void,
        *const u8,
        usize,
        u32,
        f64,
        u32,
        u32,
        *mut *const u8,
        *mut usize,
        *mut i32,
    ) -> i32,
    pub open: unsafe extern "C" fn(
        *mut c_void,
        *const u8,
        usize,
        i32,
        u32,
        *mut usize,
        *mut Hp2TextLayoutFreeTypeMetrics,
    ) -> i32,
    pub close: unsafe extern "C" fn(*mut c_void, usize) -> i32,
    pub coverage: unsafe extern "C" fn(*mut c_void, usize, *const u32, usize, *mut u32) -> i32,
    pub shape: unsafe extern "C" fn(
        *mut c_void,
        usize,
        *const u16,
        usize,
        usize,
        usize,
        *mut *const Hp2TextLayoutFreeTypeGlyph,
        *mut usize,
    ) -> i32,
    pub render: unsafe extern "C" fn(
        *mut c_void,
        usize,
        *const u32,
        usize,
        *mut Hp2TextLayoutFreeTypeBitmap,
    ) -> i32,
    pub page: unsafe extern "C" fn(
        *mut c_void,
        usize,
        *mut *const u8,
        *mut usize,
        *mut usize,
        *mut usize,
    ) -> i32,
    pub alpha_buffer: unsafe extern "C" fn(*mut c_void, i32, i32, usize, *mut *mut u8) -> i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2TextLayoutFreeTypeRequest {
    pub text: *const c_void,
    pub length: i32,
    pub unit_bytes: u32,
    pub name: *const c_void,
    pub name_length: usize,
    pub family: *const c_void,
    pub family_length: usize,
    pub font_height: i32,
    pub page_count: usize,
    pub has_font: u32,
    pub scale: f32,
    pub space_x: f32,
    pub space_y: f32,
    pub clip_x: f32,
    pub clip_y: f32,
    pub color: [f32; 4],
    pub start_x: i32,
    pub start_y: i32,
    pub visible: i32,
    pub amp: u32,
    pub center: u32,
    pub wrapped: u32,
    pub poly_flags: u32,
    pub clip: u8,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Hp2TextLayoutFreeTypePlacement {
    pub face: usize,
    pub glyph: u32,
    pub size64: u32,
    pub position: [f64; 2],
    pub advance: [f64; 2],
    pub bounds: [f64; 4],
    pub source: common::Hp2TextLayoutRange,
    pub drawable: u32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Hp2TextLayoutFreeTypeCluster {
    pub source: common::Hp2TextLayoutRange,
    pub source_end: i32,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Hp2TextLayoutFreeTypeUnderline {
    pub start: f64,
    pub end: f64,
    pub baseline: f64,
    pub position: f64,
    pub thickness: f64,
}
#[derive(Default)]
pub struct Layout {
    pub width: i32,
    pub height: i32,
    pub ascent: f64,
    pub descent: f64,
    pub glyphs: Vec<Hp2TextLayoutFreeTypePlacement>,
    identities: Vec<usize>,
    pub clusters: Vec<Hp2TextLayoutFreeTypeCluster>,
    pub underlines: Vec<Hp2TextLayoutFreeTypeUnderline>,
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
pub struct Hp2TextLayoutFreeTypeView {
    pub width: i32,
    pub height: i32,
    pub ascent: f64,
    pub descent: f64,
    pub glyphs: *const Hp2TextLayoutFreeTypePlacement,
    pub glyph_count: usize,
    pub clusters: *const Hp2TextLayoutFreeTypeCluster,
    pub cluster_count: usize,
    pub underlines: *const Hp2TextLayoutFreeTypeUnderline,
    pub underline_count: usize,
}
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Hp2TextLayoutFreeTypeFields {
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
    pub fn view(&self) -> Hp2TextLayoutFreeTypeView {
        Hp2TextLayoutFreeTypeView {
            width: self.width,
            height: self.height,
            ascent: self.ascent,
            descent: self.descent,
            glyphs: self.glyphs.as_ptr(),
            glyph_count: self.glyphs.len(),
            clusters: self.clusters.as_ptr(),
            cluster_count: self.clusters.len(),
            underlines: self.underlines.as_ptr(),
            underline_count: self.underlines.len(),
        }
    }
    pub fn identity(&self, index: usize) -> Option<(*const u8, usize)> {
        let id = *self.identities.get(index)?;
        if id == usize::MAX {
            return Some((core::ptr::NonNull::<u8>::dangling().as_ptr(), 0));
        }
        let e = engine();
        let identity = &e.faces.get(id)?.identity;
        // Face identity storage is immutable and owned for the process lifetime.
        Some((identity.as_ptr(), identity.len()))
    }
    pub fn fields(&self) -> Hp2TextLayoutFreeTypeFields {
        Hp2TextLayoutFreeTypeFields {
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
struct Face {
    handle: usize,
    identity: Vec<u8>,
    point: f64,
    size64: u32,
    metrics: Hp2TextLayoutFreeTypeMetrics,
}
#[derive(Default)]
struct Engine {
    initialized: Option<bool>,
    faces: Vec<Face>,
    cache: HashMap<Vec<u8>, HashMap<(i32, u32), usize>>,
}
// Deliberate process lifetime, matching the stable borrowed native face API.
static ENGINE: LazyLock<Mutex<Engine>> = LazyLock::new(|| Mutex::new(Engine::default()));
fn engine() -> std::sync::MutexGuard<'static, Engine> {
    ENGINE.lock().unwrap_or_else(|e| e.into_inner())
}
fn status(value: i32) -> Result<bool, ()> {
    if value < 0 { Err(()) } else { Ok(value != 0) }
}
fn required(value: i32) -> Result<(), ()> {
    if status(value)? { Ok(()) } else { Err(()) }
}
impl Engine {
    fn init(&mut self, s: &Hp2TextLayoutFreeTypeServices) -> Result<bool, ()> {
        if let Some(ok) = self.initialized {
            return Ok(ok);
        }
        let ok = status(unsafe { (s.init)(s.context) })?;
        self.initialized = Some(ok);
        Ok(ok)
    }
    fn acquire(
        &mut self,
        s: &Hp2TextLayoutFreeTypeServices,
        family: &str,
        bold: bool,
        point: f64,
        cp: Option<u32>,
    ) -> Result<Option<usize>, ()> {
        let Some(size64) = common::point_size64(point) else {
            return Ok(None);
        };
        if !self.init(s)? {
            return Ok(None);
        }
        let mut path = core::ptr::null();
        let mut length = 0;
        let mut index = 0;
        if !status(unsafe {
            (s.resolve)(
                s.context,
                family.as_ptr(),
                family.len(),
                bold as u32,
                point,
                cp.unwrap_or(0),
                cp.is_some() as u32,
                &mut path,
                &mut length,
                &mut index,
            )
        })? {
            return Ok(None);
        }
        if path.is_null() || length == 0 {
            return Ok(None);
        }
        let file = unsafe { std::slice::from_raw_parts(path, length) };
        if let Some(&id) = self
            .cache
            .get(file)
            .and_then(|sizes| sizes.get(&(index, size64)))
        {
            return Ok(Some(id));
        }
        // Prepare all Rust allocations before opening the SDK-owned resource.
        // Paths need not be UTF-8: native glyph identities preserve their bytes.
        let mut identity = Vec::with_capacity(file.len() + 24);
        identity.extend_from_slice(file);
        write!(&mut identity, ":{}|{}", index, size64).map_err(|_| ())?;
        self.faces.reserve(1);
        let sizes = self.cache.entry(file.to_vec()).or_default();
        sizes.reserve(1);
        let mut handle = 0;
        let mut metrics = Hp2TextLayoutFreeTypeMetrics::default();
        let opened = status(unsafe {
            (s.open)(
                s.context,
                file.as_ptr(),
                file.len(),
                index,
                size64,
                &mut handle,
                &mut metrics,
            )
        })?;
        if !opened || handle == 0 {
            return Ok(None);
        }
        if ![
            metrics.ascent,
            metrics.descent,
            metrics.height,
            metrics.underline_position,
            metrics.underline_thickness,
        ]
        .iter()
        .all(|v| v.is_finite())
        {
            required(unsafe { (s.close)(s.context, handle) })?;
            return Ok(None);
        }
        let id = self.faces.len();
        self.faces.push(Face {
            handle,
            identity,
            point,
            size64,
            metrics,
        });
        sizes.insert((index, size64), id);
        Ok(Some(id))
    }
    fn role_face(
        &mut self,
        s: &Hp2TextLayoutFreeTypeServices,
        role: u32,
        point: f64,
    ) -> Result<Option<usize>, ()> {
        for family in ["Times New Roman", "serif", "sans-serif"] {
            if let Some(face) = self.acquire(s, family, common::bold(role), point, None)? {
                return Ok(Some(face));
            }
        }
        Ok(None)
    }
    unsafe fn request_face(
        &mut self,
        s: &Hp2TextLayoutFreeTypeServices,
        r: &Hp2TextLayoutFreeTypeRequest,
        role: u32,
        name: &[u32],
    ) -> Result<usize, ()> {
        let scale = if r.scale > 0.0 { r.scale as f64 } else { 1.0 };
        if r.family_length > 0 {
            let family: String = unsafe { common::units(r.family, r.family_length, r.unit_bytes) }
                .into_iter()
                .take_while(|&c| c != 0)
                .map(|c| char::from_u32(c).unwrap_or('\u{fffd}'))
                .collect();
            let height = if r.font_height > 0 {
                r.font_height
            } else {
                common::role_size(8)
            };
            if let Some(face) = self.acquire(s, &family, false, height as f64 * scale, None)? {
                return Ok(face);
            }
        }
        let mut size = common::role_size(role);
        if r.family_length == 0 && !common::known_package(name) {
            size = 0;
            for page in 0..r.page_count {
                let mut data = core::ptr::null();
                let mut count = 0;
                let mut stride = 0;
                let mut offset = 0;
                required(unsafe {
                    (s.page)(
                        s.context,
                        page,
                        &mut data,
                        &mut count,
                        &mut stride,
                        &mut offset,
                    )
                })?;
                if count > 0 && (data.is_null() || stride < offset + 4) {
                    return Err(());
                }
                for i in 0..count {
                    size = size.max(unsafe {
                        core::ptr::read_unaligned(data.add(i * stride + offset).cast::<i32>())
                    });
                }
            }
            if size <= 0 {
                size = common::role_size(2);
            }
        }
        self.role_face(s, role, size as f64 * scale)?.ok_or(())
    }
    fn fallback(
        &mut self,
        s: &Hp2TextLayoutFreeTypeServices,
        cp: u32,
        bold: bool,
        base: usize,
        cache: &mut HashMap<u32, usize>,
    ) -> Result<usize, ()> {
        if let Some(&id) = cache.get(&cp) {
            return Ok(id);
        }
        let point = self.faces[base].point;
        let mut result = base;
        let families: &[&str] = &["serif"];
        for family in families {
            if let Some(id) = self.acquire(s, family, bold, point, Some(cp))? {
                result = id;
                break;
            }
        }
        cache.insert(cp, result);
        Ok(result)
    }
}
fn decode(text: &[u16], index: usize) -> (u32, usize) {
    let a = text[index] as u32;
    if (0xd800..=0xdbff).contains(&a) && index + 1 < text.len() {
        let b = text[index + 1] as u32;
        if (0xdc00..=0xdfff).contains(&b) {
            return (0x10000 + ((a - 0xd800) << 10) + (b - 0xdc00), 2);
        }
    }
    (a, 1)
}
fn attachment(c: u32) -> bool {
    matches!(c,0x0300..=0x036f|0x1ab0..=0x1aff|0x1dc0..=0x1dff|0x20d0..=0x20ff|0xfe00..=0xfe0f|0xfe20..=0xfe2f|0x200d|0xe0100..=0xe01ef)
}
struct Run {
    start: usize,
    end: usize,
    face: usize,
    glyphs: Vec<Hp2TextLayoutFreeTypeGlyph>,
}
impl Engine {
    fn shape(
        &mut self,
        s: &Hp2TextLayoutFreeTypeServices,
        text: &[u16],
        start: usize,
        end: usize,
        base: usize,
        bold: bool,
        cache: &mut HashMap<u32, usize>,
    ) -> Result<Vec<Run>, ()> {
        let mut groups = Vec::new();
        let mut cursor = start;
        while cursor < end {
            let (cp, n) = decode(text, cursor);
            let mut next = cursor + n;
            while next < end {
                let (c, n) = decode(text, next);
                if !attachment(c) {
                    break;
                }
                next += n;
            }
            groups.push((cursor, next, cp));
            cursor = next;
        }
        let scalars: Vec<u32> = groups.iter().map(|g| g.2).collect();
        let mut coverage = vec![0; scalars.len()];
        required(unsafe {
            (s.coverage)(
                s.context,
                self.faces[base].handle,
                scalars.as_ptr(),
                scalars.len(),
                coverage.as_mut_ptr(),
            )
        })?;
        let mut runs: Vec<Run> = Vec::new();
        for ((start, end, cp), covered) in groups.into_iter().zip(coverage) {
            let face = if covered != 0 {
                base
            } else {
                self.fallback(s, cp, bold, base, cache)?
            };
            if let Some(run) = runs.last_mut().filter(|run| run.face == face) {
                run.end = end;
            } else {
                runs.push(Run {
                    start,
                    end,
                    face,
                    glyphs: Vec::new(),
                });
            }
        }
        for run in &mut runs {
            let mut data = core::ptr::null();
            let mut count = 0;
            required(unsafe {
                (s.shape)(
                    s.context,
                    self.faces[run.face].handle,
                    text.as_ptr(),
                    text.len(),
                    run.start,
                    run.end - run.start,
                    &mut data,
                    &mut count,
                )
            })?;
            if count > 0 {
                if data.is_null() {
                    return Err(());
                }
                run.glyphs = unsafe { std::slice::from_raw_parts(data, count) }.to_vec();
                if run
                    .glyphs
                    .iter()
                    .any(|g| (g.cluster as usize) < run.start || (g.cluster as usize) >= run.end)
                {
                    return Err(());
                }
            }
        }
        Ok(runs)
    }
    fn append_line(
        &mut self,
        s: &Hp2TextLayoutFreeTypeServices,
        p: &Prepared,
        r: &Hp2TextLayoutFreeTypeRequest,
        start: usize,
        end: usize,
        base: usize,
        bold: bool,
        capture: bool,
        cache: &mut HashMap<u32, usize>,
        top: &mut f64,
        out: &mut Layout,
    ) -> Result<(), ()> {
        let runs = self.shape(s, &p.text, start, end, base, bold, cache)?;
        let mut advance = 0.0;
        let mut ranges = Vec::new();
        let mut ascent: f64 = 0.0;
        let mut descent: f64 = 0.0;
        let mut leading: f64 = 0.0;
        for face in runs
            .iter()
            .map(|run| run.face)
            .chain(runs.is_empty().then_some(base))
        {
            let m = self.faces[face].metrics;
            ascent = ascent.max(m.ascent);
            descent = descent.max(m.descent);
            leading = leading.max((m.height - m.ascent - m.descent).max(0.0));
        }
        for run in &runs {
            let mut starts: Vec<usize> = run.glyphs.iter().map(|g| g.cluster as usize).collect();
            starts.sort_unstable();
            starts.dedup();
            for (i, &start) in starts.iter().enumerate() {
                let end = starts.get(i + 1).copied().unwrap_or(run.end);
                if end > start {
                    ranges.push(Range {
                        location: start as isize,
                        length: (end - start) as isize,
                    });
                }
            }
            for g in &run.glyphs {
                advance += g.x_advance as f64 / 64.0 + r.space_x as f64;
            }
        }
        ranges.sort_unstable_by_key(|v| (v.location, v.length));
        let next = *top + (ascent + descent + leading + r.space_y as f64).max(0.0);
        if !advance.is_finite()
            || advance > i32::MAX as f64
            || !next.is_finite()
            || next > i32::MAX as f64
        {
            return Err(());
        }
        let origin = if r.center != 0 { -advance * 0.5 } else { 0.0 };
        let baseline = *top + ascent;
        if capture {
            let mut pen = 0.0;
            let mut pens = BTreeMap::new();
            for run in &runs {
                let face = &self.faces[run.face];
                let ids: Vec<u32> = run.glyphs.iter().map(|g| g.glyph).collect();
                let mut bitmaps = vec![Hp2TextLayoutFreeTypeBitmap::default(); ids.len()];
                required(unsafe {
                    (s.render)(
                        s.context,
                        face.handle,
                        ids.as_ptr(),
                        ids.len(),
                        bitmaps.as_mut_ptr(),
                    )
                })?;
                for (glyph, bitmap) in run.glyphs.iter().zip(bitmaps) {
                    let source = *ranges
                        .iter()
                        .find(|v| {
                            glyph.cluster as isize >= v.location
                                && (glyph.cluster as isize) < v.location + v.length
                        })
                        .ok_or(())?;
                    pens.entry(source.location).or_insert(origin + pen);
                    let drawable = bitmap.width != 0 && bitmap.rows != 0;
                    let bounds = if drawable {
                        [
                            bitmap.left as f64,
                            (bitmap.top as f64) - (bitmap.rows as f64),
                            bitmap.left as f64 + bitmap.width as f64,
                            bitmap.top as f64,
                        ]
                    } else {
                        [0.0; 4]
                    };
                    let position = [
                        origin + pen + glyph.x_offset as f64 / 64.0,
                        baseline + glyph.y_offset as f64 / 64.0,
                    ];
                    let x_advance = glyph.x_advance as f64 / 64.0 + r.space_x as f64;
                    if !position.iter().all(|v| v.is_finite()) || !x_advance.is_finite() {
                        return Err(());
                    }
                    out.glyphs.push(Hp2TextLayoutFreeTypePlacement {
                        face: face.handle,
                        glyph: glyph.glyph,
                        size64: if drawable { face.size64 } else { 0 },
                        position,
                        advance: [x_advance, 0.0],
                        bounds,
                        source,
                        drawable: drawable as u32,
                    });
                    out.identities
                        .push(if drawable { run.face } else { usize::MAX });
                    pen += x_advance;
                }
            }
            pens.insert(end as isize, origin + pen);
            for range in &p.underlines {
                let a = (start as isize).max(range.location);
                let b = (end as isize).min(range.location + range.length);
                if a >= b {
                    continue;
                }
                if let (Some((_, &a)), Some((_, &b))) =
                    (pens.range(a..).next(), pens.range(b..).next())
                {
                    if a.is_finite() && b.is_finite() && a != b {
                        let m = self.faces[base].metrics;
                        out.underlines.push(Hp2TextLayoutFreeTypeUnderline {
                            start: a.min(b),
                            end: a.max(b),
                            baseline,
                            position: m.underline_position,
                            thickness: m.underline_thickness.max(1.0),
                        });
                    }
                }
            }
        }
        for source in ranges {
            append_cluster(out, p, source)?;
        }
        out.width = out.width.max(if advance > 0.0 {
            advance.ceil() as i32
        } else {
            0
        });
        out.ascent = out.ascent.max(ascent);
        out.descent = out.descent.max(descent);
        *top = next;
        Ok(())
    }
    fn break_end(
        &mut self,
        s: &Hp2TextLayoutFreeTypeServices,
        p: &Prepared,
        r: &Hp2TextLayoutFreeTypeRequest,
        start: usize,
        end: usize,
        base: usize,
        bold: bool,
        cache: &mut HashMap<u32, usize>,
        available: f64,
    ) -> Result<usize, ()> {
        let runs = self.shape(s, &p.text, start, end, base, bold, cache)?;
        let mut widths: BTreeMap<usize, f64> = BTreeMap::new();
        for run in runs {
            for g in run.glyphs {
                *widths.entry(g.cluster as usize).or_default() +=
                    g.x_advance as f64 / 64.0 + r.space_x as f64;
            }
        }
        let mut cumulative = 0.0;
        let mut candidates = Vec::with_capacity(widths.len());
        let mut widths = widths.into_iter().peekable();
        while let Some((start, width)) = widths.next() {
            cumulative += width;
            let end = widths.peek().map_or(end, |v| v.0);
            candidates.push((start, end, cumulative));
        }
        let last = candidates.last().ok_or(())?;
        if last.2 <= available {
            return Ok(last.1);
        }
        let mut fit = None;
        let mut whitespace = None;
        for &(start, end, width) in &candidates {
            if width > available {
                break;
            }
            fit = Some(end);
            if matches!(p.text[start], 32 | 9) {
                whitespace = Some(end);
            }
        }
        Ok(whitespace.or(fit).unwrap_or(candidates[0].1))
    }
    fn build(
        &mut self,
        s: &Hp2TextLayoutFreeTypeServices,
        p: &Prepared,
        r: &Hp2TextLayoutFreeTypeRequest,
        base: usize,
        bold: bool,
        capture: bool,
    ) -> Result<Layout, ()> {
        let mut out = Layout::default();
        let mut cache = HashMap::new();
        let mut top = 0.0;
        let mut start = 0;
        let available = r.clip_x as f64 - if r.center != 0 { 0.0 } else { r.start_x as f64 };
        while start < p.text.len() {
            let mut end = start;
            while end < p.text.len() && !matches!(p.text[end], 10 | 13) {
                end += 1;
            }
            let mut cursor = start;
            if cursor == end {
                self.append_line(
                    s, p, r, cursor, end, base, bold, capture, &mut cache, &mut top, &mut out,
                )?;
            }
            while cursor < end {
                let line_end = if r.wrapped != 0 {
                    if available <= 0.0 {
                        return Err(());
                    }
                    self.break_end(s, p, r, cursor, end, base, bold, &mut cache, available)?
                } else {
                    end
                };
                if line_end <= cursor || line_end > end {
                    return Err(());
                }
                self.append_line(
                    s, p, r, cursor, line_end, base, bold, capture, &mut cache, &mut top, &mut out,
                )?;
                cursor = line_end;
            }
            start = end;
            if end < p.text.len() {
                start += 1;
                if p.text[end] == 13 && start < p.text.len() && p.text[start] == 10 {
                    start += 1;
                }
                append_cluster(
                    &mut out,
                    p,
                    Range {
                        location: end as isize,
                        length: (start - end) as isize,
                    },
                )?;
                if start == p.text.len() {
                    self.append_line(
                        s, p, r, start, start, base, bold, capture, &mut cache, &mut top, &mut out,
                    )?;
                }
            }
        }
        out.height = top.ceil().max(0.0) as i32;
        out.start_x = r.start_x;
        out.start_y = r.start_y;
        out.clip_x = r.clip_x;
        out.clip_y = r.clip_y;
        out.poly_flags = r.poly_flags;
        out.color = r.color;
        out.clip = r.clip != 0;
        out.origin_x = 0.0;
        out.origin_y = 0.0;
        out.draw_bridge = core::ptr::null_mut();
        Ok(out)
    }
}
fn append_cluster(out: &mut Layout, p: &Prepared, source: Range) -> Result<(), ()> {
    let end = (source.location + source.length) as usize;
    if source.location < 0 || source.length <= 0 || end > p.source_ends.len() {
        return Err(());
    }
    if out.clusters.last().is_some_and(|v| v.source == source) {
        return Ok(());
    }
    out.clusters.push(Hp2TextLayoutFreeTypeCluster {
        source,
        source_end: p.source_ends[end - 1],
    });
    Ok(())
}
pub unsafe fn create(
    s: &Hp2TextLayoutFreeTypeServices,
    r: &Hp2TextLayoutFreeTypeRequest,
) -> Option<Layout> {
    if r.has_font == 0
        || ![
            r.scale, r.space_x, r.space_y, r.clip_x, r.clip_y, r.color[0], r.color[1], r.color[2],
            r.color[3],
        ]
        .into_iter()
        .all(common::safe_float)
    {
        return None;
    }
    let mut p = unsafe { common::preprocess(r.text, r.length, r.unit_bytes, r.amp != 0) }?;
    let name = unsafe { common::units(r.name, r.name_length, r.unit_bytes) };
    let role = common::role(&name, r.family_length > 0);
    let mut e = engine();
    let base = unsafe { e.request_face(s, r, role, &name) }.ok()?;
    if r.visible > 0 && p.source_ends.last().is_some_and(|&v| r.visible < v) {
        let probe = e.build(s, &p, r, base, common::bold(role), false).ok()?;
        let end = common::visible_end(
            probe.clusters.iter().map(|v| (v.source, v.source_end)),
            r.visible,
            p.text.len(),
        );
        p.text.truncate(end);
        p.source_ends.truncate(end);
        p.underlines
            .retain(|v| v.location + v.length <= end as isize);
    }
    e.build(s, &p, r, base, common::bold(role), true).ok()
}
pub fn probe(s: &Hp2TextLayoutFreeTypeServices) -> Result<u32, ()> {
    let mut e = engine();
    let reason = if !e.init(s)? {
        1
    } else if let Some(id) = e.role_face(s, 2, 12.0)? {
        let mut glyph = 0;
        required(unsafe { (s.coverage)(s.context, e.faces[id].handle, &65, 1, &mut glyph) })?;
        let mut bitmap = Hp2TextLayoutFreeTypeBitmap::default();
        if glyph == 0
            || !status(unsafe {
                (s.render)(s.context, e.faces[id].handle, &glyph, 1, &mut bitmap)
            })?
        {
            2
        } else {
            0
        }
    } else {
        1
    };
    Ok(reason)
}
pub fn raster(
    s: &Hp2TextLayoutFreeTypeServices,
    p: &Hp2TextLayoutFreeTypePlacement,
    page: i32,
) -> Option<()> {
    let rect = common::raster_rectangle(p.bounds, p.drawable != 0, page)?;
    if p.face == 0 {
        return None;
    }
    // FreeType exposes a mutable face slot: retain the same lock used by
    // shaping/probing until every borrowed bitmap byte has been copied.
    let _engine = engine();
    let mut bitmap = Hp2TextLayoutFreeTypeBitmap::default();
    required(unsafe { (s.render)(s.context, p.face, &p.glyph, 1, &mut bitmap) }).ok()?;
    let width = rect[2] as usize;
    let height = rect[3] as usize;
    let pitch = bitmap.pitch.unsigned_abs() as usize;
    if bitmap.gray == 0
        || bitmap.width as usize != width
        || bitmap.rows as usize != height
        || pitch < width
        || bitmap.data.is_null()
    {
        return None;
    }
    let length = width.checked_mul(height)?;
    let mut destination = core::ptr::null_mut();
    required(unsafe {
        (s.alpha_buffer)(
            s.context,
            width as i32,
            height as i32,
            length,
            &mut destination,
        )
    })
    .ok()?;
    if destination.is_null() {
        return None;
    }
    for row in 0..height {
        let source = if bitmap.pitch >= 0 {
            row
        } else {
            height - 1 - row
        };
        let offset = source.checked_mul(pitch)?;
        unsafe {
            core::ptr::copy_nonoverlapping(
                bitmap.data.add(offset),
                destination.add(row * width),
                width,
            )
        };
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coverage_units_keep_supplementary_scalars_and_attachments() {
        let text = [0xd83d, 0xde42, 0x0301, 0x200d, b'A' as u16];
        assert_eq!(decode(&text, 0), (0x1f642, 2));
        assert_eq!(decode(&text, 4), (b'A' as u32, 1));
        for scalar in [
            0x0301, 0x1ab0, 0x1dc0, 0x20d0, 0xfe0f, 0xfe20, 0x200d, 0xe0100,
        ] {
            assert!(attachment(scalar));
        }
        assert!(!attachment(b'A' as u32));
        assert!(!attachment(0x1f642));
    }

    #[test]
    fn clusters_preserve_native_source_ends_without_duplicate_ranges() {
        let prepared = Prepared {
            text: vec![0xd83d, 0xde42, 13, 10],
            source_ends: vec![1, 1, 2, 3],
            underlines: Vec::new(),
        };
        let mut layout = Layout::default();
        let scalar = Range {
            location: 0,
            length: 2,
        };
        append_cluster(&mut layout, &prepared, scalar).unwrap();
        append_cluster(&mut layout, &prepared, scalar).unwrap();
        append_cluster(
            &mut layout,
            &prepared,
            Range {
                location: 2,
                length: 2,
            },
        )
        .unwrap();
        assert_eq!(layout.clusters.len(), 2);
        assert_eq!(layout.clusters[0].source_end, 1);
        assert_eq!(layout.clusters[1].source_end, 3);
        assert_eq!(
            common::visible_end(
                layout.clusters.iter().map(|c| (c.source, c.source_end)),
                2,
                4
            ),
            2
        );
        assert!(
            append_cluster(
                &mut layout,
                &prepared,
                Range {
                    location: 4,
                    length: 1
                }
            )
            .is_err()
        );
    }

    #[test]
    fn native_exception_is_not_an_unavailable_font() {
        assert_eq!(status(-1), Err(()));
        assert_eq!(status(0), Ok(false));
        assert_eq!(status(1), Ok(true));
        assert_eq!(required(0), Err(()));
    }
}
