//! Replays `fixtures/canvas_text_replay.txt` (generated from the ORIGINAL
//! `UnCanvas.cpp` text paths, compiled standalone against `git archive 40bde73`)
//! through `hp2_engine::canvas_text` with a fake host. Every result is compared
//! bit-for-bit, and every device call (`lock`/`unlock`/`draw`/`log`) must match
//! the original trace in order.

use std::ffi::c_void;

use hp2_engine::canvas::Hp2CanvasTileRect;
use hp2_engine::canvas_text::{
    CanvasTextHost, DrawStringArgs, Font, Hp2CanvasSize, Hp2CanvasTileDraw, Hp2CanvasView,
    Hp2FontChar, Hp2NativeTextRequest, Hp2Plane, Texture, draw_string, get_char_size,
    has_safe_request_floats, native_text_size, text_scale_from, wrapped_print,
};

const FIXTURE: &str = include_str!("fixtures/canvas_text_replay.txt");

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

impl CanvasTextHost for Fake {
    fn ui_scale(&mut self) -> f32 {
        self.ui.unwrap_or(1.0)
    }

    fn language_is_int(&mut self) -> bool {
        true
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

    fn draw_tile_texture(&mut self, _cv: &Hp2CanvasView, _draw: &Hp2CanvasTileDraw) {
        // Not reached by the fixture (DrawPattern/DrawIcon/exec tiles are not replayed).
        self.dev.push("draw-texture".into());
    }

    fn texture_size(&mut self, _tex: Texture) -> Hp2CanvasSize {
        Hp2CanvasSize::default()
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

/// Parses the 21 tokens following `cv`, in the harness `PrintCanvas` order:
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

#[test]
fn canvas_text_matches_original_trace() {
    let fonts = parse_fonts(FIXTURE);
    assert_eq!(fonts.len(), 3, "three fonts in the fixture");
    let mut fake = Fake {
        fonts,
        ..Fake::default()
    };

    let lines: Vec<&str> = FIXTURE.lines().collect();
    let mut i = 0usize;
    let mut counts = [0usize; 5];
    while i < lines.len() {
        let line = lines[i];
        if !line.starts_with("case ") {
            i += 1;
            continue;
        }
        let t: Vec<&str> = line.split_whitespace().collect();
        fake.dev.clear();
        match t[1] {
            "gcs" => {
                // case gcs <font> <ch> <scale bits>
                counts[0] += 1;
                let f = int(t[2]) as usize;
                let ch = int(t[3]) as u16;
                let scale = hex_f32(t[4]);
                let res: Vec<&str> = lines[i + 1].split_whitespace().collect();
                let (w, h) = get_char_size(&mut fake, font_ptr(f), ch, scale);
                assert_eq!(w.to_string(), res[1], "gcs width line {}", i + 1);
                assert_eq!(h.to_string(), res[2], "gcs height line {}", i + 1);
                i += 2;
            }
            "gts" => {
                // case gts cv <21 tokens>
                counts[1] += 1;
                let cvt = &t[3..];
                let cv = parse_cv(cvt);
                fake.ui = parse_ui(cvt);
                let res: Vec<&str> = lines[i + 1].split_whitespace().collect();
                let ui = parse_ui(cvt).unwrap_or(1.0);
                let ts = text_scale_from(&cv, ui);
                let safe = has_safe_request_floats(&cv, ts);
                assert_eq!(fb(ts), res[1], "gts scale line {}", i + 1);
                assert_eq!(
                    u8::from(safe).to_string(),
                    res[2],
                    "gts safe line {}",
                    i + 1
                );
                i += 2;
            }
            "ds" => {
                // case ds <font> <flags> <clip> <amp> <dx> <dy> cv <21 tokens>
                counts[2] += 1;
                let f = int(t[2]) as usize;
                let flags = int(t[3]) as u32;
                let clip = int(t[4]) != 0;
                let amp = int(t[5]) != 0;
                let dx = int(t[6]);
                let dy = int(t[7]);
                let cvt = &t[9..];
                let cv = parse_cv(cvt);
                fake.ui = parse_ui(cvt);
                let text = parse_text(lines[i + 1]);
                let res = lines[i + 2].strip_prefix("res ").expect("res");
                let ret = draw_string(
                    &mut fake,
                    &cv,
                    &DrawStringArgs {
                        flags,
                        font: font_ptr(f),
                        x: dx,
                        y: dy,
                        color: Hp2Plane {
                            x: 0.25,
                            y: 0.5,
                            z: 0.75,
                            w: 1.0,
                        },
                        clip,
                        amp,
                    },
                    &text,
                );
                assert_eq!(ret.to_string(), res, "ds result line {}", i + 2);
                i = check_dev(&lines, i + 3, &fake.dev, line);
            }
            "wp" => {
                // case wp <font> <style> <center> <numChars> cv <21 tokens>
                counts[3] += 1;
                let f = int(t[2]) as usize;
                let style = int(t[3]);
                let center = int(t[4]) != 0;
                let num_chars = int(t[5]);
                let cvt = &t[7..];
                let mut cv = parse_cv(cvt);
                cv.style = style;
                fake.ui = parse_ui(cvt);
                let text = parse_text(lines[i + 1]);
                let res: Vec<&str> = lines[i + 2].split_whitespace().collect();
                let (xl, yl) = wrapped_print(
                    &mut fake,
                    &mut cv,
                    font_ptr(f),
                    style,
                    center,
                    &text,
                    num_chars,
                );
                assert_eq!(xl.to_string(), res[1], "wp XL line {}", i + 2);
                assert_eq!(yl.to_string(), res[2], "wp YL line {}", i + 2);
                assert_eq!(fb(cv.cur_x), res[3], "wp CurX line {}", i + 2);
                assert_eq!(fb(cv.cur_y), res[4], "wp CurY line {}", i + 2);
                i = check_dev(&lines, i + 3, &fake.dev, line);
            }
            "ts" => {
                // case ts <font> cv <21 tokens>
                counts[4] += 1;
                let f = int(t[2]) as usize;
                let cvt = &t[4..];
                let mut cv = parse_cv(cvt);
                cv.font = font_ptr(f);
                fake.ui = parse_ui(cvt);
                let text = parse_text(lines[i + 1]);
                let res: Vec<&str> = lines[i + 2].split_whitespace().collect();
                let (xl, yl) =
                    native_text_size(&mut fake, &mut cv, &text).expect("TextSize with a font");
                assert_eq!(fb(xl as f32), res[1], "ts XL line {}", i + 2);
                assert_eq!(fb(yl as f32), res[2], "ts YL line {}", i + 2);
                i = check_dev(&lines, i + 3, &fake.dev, line);
            }
            other => panic!("unknown case kind {other}"),
        }
    }
    assert_eq!(counts, [67, 67, 67, 133, 66], "case counts");
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
