//! Line-by-line port of `HarryPotter2/Unreal/Import/Src/HP2ImportIni.cpp`.
//!
//! Scope: the importer's two public entry points, [`build_default_ini`] and
//! [`build_def_user_ini`], and every private helper they use. Error strings,
//! control flow and the generated `System/Default.ini`/`System/DefUser.ini`
//! bytes are kept byte-identical to the C++ this replaces (the deterministic
//! overlay-manifest/importer-parity gates depend on exact text).
//!
//! Deviation from the C++, documented rather than silently approximated:
//! [`lower`]'s non-ASCII path uses Rust's standard `str::to_lowercase`
//! instead of the C++ side's `utf8proc` `CASEFOLD` table. Every section/key
//! name this module compares case-insensitively is ASCII in every `.ini`
//! template and retail file in this repository, so the two algorithms never
//! observably differ here; this crate does not pull in a casefold table to
//! cover an unexercised path.

use std::ffi::CStr;
use std::io::Read as _;

type Lines = Vec<String>;

/// Mirrors `HarryPotter2/Unreal/Import/Inc/HP2Import.h`'s
/// `enum class Profile { Safe, Full, RetailOnly };` — same discriminants,
/// read from C++ across FFI as a `u32`.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Profile {
    Safe = 0,
    Full = 1,
    RetailOnly = 2,
}

impl Profile {
    /// Decodes the `u32` the C++ side sends. `None` on an out-of-range value;
    /// the caller treats that as a bug, not user input.
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Profile::Safe),
            1 => Some(Profile::Full),
            2 => Some(Profile::RetailOnly),
            _ => None,
        }
    }
}

/// Builds `System/Default.ini`. Port of `BuildDefaultIni`.
pub fn build_default_ini(
    template_path: &str,
    retail_path: &str,
    profile: Profile,
) -> Result<String, String> {
    let mut lines = decode_ini(template_path)?;
    let retail = decode_ini(retail_path)?;
    if profile != Profile::Safe {
        remove(&mut lines, "Engine.Engine", "EASplashWaitTime", false)?;
        remove(&mut lines, "Engine.Engine", "WBSplashWaitTime", false)?;
        for key in [
            "EASplashWaitTime",
            "WBSplashWaitTime",
            "LaunchCode",
            "UseSaveSlot",
        ] {
            let value = retail_value(&retail, "FrontEnd", key)?;
            set(&mut lines, "FrontEnd", key, &value)?;
        }
    }
    if profile == Profile::RetailOnly {
        remove_section(&mut lines, "D3D11Drv.D3D11RenderDevice");
        remove_section(&mut lines, "Editor.EditorEngine");
    }
    let engine: [(&str, &str); 6] = [
        ("GameRenderDevice", "XOpenGLDrv.XOpenGLRenderDevice"),
        ("AudioDevice", "ALAudio.ALAudioSubsystem"),
        ("WindowedRenderDevice", "XOpenGLDrv.XOpenGLRenderDevice"),
        ("RenderDevice", "XOpenGLDrv.XOpenGLRenderDevice"),
        ("ViewportManager", "SDLDrv.SDLClient"),
        (
            "Language",
            if profile == Profile::Safe {
                "int"
            } else {
                "usa"
            },
        ),
    ];
    for (key, value) in engine {
        set(&mut lines, "Engine.Engine", key, value)?;
    }
    set(
        &mut lines,
        "Engine.GameEngine",
        "FrameRateLimit",
        "144.000000",
    )?;
    remove(&mut lines, "Engine.GameEngine", "ServerActors", true)?;
    set(&mut lines, "Core.System", "SavePath", "../Save")?;
    set(&mut lines, "Core.System", "CachePath", "../Cache")?;
    let paths = [
        "../System/*.u",
        "../Maps/*.unr",
        "../Textures/*.utx",
        "../Sounds/*.uax",
        "../Music/*.umx",
        "../save/*.usa",
    ];
    let (start, end) = span(&lines, "Core.System");
    if start == lines.len() {
        return Err("Default.ini template has no [Core.System]".to_string());
    }
    let hits = value_hits(&lines, start, end, "Paths");
    for &i in hits.iter().rev() {
        lines.remove(i);
    }
    let (start, end) = span(&lines, "Core.System");
    let mut insert = end;
    while insert > start + 1 && trim(&lines[insert - 1]).is_empty() {
        insert -= 1;
    }
    for path in paths {
        lines.insert(insert, format!("Paths={path}"));
        insert += 1;
    }
    let sdl: [(&str, &str); 16] = [
        ("WindowedViewportX", "800"),
        ("WindowedViewportY", "600"),
        ("WindowedColorBits", "32"),
        ("FullscreenViewportX", "800"),
        ("FullscreenViewportY", "600"),
        ("FullscreenColorBits", "32"),
        ("Brightness", "0.400000"),
        ("UseJoystick", "True"),
        ("CaptureMouse", "True"),
        ("StartupFullscreen", "False"),
        ("BorderlessWindow", "False"),
        ("UseDesktopResolution", "False"),
        ("AllowUnicodeKeys", "True"),
        ("NativeText", "True"),
        ("AllowCommandQKeys", "True"),
        ("MacKeepAllScreensOn", "False"),
    ];
    for (key, value) in sdl {
        set(&mut lines, "SDLDrv.SDLClient", key, value)?;
    }
    let gl: [(&str, &str); 14] = [
        ("OpenGLVersion", "Core"),
        ("UseBindlessTextures", "False"),
        ("UsePersistentBuffers", "False"),
        ("UseShaderDrawParameters", "False"),
        ("UseBufferInvalidation", "False"),
        ("UseShaderCache", "False"),
        ("DetailTextures", "True"),
        ("MacroTextures", "True"),
        ("UseTrilinear", "True"),
        ("UsePrecache", "True"),
        ("UseVSync", "Off"),
        ("UseAA", "True"),
        ("NumAASamples", "4"),
        ("MaxAnisotropy", "16.000000"),
    ];
    for (key, value) in gl {
        set(&mut lines, "XOpenGLDrv.XOpenGLRenderDevice", key, value)?;
    }
    if profile != Profile::Safe {
        let music_volume = retail_value(&retail, "ALAudio.ALAudioSubsystem", "MusicVolume")?;
        set(
            &mut lines,
            "ALAudio.ALAudioSubsystem",
            "MusicVolume",
            &music_volume,
        )?;
        set(&mut lines, "HGame.baseConsole", "bDebugMode", "False")?;
        set(&mut lines, "HGame.HPConsole", "bDebugMode", "False")?;
    }
    validate_default(&lines, profile)?;
    Ok(encode(lines))
}

/// Builds `System/DefUser.ini`. Port of `BuildDefUserIni`.
pub fn build_def_user_ini(
    template_path: &str,
    retail_path: &str,
    profile: Profile,
) -> Result<String, String> {
    let mut lines = decode_ini(template_path)?;
    set(
        &mut lines,
        "Engine.PlayerPawn",
        "bModernThirdPersonControls",
        "False",
    )?;
    set(
        &mut lines,
        "Engine.PlayerPawn",
        "ObjectDetail",
        "ObjectDetailVeryHigh",
    )?;
    if profile == Profile::Safe {
        return Ok(encode(lines));
    }
    let retail = decode_ini(retail_path)?;
    for key in [
        "Aliases[8]",
        "RightMouse",
        "LeftMouse",
        "Tab",
        "Enter",
        "Ctrl",
        "Alt",
        "Space",
        "Z",
        "Slash",
        "A",
    ] {
        let value = retail_value(&retail, "Engine.Input", key)?;
        set(&mut lines, "Engine.Input", key, &value)?;
    }
    for key in [
        "fDamageMultiplier_Easy",
        "fDamageMultiplier_Medium",
        "fDamageMultiplier_Hard",
    ] {
        let value = retail_value(&retail, "HGame.Harry", key)?;
        set(&mut lines, "HGame.Harry", key, &value)?;
    }
    if profile == Profile::Full {
        for key in ["Joy1", "Joy2", "Joy3", "Joy4", "JoyY", "JoyU"] {
            // Re-decodes the template each iteration, matching the C++ (a
            // pure re-read of an unwritten file; six redundant parses).
            let fallback_template = decode_ini(template_path)?;
            if ini_value(&lines, "Engine.Input", key)?
                != ini_value(&fallback_template, "Engine.Input", key)?
            {
                return Err(format!(
                    "controller fallback was not preserved: [Engine.Input] {key}"
                ));
            }
        }
    }
    validate_defuser(&lines, &retail)?;
    Ok(encode(lines))
}

fn read(path: &str) -> Result<Vec<u8>, String> {
    match std::fs::File::open(path) {
        Ok(mut file) => {
            let mut data = Vec::new();
            if file.read_to_end(&mut data).is_err() {
                return Err(format!("cannot read config {path}: read error"));
            }
            Ok(data)
        }
        Err(err) => {
            let code = err.raw_os_error().unwrap_or(0);
            let message = strerror(code);
            Err(format!(
                "cannot read config {path}: [Errno {code}] {message}: '{path}'"
            ))
        }
    }
}

/// `std::strerror` equivalent: the OS-provided message text, matching the
/// C++ side byte for byte (both ultimately read the same libc table).
fn strerror(code: i32) -> String {
    unsafe {
        let ptr = libc::strerror(code);
        if ptr.is_null() {
            return String::new();
        }
        CStr::from_ptr(ptr).to_string_lossy().into_owned()
    }
}

fn validate_utf8(data: &[u8], path: &str) -> Result<(), String> {
    let mut i = 0usize;
    while i < data.len() {
        let c = data[i];
        if c < 0x80 {
            i += 1;
            continue;
        }
        let n: usize = if (0xc2..=0xdf).contains(&c) {
            2
        } else if (0xe0..=0xef).contains(&c) {
            3
        } else if (0xf0..=0xf4).contains(&c) {
            4
        } else {
            0
        };
        let mut valid = n != 0 && i + n <= data.len();
        let mut invalid_continuation = false;
        let mut j = 1usize;
        while valid && j < n {
            if data[i + j] & 0xc0 != 0x80 {
                valid = false;
                invalid_continuation = true;
            }
            j += 1;
        }
        if valid && n == 3 {
            let d = data[i + 1];
            valid = !((c == 0xe0 && d < 0xa0) || (c == 0xed && d >= 0xa0));
        }
        if valid && n == 4 {
            let d = data[i + 1];
            valid = !((c == 0xf0 && d < 0x90) || (c == 0xf4 && d >= 0x90));
        }
        if !valid {
            // The next two branches both yield "invalid continuation byte"
            // via separate conditions; this mirrors the C++ `ValidateUtf8`
            // control flow line for line rather than collapsing it.
            #[allow(clippy::if_same_then_else)]
            let reason = if n == 0 {
                "invalid start byte"
            } else if i + n > data.len() {
                "unexpected end of data"
            } else if (c == 0xe0 && i + 1 < data.len() && data[i + 1] < 0xa0)
                || (c == 0xed && i + 1 < data.len() && data[i + 1] >= 0xa0)
                || (c == 0xf0 && i + 1 < data.len() && data[i + 1] < 0x90)
                || (c == 0xf4 && i + 1 < data.len() && data[i + 1] >= 0x90)
            {
                "invalid continuation byte"
            } else if invalid_continuation {
                "invalid continuation byte"
            } else {
                "invalid start byte"
            };
            return Err(format!(
                "config is not UTF-8/ASCII {path}: 'utf-8' codec can't decode byte 0x{c:02x} in position {i}: {reason}"
            ));
        }
        i += n;
    }
    Ok(())
}

fn decode_ini(path: &str) -> Result<Lines, String> {
    let mut data = read(path)?;
    if data.len() >= 2
        && ((data[0] == 0xff && data[1] == 0xfe) || (data[0] == 0xfe && data[1] == 0xff))
    {
        return Err(format!(
            "UTF-16 config is unsupported for deterministic merge: {path}"
        ));
    }
    if data.len() >= 3 && data[0] == 0xef && data[1] == 0xbb && data[2] == 0xbf {
        data.drain(0..3);
    }
    validate_utf8(&data, path)?;
    let mut lines = Lines::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < data.len() {
        let c = data[i];
        let delimiter: usize = if c == b'\n' || c == 0x0b || c == 0x0c || (0x1c..=0x1e).contains(&c)
        {
            1
        } else if c == b'\r' {
            if i + 1 < data.len() && data[i + 1] == b'\n' {
                2
            } else {
                1
            }
        } else if i + 1 < data.len() && c == 0xc2 && data[i + 1] == 0x85 {
            2
        } else if i + 2 < data.len()
            && c == 0xe2
            && data[i + 1] == 0x80
            && (data[i + 2] == 0xa8 || data[i + 2] == 0xa9)
        {
            3
        } else {
            0
        };
        if delimiter != 0 {
            // SAFETY: validate_utf8 above confirmed `data` is valid UTF-8;
            // every delimiter recognized here starts and ends on a UTF-8
            // char boundary (ASCII controls, or whole 2/3-byte sequences).
            lines.push(String::from_utf8(data[start..i].to_vec()).expect("validated UTF-8"));
            i += delimiter;
            start = i;
        } else {
            i += 1;
        }
    }
    if start < data.len() {
        lines.push(String::from_utf8(data[start..].to_vec()).expect("validated UTF-8"));
    }
    Ok(lines)
}

fn trim(s: &str) -> &str {
    s.trim_matches(|c| matches!(c, ' ' | '\t' | '\r' | '\n'))
}

/// See the module-level doc comment: ASCII fast path identical to the C++;
/// non-ASCII falls back to `str::to_lowercase` rather than `utf8proc`
/// casefolding, a gap never exercised by this repo's ASCII-only ini keys.
fn lower(s: &str) -> String {
    if s.is_ascii() {
        s.to_ascii_lowercase()
    } else {
        s.to_lowercase()
    }
}

fn ascii_lower_eq(bytes: &[u8], target: &[u8]) -> bool {
    bytes.len() == target.len()
        && bytes
            .iter()
            .zip(target.iter())
            .all(|(&b, &t)| b.to_ascii_lowercase() == t)
}

fn span(lines: &Lines, section: &str) -> (usize, usize) {
    let mut start = lines.len();
    for (i, raw) in lines.iter().enumerate() {
        let line = trim(raw);
        let bytes = line.as_bytes();
        if bytes.len() > 2 && bytes[0] == b'[' && bytes[bytes.len() - 1] == b']' {
            if start != lines.len() {
                return (start, i);
            }
            let inner = trim(&line[1..line.len() - 1]);
            if lower(inner) == lower(section) {
                start = i;
            }
        }
    }
    (start, lines.len())
}

fn ensure_section(lines: &mut Lines, section: &str) {
    if span(lines, section).0 != lines.len() {
        return;
    }
    while !lines.is_empty() && trim(lines.last().unwrap()).is_empty() {
        lines.pop();
    }
    if !lines.is_empty() {
        lines.push(String::new());
    }
    lines.push(format!("[{section}]"));
}

fn value_hits(lines: &Lines, start: usize, end: usize, key: &str) -> Vec<usize> {
    let mut hits = Vec::new();
    for (i, raw) in lines.iter().enumerate().take(end).skip(start + 1) {
        let first = match raw.find(|c: char| c != ' ' && c != '\t') {
            Some(f) => f,
            None => continue,
        };
        let first_byte = raw.as_bytes()[first];
        if first_byte == b';' || first_byte == b'#' {
            continue;
        }
        if let Some(equal) = raw.find('=')
            && equal >= first
        {
            let candidate = lower(trim(&raw[first..equal]));
            if candidate == lower(key) {
                hits.push(i);
            }
        }
    }
    hits
}

fn remove_section(lines: &mut Lines, section: &str) {
    let (mut start, end) = span(lines, section);
    if start == lines.len() {
        return;
    }
    while start > 0 && trim(&lines[start - 1]).is_empty() {
        start -= 1;
    }
    lines.drain(start..end);
}

fn ini_value(lines: &Lines, section: &str, key: &str) -> Result<String, String> {
    let (start, end) = span(lines, section);
    if start == lines.len() {
        return Err(format!("missing config section [{section}]"));
    }
    let mut values = Vec::new();
    for i in value_hits(lines, start, end, key) {
        let equal = lines[i].find('=').expect("value_hits guarantees '='");
        values.push(trim(&lines[i][equal + 1..]).to_string());
    }
    if values.len() != 1 {
        return Err(format!(
            "expected one [{section}] {key}, found {}",
            values.len()
        ));
    }
    Ok(values.into_iter().next().unwrap())
}

fn set(lines: &mut Lines, section: &str, key: &str, value: &str) -> Result<(), String> {
    ensure_section(lines, section);
    let (start, end) = span(lines, section);
    let hits = value_hits(lines, start, end, key);
    if hits.len() > 1 {
        return Err(format!("ambiguous scalar config value [{section}] {key}"));
    }
    let replacement = format!("{key}={value}");
    if let Some(&first) = hits.first() {
        lines[first] = replacement;
    } else {
        let mut insert = end;
        while insert > start + 1 && trim(&lines[insert - 1]).is_empty() {
            insert -= 1;
        }
        lines.insert(insert, replacement);
    }
    Ok(())
}

fn remove(lines: &mut Lines, section: &str, key: &str, all: bool) -> Result<(), String> {
    let (start, end) = span(lines, section);
    if start == lines.len() {
        return Ok(());
    }
    let hits = value_hits(lines, start, end, key);
    if hits.len() > 1 && !all {
        return Err(format!("ambiguous scalar config value [{section}] {key}"));
    }
    for &i in hits.iter().rev() {
        lines.remove(i);
    }
    Ok(())
}

fn retail_value(retail: &Lines, section: &str, key: &str) -> Result<String, String> {
    ini_value(retail, section, key)
}

fn encode(mut lines: Lines) -> String {
    while !lines.is_empty() && trim(lines.last().unwrap()).is_empty() {
        lines.pop();
    }
    lines.join("\r\n")
}

fn validate_default(lines: &Lines, profile: Profile) -> Result<(), String> {
    let retail = profile != Profile::Safe;
    let expected: [(&str, &str, &str); 8] = [
        (
            "Engine.Engine",
            "GameRenderDevice",
            "XOpenGLDrv.XOpenGLRenderDevice",
        ),
        (
            "Engine.Engine",
            "WindowedRenderDevice",
            "XOpenGLDrv.XOpenGLRenderDevice",
        ),
        (
            "Engine.Engine",
            "RenderDevice",
            "XOpenGLDrv.XOpenGLRenderDevice",
        ),
        ("Engine.Engine", "ViewportManager", "SDLDrv.SDLClient"),
        ("Engine.Engine", "AudioDevice", "ALAudio.ALAudioSubsystem"),
        (
            "Engine.Engine",
            "Language",
            if retail { "usa" } else { "int" },
        ),
        ("Engine.GameEngine", "FrameRateLimit", "144.000000"),
        ("SDLDrv.SDLClient", "NativeText", "True"),
    ];
    for (section, key, value) in expected {
        let actual = ini_value(lines, section, key)?;
        if lower(&actual) != lower(value) {
            return Err(format!(
                "non-portable Default.ini [{section}] {key}={actual}"
            ));
        }
    }
    let mut text = String::new();
    for line in lines {
        text.push_str(&lower(line));
        text.push('\n');
    }
    if text.contains("d3d11") {
        return Err("D3D11 leaked into generated Default.ini".to_string());
    }
    const TUPLE_KEYS: [&str; 10] = ["x", "y", "z", "pitch", "yaw", "roll", "r", "g", "b", "a"];
    for raw in lines {
        let stripped = trim(raw);
        if let Some(equal) = stripped.find('=') {
            let key = lower(trim(&stripped[..equal]));
            if TUPLE_KEYS.contains(&key.as_str()) {
                return Err(format!(
                    "malformed retail tuple fragment leaked into Default.ini: {raw}"
                ));
            }
        }
    }
    let (start, end) = span(lines, "Core.System");
    if start == lines.len() {
        return Err("Default.ini template has no [Core.System]".to_string());
    }
    for raw in lines.iter().take(end).skip(start + 1) {
        let stripped = lower(trim(raw));
        if (stripped.starts_with("paths=")
            || stripped.starts_with("savepath=")
            || stripped.starts_with("cachepath="))
            && stripped.contains('\\')
        {
            return Err(format!(
                "Windows path separator in generated Default.ini: {raw}"
            ));
        }
    }
    let (game_start, game_end) = span(lines, "Engine.GameEngine");
    if game_start != lines.len() {
        for raw in lines.iter().take(game_end).skip(game_start + 1) {
            if let Some(first) = raw.find(|c: char| c != ' ' && c != '\t') {
                let bytes = raw.as_bytes();
                let cut = (first + 13).min(bytes.len());
                if ascii_lower_eq(&bytes[first..cut], b"serveractors=") {
                    return Err(
                        "unavailable ServerActors leaked into generated Default.ini".to_string()
                    );
                }
            }
        }
    }
    Ok(())
}

fn validate_defuser(lines: &Lines, retail: &Lines) -> Result<(), String> {
    const INPUT_KEYS: [&str; 11] = [
        "Aliases[8]",
        "RightMouse",
        "LeftMouse",
        "Tab",
        "Enter",
        "Ctrl",
        "Alt",
        "Space",
        "Z",
        "Slash",
        "A",
    ];
    for key in INPUT_KEYS {
        if ini_value(lines, "Engine.Input", key)? != ini_value(retail, "Engine.Input", key)? {
            return Err(format!(
                "retail final binding was not merged: [Engine.Input] {key}"
            ));
        }
    }
    if ini_value(lines, "Engine.PlayerPawn", "bModernThirdPersonControls")? != "False" {
        return Err("Modern controls must default to False".to_string());
    }
    if ini_value(lines, "Engine.PlayerPawn", "ObjectDetail")? != "ObjectDetailVeryHigh" {
        return Err("max-quality ObjectDetailVeryHigh default was not applied".to_string());
    }
    for (key, value) in [
        ("fDamageMultiplier_Easy", "1.2"),
        ("fDamageMultiplier_Medium", "2.0"),
        ("fDamageMultiplier_Hard", "3.0"),
    ] {
        if ini_value(lines, "HGame.Harry", key)? != value {
            return Err(format!("retail difficulty value was not merged: {key}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    fn write_temp(name: &str, contents: &[u8]) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("hp2_ini_test_{}_{name}", std::process::id()));
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(contents).unwrap();
        path
    }

    #[test]
    fn decode_splits_crlf_lf_and_lone_cr() {
        let path = write_temp("mixed", b"a=1\r\nb=2\nc=3\r[S]");
        let lines = decode_ini(path.to_str().unwrap()).unwrap();
        assert_eq!(lines, vec!["a=1", "b=2", "c=3", "[S]"]);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn decode_rejects_utf16_bom() {
        let path = write_temp("utf16", &[0xff, 0xfe, b'a', 0]);
        let err = decode_ini(path.to_str().unwrap()).unwrap_err();
        assert!(
            err.starts_with("UTF-16 config is unsupported for deterministic merge:"),
            "{err}"
        );
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn decode_strips_utf8_bom() {
        let path = write_temp("bom", b"\xef\xbb\xbf[S]\nK=1");
        let lines = decode_ini(path.to_str().unwrap()).unwrap();
        assert_eq!(lines, vec!["[S]", "K=1"]);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn validate_utf8_reports_python_style_message() {
        let err = validate_utf8(&[0x61, 0xff], "/tmp/x.ini").unwrap_err();
        assert_eq!(
            err,
            "config is not UTF-8/ASCII /tmp/x.ini: 'utf-8' codec can't decode byte 0xff in position 1: invalid start byte"
        );
    }

    #[test]
    fn read_missing_file_reports_errno_format() {
        let err = read("/nonexistent/hp2-ini-test-path.ini").unwrap_err();
        assert!(
            err.starts_with("cannot read config /nonexistent/hp2-ini-test-path.ini: [Errno "),
            "{err}"
        );
        assert!(
            err.ends_with(": '/nonexistent/hp2-ini-test-path.ini'"),
            "{err}"
        );
    }

    #[test]
    fn set_matches_key_case_insensitively_but_writes_the_callers_casing() {
        // C++ `Set` builds the replacement line as `key + "=" + value` using
        // the caller's key argument, not the original line's casing.
        let mut lines: Lines = vec![
            "[Engine.Engine]".to_string(),
            "GameRenderDevice=Old".to_string(),
        ];
        set(&mut lines, "engine.engine", "gamerenderdevice", "New").unwrap();
        assert_eq!(lines, vec!["[Engine.Engine]", "gamerenderdevice=New"]);
    }

    #[test]
    fn set_creates_missing_section_with_blank_separator() {
        let mut lines: Lines = vec!["[A]".to_string(), "K=1".to_string()];
        set(&mut lines, "B", "K", "2").unwrap();
        assert_eq!(lines, vec!["[A]", "K=1", "", "[B]", "K=2"]);
    }

    #[test]
    fn ini_value_ambiguous_fails() {
        let lines: Lines = vec!["[S]".to_string(), "K=1".to_string(), "K=2".to_string()];
        let err = ini_value(&lines, "S", "K").unwrap_err();
        assert_eq!(err, "expected one [S] K, found 2");
    }

    #[test]
    fn remove_all_strips_every_hit_without_ambiguity_error() {
        let mut lines: Lines = vec![
            "[Engine.GameEngine]".to_string(),
            "ServerActors=A".to_string(),
            "ServerActors=B".to_string(),
        ];
        remove(&mut lines, "Engine.GameEngine", "ServerActors", true).unwrap();
        assert_eq!(lines, vec!["[Engine.GameEngine]"]);
    }

    #[test]
    fn encode_joins_with_crlf_and_drops_trailing_blank_lines() {
        let lines: Lines = vec![
            "[S]".to_string(),
            "K=1".to_string(),
            "".to_string(),
            "".to_string(),
        ];
        assert_eq!(encode(lines), "[S]\r\nK=1");
    }
}
