//! Replays `fixtures/canvas_draw_calls.txt` (generated from the ORIGINAL
//! `UCanvas::DrawTile`/`DrawPattern`/`DrawIcon`/`SetClip` and `execDrawTile`,
//! `execDrawTileClipped`, `execDrawTextClipped` bodies at 40bde73, compiled
//! standalone against a stub engine) through `hp2_engine::canvas_text` with a
//! fake host. Every result is compared bit-for-bit, and every device call
//! (`lock`/`draw`/`unlock`/`log`) must match the original trace in order.

use std::ffi::c_void;

use hp2_engine::canvas::{Hp2CanvasTileRect, clip_tile, uv_factor};
use hp2_engine::canvas_text::{
    CanvasTextHost, Font, Hp2CanvasPatternArgs, Hp2CanvasSize, Hp2CanvasTileDraw, Hp2CanvasView,
    Hp2FontChar, Hp2NativeTextRequest, Hp2Plane, PF_MASKED, Texture, draw_icon, draw_pattern,
    native_draw_text_clipped, native_draw_tile, native_draw_tile_clipped, set_clip,
};

const FIXTURE: &str = include_str!("fixtures/canvas_draw_calls.txt");

/// `FPlane(0.25f, 0.5f, 0.75f, 1.f)` passed as `Col` by the harness.
const COL: Hp2Plane = Hp2Plane {
    x: 0.25,
    y: 0.5,
    z: 0.75,
    w: 1.0,
};

struct FakePage {
    tex: i32,
    chars: Vec<Hp2FontChar>,
}

struct FakeFont {
    cpp: i32,
    remapped: bool,
    map: Vec<(u16, u16)>,
    pages: Vec<FakePage>,
}

/// Device-call recorder standing in for the engine (mirrors `harness.cpp` stubs).
#[derive(Default)]
struct Fake {
    fonts: Vec<FakeFont>,
    slots: [i32; 5],
    ui: Option<f32>,
    int_lang: bool,
    dev: Vec<String>,
}

fn font_ptr(index: usize) -> Font {
    Font((index + 1) as *mut c_void)
}

fn font_index(font: Font) -> usize {
    font.0 as usize - 1
}

fn fb(f: f32) -> String {
    format!("{:08x}", f.to_bits())
}

/// `DWORD ColHash(FPlane)` from `harness.cpp`.
fn col_hash(p: Hp2Plane) -> u32 {
    let mut h: u32 = 2_166_136_261;
    for v in [p.x.to_bits(), p.y.to_bits(), p.z.to_bits(), p.w.to_bits()] {
        h = (h ^ v).wrapping_mul(16_777_619);
        h ^= h >> 13;
    }
    h
}

/// `MakeTex()` in `harness.cpp`: id -> (USize, VSize, PolyFlags).
fn tex_info(id: i32) -> (i32, i32, u32) {
    match id {
        1 => (256, 128, 0),
        2 => (64, 64, 2),
        3 => (128, 32, 0),
        _ => panic!("unknown texture {id}"),
    }
}

impl CanvasTextHost for Fake {
    fn ui_scale(&mut self) -> f32 {
        self.ui.unwrap_or(1.0)
    }

    fn language_is_int(&mut self) -> bool {
        self.int_lang
    }

    fn viewport_draw_string(
        &mut self,
        _cv: &Hp2CanvasView,
        _flags: u32,
        _font: Font,
        _x: &mut i32,
        _y: &mut i32,
        _text: &[u16],
        _color: Hp2Plane,
    ) {
        self.dev.push("viewport-drawstring".into());
    }

    fn try_native_text(
        &mut self,
        _cv: &Hp2CanvasView,
        _req: &Hp2NativeTextRequest,
        _font: Font,
        _text: &[u16],
        _color: Hp2Plane,
    ) -> Option<(i32, i32)> {
        None
    }

    fn font_chars_per_page(&mut self, font: Font) -> i32 {
        self.fonts[font_index(font)].cpp
    }

    fn font_remap_char(&mut self, font: Font, ch: u16) -> u16 {
        let f = &self.fonts[font_index(font)];
        if !f.remapped {
            return ch;
        }
        f.map
            .iter()
            .find(|(from, _)| *from == ch)
            .map_or(32, |(_, to)| *to)
    }

    fn font_page_count(&mut self, font: Font) -> i32 {
        self.fonts[font_index(font)].pages.len() as i32
    }

    fn font_page_texture(&mut self, font: Font, page: i32) -> Texture {
        let tex = self.fonts[font_index(font)].pages[page as usize].tex;
        Texture(tex as usize as *mut c_void)
    }

    fn font_page_char_count(&mut self, font: Font, page: i32) -> i32 {
        self.fonts[font_index(font)].pages[page as usize]
            .chars
            .len() as i32
    }

    fn font_page_char(&mut self, font: Font, page: i32, index: i32) -> Hp2FontChar {
        self.fonts[font_index(font)].pages[page as usize].chars[index as usize]
    }

    fn font_has_name(&mut self, _font: Font) -> bool {
        false
    }

    fn slot_reset(&mut self) {
        self.slots = [0; 5];
    }

    fn slot_texture(&mut self, slot: usize) -> Texture {
        Texture(self.slots[slot] as usize as *mut c_void)
    }

    fn slot_lock(&mut self, _cv: &Hp2CanvasView, slot: usize, tex: Texture) {
        let id = tex.0 as usize as i32;
        self.slots[slot] = id;
        self.dev.push(format!("lock {id} 0"));
    }

    fn slot_unlock(&mut self, slot: usize) {
        // Engine UTexture::Unlock is empty: the slot keeps its Texture.
        self.dev.push(format!("unlock {}", self.slots[slot]));
    }

    fn draw_tile_slot(
        &mut self,
        cv: &Hp2CanvasView,
        slot: usize,
        rect: &Hp2CanvasTileRect,
        color: Hp2Plane,
        flags: u32,
    ) {
        let id = self.slots[slot];
        let shown = if id == 0 { -1 } else { id };
        self.dev.push(format!(
            "draw {shown} {} {} {} {} {} {} {} {} {} {:08x} {:08x}",
            fb(rect.x),
            fb(rect.y),
            fb(rect.xl),
            fb(rect.yl),
            fb(rect.u),
            fb(rect.v),
            fb(rect.ul),
            fb(rect.vl),
            fb(cv.z),
            col_hash(color),
            flags
        ));
    }

    /// `UCanvas::DrawTile` (40bde73 `UnCanvas.cpp:67-120`): reject/clip, then
    /// `Lock(-1)`, UV scale, `RenDev->DrawTile` with `PF_Masked` ORed from the
    /// texture, then `Unlock`.
    fn draw_tile_texture(&mut self, cv: &Hp2CanvasView, draw: &Hp2CanvasTileDraw) {
        let mut rect = draw.rect;
        if !clip_tile(cv.frame_fx, cv.frame_fy, &mut rect) {
            return;
        }
        let id = draw.tex as usize as i32;
        let (us, vs, pf) = tex_info(id);
        self.dev.push(format!("lock {id} -1"));
        let uf = uv_factor(1.0, us, us);
        let vf = uv_factor(1.0, vs, vs);
        rect.u *= uf;
        rect.ul *= uf;
        rect.v *= vf;
        rect.vl *= vf;
        self.dev.push(format!(
            "draw {id} {} {} {} {} {} {} {} {} {} {:08x} {:08x}",
            fb(rect.x),
            fb(rect.y),
            fb(rect.xl),
            fb(rect.yl),
            fb(rect.u),
            fb(rect.v),
            fb(rect.ul),
            fb(rect.vl),
            fb(draw.z),
            col_hash(draw.color),
            draw.flags | (pf & PF_MASKED)
        ));
        self.dev.push(format!("unlock {id}"));
    }

    fn texture_size(&mut self, tex: Texture) -> Hp2CanvasSize {
        let (w, h, _) = tex_info(tex.0 as usize as i32);
        Hp2CanvasSize { w, h }
    }

    fn log(&mut self, code: u32) {
        self.dev.push(format!("log {code}"));
    }
}

fn hex_f32(s: &str) -> f32 {
    f32::from_bits(u32::from_str_radix(s, 16).expect("hex float bits"))
}

fn int(s: &str) -> i32 {
    s.parse().expect("int")
}

/// The 21 tokens of a `cv` record (`PrintCanvas`), given the line that starts with `cv`
/// or the token slice after the `cv` marker.
fn cv_tokens(line: &str) -> Vec<&str> {
    let t: Vec<&str> = line.split_whitespace().collect();
    assert_eq!(t[0], "cv", "cv record");
    assert_eq!(t.len(), 22, "cv token count");
    t[1..].to_vec()
}

/// Parses the 21 tokens of a `cv` record, in the harness `PrintCanvas` order:
/// 9 float bits, FrameX/FrameY, FrameFX/FrameFY bits, UI bits or `-`,
/// R G B A, FontL FontB FontM (-1 = null).
fn parse_cv(t: &[&str]) -> Hp2CanvasView {
    assert_eq!(t.len(), 21, "cv token count");
    let font_of = |s: &str| -> Font {
        let v = int(s);
        if v < 0 {
            Font(std::ptr::null_mut())
        } else {
            font_ptr(v as usize)
        }
    };
    let (r, g, b, a) = (int(t[14]), int(t[15]), int(t[16]), int(t[17]));
    Hp2CanvasView {
        font: Font(std::ptr::null_mut()),
        large_font: font_of(t[18]),
        big_font: font_of(t[19]),
        med_font: font_of(t[20]),
        cur_x: hex_f32(t[6]),
        cur_y: hex_f32(t[7]),
        cur_yl: 0.0,
        clip_x: hex_f32(t[4]),
        clip_y: hex_f32(t[5]),
        org_x: hex_f32(t[2]),
        org_y: hex_f32(t[3]),
        space_x: hex_f32(t[0]),
        space_y: hex_f32(t[1]),
        z: hex_f32(t[8]),
        x: 0,
        frame_x: int(t[9]),
        frame_y: int(t[10]),
        frame_fx: hex_f32(t[11]),
        frame_fy: hex_f32(t[12]),
        has_frame: 1,
        style: 0,
        center: 0,
        no_smooth: 0,
        color: Hp2Plane {
            x: r as f32 / 255.0,
            y: g as f32 / 255.0,
            z: b as f32 / 255.0,
            w: a as f32 / 255.0,
        },
    }
}

/// UI scale for the case: `-` means no client (scale stays 1).
fn parse_ui(t: &[&str]) -> Option<f32> {
    if t[13] == "-" {
        None
    } else {
        Some(hex_f32(t[13]))
    }
}

/// Decodes `text` tokens (`-` = empty), appending the NUL terminator.
fn parse_text(line: &str) -> Vec<u16> {
    let body = line.strip_prefix("text ").expect("text line");
    let mut out: Vec<u16> = if body == "-" {
        Vec::new()
    } else {
        body.split(',').map(|s| int(s) as u16).collect()
    };
    out.push(0);
    out
}

fn parse_fonts(fixture: &str) -> Vec<FakeFont> {
    let mut fonts: Vec<FakeFont> = Vec::new();
    for line in fixture.lines().filter(|l| l.starts_with("font ")) {
        let t: Vec<&str> = line.split_whitespace().collect();
        let fid = int(t[1]) as usize;
        let cpp = int(t[2]);
        let remapped = int(t[3]) != 0;
        let nmap = int(t[4]) as usize;
        let mut i = 5;
        let mut map = Vec::new();
        for _ in 0..nmap {
            map.push((int(t[i]) as u16, int(t[i + 1]) as u16));
            i += 2;
        }
        let npages = int(t[i]) as usize;
        i += 1;
        let mut pages = Vec::new();
        for _ in 0..npages {
            let tex = int(t[i]);
            let nchars = int(t[i + 1]) as usize;
            i += 2;
            let mut chars = Vec::new();
            for _ in 0..nchars {
                chars.push(Hp2FontChar {
                    start_u: int(t[i]),
                    start_v: int(t[i + 1]),
                    u_size: int(t[i + 2]),
                    v_size: int(t[i + 3]),
                });
                i += 4;
            }
            pages.push(FakePage { tex, chars });
        }
        assert_eq!(i, t.len(), "font line {fid} fully consumed");
        while fonts.len() <= fid {
            fonts.push(FakeFont {
                cpp: 0,
                remapped: false,
                map: Vec::new(),
                pages: Vec::new(),
            });
        }
        fonts[fid] = FakeFont {
            cpp,
            remapped,
            map,
            pages,
        };
    }
    fonts
}

/// Compares the `dev` lines that follow a case with the recorded device calls.
/// Returns the index of the first line after them.
fn check_dev(lines: &[&str], mut i: usize, got: &[String], case: &str) -> usize {
    let mut expected: Vec<&str> = Vec::new();
    while i < lines.len() && lines[i].starts_with("dev ") {
        expected.push(&lines[i]["dev ".len()..]);
        i += 1;
    }
    let got: Vec<&str> = got.iter().map(String::as_str).collect();
    if let Some(k) = (0..got.len().max(expected.len())).find(|&k| got.get(k) != expected.get(k)) {
        panic!(
            "device trace for {case}: first difference at call {k}: got {:?}, want {:?} (got {} calls, want {})",
            got.get(k),
            expected.get(k),
            got.len(),
            expected.len()
        );
    }
    i
}

#[test]
fn canvas_draw_calls_match_original_trace() {
    let fonts = parse_fonts(FIXTURE);
    assert_eq!(fonts.len(), 3, "three fonts in the fixture");
    let mut fake = Fake {
        fonts,
        ..Fake::default()
    };

    let lines: Vec<&str> = FIXTURE.lines().collect();
    let mut i = 0usize;
    // pat, icn, clp, dt, dtc, dtx
    let mut counts = [0usize; 6];
    while i < lines.len() {
        let line = lines[i];
        if !line.starts_with("case ") {
            i += 1;
            continue;
        }
        let t: Vec<&str> = line.split_whitespace().collect();
        fake.dev.clear();
        match t[1] {
            "pat" => {
                // case pat <tex> X Y XL YL Scale OrgX OrgY Z flags / cv / res / dev*
                counts[0] += 1;
                let cv = parse_cv(&cv_tokens(lines[i + 1]));
                fake.ui = parse_ui(&cv_tokens(lines[i + 1]));
                let args = Hp2CanvasPatternArgs {
                    tex: int(t[2]) as usize as *mut c_void,
                    x: hex_f32(t[3]),
                    y: hex_f32(t[4]),
                    xl: hex_f32(t[5]),
                    yl: hex_f32(t[6]),
                    scale: hex_f32(t[7]),
                    org_x: hex_f32(t[8]),
                    org_y: hex_f32(t[9]),
                    span: std::ptr::null_mut(),
                    z: hex_f32(t[10]),
                    color: COL,
                    fog: Hp2Plane::ZERO,
                    flags: int(t[11]) as u32,
                };
                assert_eq!(lines[i + 2], "res", "pat res line {}", i + 2);
                draw_pattern(&mut fake, &cv, &args);
                i = check_dev(&lines, i + 3, &fake.dev, line);
            }
            "icn" => {
                // case icn <tex> X Y XL YL Z flags / cv / res / dev*
                counts[1] += 1;
                let cv = parse_cv(&cv_tokens(lines[i + 1]));
                fake.ui = parse_ui(&cv_tokens(lines[i + 1]));
                let args = Hp2CanvasPatternArgs {
                    tex: int(t[2]) as usize as *mut c_void,
                    x: hex_f32(t[3]),
                    y: hex_f32(t[4]),
                    xl: hex_f32(t[5]),
                    yl: hex_f32(t[6]),
                    scale: 0.0,
                    org_x: 0.0,
                    org_y: 0.0,
                    span: std::ptr::null_mut(),
                    z: hex_f32(t[7]),
                    color: COL,
                    fog: Hp2Plane::ZERO,
                    flags: int(t[8]) as u32,
                };
                assert_eq!(lines[i + 2], "res", "icn res line {}", i + 2);
                draw_icon(&mut fake, &cv, &args);
                i = check_dev(&lines, i + 3, &fake.dev, line);
            }
            "clp" => {
                // case clp X Y XL YL cv <21> / res <6 bits>
                counts[2] += 1;
                let mut cv = parse_cv(&cv_tokens(&format!(
                    "cv {}",
                    line.split(" cv ").nth(1).expect("cv")
                )));
                fake.ui = parse_ui(&cv_tokens(&format!(
                    "cv {}",
                    line.split(" cv ").nth(1).expect("cv")
                )));
                set_clip(&mut cv, int(t[2]), int(t[3]), int(t[4]), int(t[5]));
                let r: Vec<&str> = lines[i + 1].split_whitespace().collect();
                assert_eq!(r[0], "res", "clp res line {}", i + 1);
                assert_eq!(fb(cv.cur_x), r[1], "clp CurX line {}", i + 1);
                assert_eq!(fb(cv.cur_y), r[2], "clp CurY line {}", i + 1);
                assert_eq!(fb(cv.org_x), r[3], "clp OrgX line {}", i + 1);
                assert_eq!(fb(cv.org_y), r[4], "clp OrgY line {}", i + 1);
                assert_eq!(fb(cv.clip_x), r[5], "clp ClipX line {}", i + 1);
                assert_eq!(fb(cv.clip_y), r[6], "clp ClipY line {}", i + 1);
                i = check_dev(&lines, i + 2, &fake.dev, line);
            }
            "dt" | "dtc" => {
                // case dt|dtc <tex> <style> XL YL U V UL VL <noSmooth> / cv / res / dev*
                let is_clipped = t[1] == "dtc";
                if is_clipped {
                    counts[4] += 1;
                } else {
                    counts[3] += 1;
                }
                let mut cv = parse_cv(&cv_tokens(lines[i + 1]));
                fake.ui = parse_ui(&cv_tokens(lines[i + 1]));
                cv.style = int(t[3]);
                cv.no_smooth = int(t[10]) as u8;
                let tex = int(t[2]) as usize as *mut c_void;
                let rect = Hp2CanvasTileRect {
                    x: 0.0,
                    y: 0.0,
                    xl: hex_f32(t[4]),
                    yl: hex_f32(t[5]),
                    u: hex_f32(t[6]),
                    v: hex_f32(t[7]),
                    ul: hex_f32(t[8]),
                    vl: hex_f32(t[9]),
                };
                if is_clipped {
                    native_draw_tile_clipped(&mut fake, &mut cv, tex, rect);
                } else {
                    native_draw_tile(&mut fake, &mut cv, tex, rect);
                }
                let r: Vec<&str> = lines[i + 2].split_whitespace().collect();
                assert_eq!(r[0], "res", "{} res line {}", t[1], i + 2);
                assert_eq!(fb(cv.cur_x), r[1], "{} CurX line {}", t[1], i + 2);
                assert_eq!(fb(cv.cur_yl), r[2], "{} CurYL line {}", t[1], i + 2);
                assert_eq!(fb(cv.cur_y), r[3], "{} CurY line {}", t[1], i + 2);
                i = check_dev(&lines, i + 3, &fake.dev, line);
            }
            "dtx" => {
                // case dtx <intLang> <style> <checkHotKey> <fontSel> cv <21> / text / res <font> <CurX> <CurY> / dev*
                counts[5] += 1;
                let cvt = &t[7..];
                let mut cv = parse_cv(cvt);
                fake.ui = parse_ui(cvt);
                fake.int_lang = int(t[2]) != 0;
                cv.style = int(t[3]);
                let check = int(t[4]) != 0;
                let sel = int(t[5]) as usize;
                let cand = [cv.large_font, cv.big_font, cv.med_font];
                // `U->Font = cand[sel] ? cand[sel] : U->MedFont`.
                cv.font = if cand[sel].0.is_null() {
                    cv.med_font
                } else {
                    cand[sel]
                };
                let text = parse_text(lines[i + 1]);
                let r: Vec<&str> = lines[i + 2].split_whitespace().collect();
                assert_eq!(r[0], "res", "dtx res line {}", i + 2);
                native_draw_text_clipped(&mut fake, &mut cv, &text, check);
                let fi = (0..3)
                    .find(|&k| std::ptr::eq(cv.font.0, font_ptr(k).0))
                    .map_or(-1, |k| k as i32);
                assert_eq!(fi.to_string(), r[1], "dtx font line {}", i + 2);
                assert_eq!(fb(cv.cur_x), r[2], "dtx CurX line {}", i + 2);
                assert_eq!(fb(cv.cur_y), r[3], "dtx CurY line {}", i + 2);
                i = check_dev(&lines, i + 3, &fake.dev, line);
            }
            other => panic!("unknown case kind {other}"),
        }
    }
    assert_eq!(counts, [150, 150, 150, 150, 150, 150], "case counts");
}
