//! Player-facing accessibility options: subtitle size, always-on subtitles and a colour-blind
//! HUD palette. Each option has a cached getter and a setter that persists to the user ini
//! (`[HP2.Accessibility]`) through the ordinary config APIs and applies live.
//!
//! All defaults reproduce the original behaviour: 100% subtitles, subtitles only where the
//! scripts request them, retail red/green HUD feedback.
use std::{
    ffi::c_void,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};

use hp2_abi::engine::generated::{TArrayRaw, UObject};
use hp2_core::core_access::{CoreAccess, RuntimeResult};

use crate::{
    stock_graphics_menu::object,
    ui_vm::{self, V},
};

pub(crate) const SECTION: &str = "HP2.Accessibility";
pub(crate) const KEY_SUBTITLE_SCALE: &str = "SubtitleScale";
pub(crate) const KEY_ALWAYS_SUBTITLES: &str = "AlwaysSubtitles";
pub(crate) const KEY_COLOR_BLIND_HUD: &str = "ColorBlindHud";

/// Subtitle size range in percent of the original size.
pub(crate) const SUBTITLE_SCALE_MIN: f32 = 100.0;
pub(crate) const SUBTITLE_SCALE_MAX: f32 = 200.0;

static LOADED: AtomicBool = AtomicBool::new(false);
static SUBTITLE_PERCENT: AtomicU32 = AtomicU32::new(100);
static ALWAYS_SUBTITLES: AtomicBool = AtomicBool::new(false);
static COLOR_BLIND: AtomicBool = AtomicBool::new(false);

fn units(text: &str) -> Vec<u32> {
    text.chars().map(u32::from).collect()
}

fn read_text(access: CoreAccess<'_>, key: &str) -> Option<String> {
    access
        .config_get_units(&units(SECTION), &units(key))
        .ok()
        .flatten()
        .map(|v| v.iter().take_while(|&&c| c != 0).filter_map(|&c| char::from_u32(c)).collect())
}

fn parse_bool(text: &str, default: bool) -> bool {
    match text.trim().to_ascii_lowercase().as_str() {
        "false" | "off" | "no" | "0" => false,
        "true" | "on" | "yes" | "1" => true,
        _ => default,
    }
}

/// Parses a percent, tolerating a trailing `%` or a decimal part; anything unusable is 100.
fn parse_percent(text: &str) -> u32 {
    text.trim().trim_end_matches('%').trim().parse::<f32>().map_or(100, clamp_percent)
}

fn clamp_percent(value: f32) -> u32 {
    if value.is_finite() { value.round().clamp(SUBTITLE_SCALE_MIN, SUBTITLE_SCALE_MAX) as u32 } else { 100 }
}

/// Loads every option once from the user ini; later reads are atomic loads (these are read per frame).
fn ensure(access: CoreAccess<'_>) {
    if LOADED.load(Ordering::Acquire) {
        return;
    }
    SUBTITLE_PERCENT.store(read_text(access, KEY_SUBTITLE_SCALE).map_or(100, |t| parse_percent(&t)), Ordering::Relaxed);
    ALWAYS_SUBTITLES.store(read_text(access, KEY_ALWAYS_SUBTITLES).is_some_and(|t| parse_bool(&t, false)), Ordering::Relaxed);
    COLOR_BLIND.store(read_text(access, KEY_COLOR_BLIND_HUD).is_some_and(|t| parse_bool(&t, false)), Ordering::Relaxed);
    LOADED.store(true, Ordering::Release);
}

fn write(access: CoreAccess<'_>, key: &str, value: &str) -> RuntimeResult<()> {
    access.config_update_system_staged(&[(units(SECTION), units(key), units(value))])
}

/// Subtitle size in percent: 100 (original) to 200.
pub(crate) fn subtitle_scale(access: CoreAccess<'_>) -> f32 {
    ensure(access);
    SUBTITLE_PERCENT.load(Ordering::Relaxed) as f32
}

/// Persists the subtitle size (clamped to 100..=200 percent) and applies it to the next frame.
pub(crate) fn set_subtitle_scale(access: CoreAccess<'_>, percent: f32) -> RuntimeResult<()> {
    ensure(access);
    let percent = clamp_percent(percent);
    write(access, KEY_SUBTITLE_SCALE, &percent.to_string())?;
    SUBTITLE_PERCENT.store(percent, Ordering::Relaxed);
    Ok(())
}

/// The subtitle size as a multiplier of the original (1.0 to 2.0).
pub(crate) fn subtitle_factor(access: CoreAccess<'_>) -> f32 {
    subtitle_scale(access) / 100.0
}

/// Whether every spoken line shows a subtitle.
pub(crate) fn always_subtitles(access: CoreAccess<'_>) -> bool {
    ensure(access);
    ALWAYS_SUBTITLES.load(Ordering::Relaxed)
}

pub(crate) fn set_always_subtitles(access: CoreAccess<'_>, on: bool) -> RuntimeResult<()> {
    ensure(access);
    write(access, KEY_ALWAYS_SUBTITLES, if on { "True" } else { "False" })?;
    ALWAYS_SUBTITLES.store(on, Ordering::Relaxed);
    Ok(())
}

/// Whether the Rust-owned HUD indicators use the blue/orange palette.
pub(crate) fn color_blind_hud(access: CoreAccess<'_>) -> bool {
    ensure(access);
    COLOR_BLIND.load(Ordering::Relaxed)
}

pub(crate) fn set_color_blind_hud(access: CoreAccess<'_>, on: bool) -> RuntimeResult<()> {
    ensure(access);
    write(access, KEY_COLOR_BLIND_HUD, if on { "True" } else { "False" })?;
    COLOR_BLIND.store(on, Ordering::Relaxed);
    Ok(())
}

/// Added-colour passes over a health bolt (translucent drawing multiplies texel by colour).
pub(crate) const HEALTH_OVERLAY_PASSES: usize = 3;

/// How the health fill is drawn for one script-supplied change colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HealthPalette {
    /// Modulation for the gold fill (white keeps the original gold).
    pub fill: [u8; 4],
    /// Colour added (translucently) over the fill with the bolt-shaped HUD icon; `None` adds nothing.
    pub overlay: Option<[u8; 4]>,
}

/// Maps the retail change colour (red loss, green gain, fading back to white) onto blue/orange.
/// The fill texture is gold, which a tint can only darken, so the hue change comes from an added
/// overlay: loss blanks the bolt and adds blue, gain keeps the gold and adds orange brightness.
/// Both fade back to the neutral retail look with the script's own fade.
pub(crate) fn health_palette(tint: [u8; 4]) -> HealthPalette {
    let base = tint[0].min(tint[1]).min(tint[2]);
    let red = tint[0] - base;
    let green = tint[1] - base;
    let alpha = tint[3];
    if red == 0 && green == 0 {
        return HealthPalette { fill: tint, overlay: None };
    }
    let scale = |v: u8, s: u8| (u16::from(v) * u16::from(s) / 255) as u8;
    if green > red {
        // strength 0 (white) .. 255 (full retail green)
        let s = green - red;
        return HealthPalette { fill: [255, 255, 255, alpha], overlay: Some([scale(255, s), scale(140, s), 0, alpha]) };
    }
    let s = red - green;
    let dim = 255 - scale(255, s);
    HealthPalette { fill: [dim, dim, dim, alpha], overlay: Some([0, scale(90, s), scale(255, s), alpha]) }
}

/// The dialogue id of a sound in the `AllDialog` package, from its full name
/// (`Sound AllDialog.Harry_01`), or `None` for any other sound.
fn dialog_id(full_name: &str) -> Option<&str> {
    let path = full_name.split_once(' ').map_or(full_name, |(_, path)| path);
    let mut parts = path.split('.');
    let mut previous = parts.next()?;
    let mut last = previous;
    let mut in_dialog = previous.eq_ignore_ascii_case("AllDialog");
    for part in parts {
        previous = last;
        last = part;
        in_dialog |= previous.eq_ignore_ascii_case("AllDialog");
    }
    (in_dialog && last != path && !last.is_empty()).then_some(last)
}

/// Removes the facial-expression marker (`[Normal]`, as `Actor.HandleFacialExpression` does)
/// and any `<tag>` markers that prefix a localized line.
fn strip_markers(text: &str) -> &str {
    let mut rest = text.trim_start();
    loop {
        let (open, close) = match rest.chars().next() { Some('[') => ('[', ']'), Some('<') => ('<', '>'), _ => break };
        match rest[open.len_utf8()..].find(close) {
            Some(end) => rest = rest[open.len_utf8() + end + 1..].trim_start(),
            None => break,
        }
    }
    rest.trim_end()
}

unsafe fn localize(a: CoreAccess<'_>, receiver: *mut UObject, key: &str) -> RuntimeResult<String> {
    // SAFETY: the receiver is the live actor playing the sound; the result string is released below.
    unsafe {
        let mut raw: TArrayRaw = std::mem::zeroed();
        let outcome = ui_vm::invoke_virtual(
            a,
            receiver,
            "Localize",
            &[("SectionName", V::S("all")), ("KeyName", V::S(key)), ("PackageName", V::S("HPdialog"))],
            &[],
            (&raw mut raw).cast::<c_void>(),
        );
        let text = outcome.and_then(|_| crate::ui_console_views::string_value(a, &raw));
        let cleanup = a.destroy_owned_fstring(&mut raw);
        let text = text?;
        cleanup?;
        Ok(text)
    }
}

/// Always-on subtitles: after an actor plays an `AllDialog` sound, show its localized line when
/// the script did not ask for one. The stock script path (`DeliverLocalizedDialog`) sets the same
/// text right after `PlaySound`, which simply replaces ours with identical text.
///
/// # Safety
/// `actor` is the live receiver of the native `PlaySound`/`PlayOwnedSound`; `sound` is its sound
/// argument (null or a live Sound).
pub(crate) unsafe fn on_play_sound(a: CoreAccess<'_>, actor: *mut UObject, sound: *mut c_void) -> RuntimeResult<()> {
    if sound.is_null() || !always_subtitles(a) {
        return Ok(());
    }
    // SAFETY: the sound and actor are live for the call; every reflected read is checked.
    unsafe {
        let full: String = a.object_full_name(sound.cast())?
            .into_iter()
            .filter_map(char::from_u32)
            .collect();
        let Some(id) = dialog_id(&full) else { return Ok(()) };
        let level = object(a, actor, "Level")?;
        if level.is_null() {
            return Ok(());
        }
        let harry = object(a, level, "PlayerHarryActor")?;
        if harry.is_null() {
            return Ok(());
        }
        let hud = object(a, harry, "MyHud")?;
        if hud.is_null() {
            return Ok(());
        }
        let line = localize(a, actor, id)?;
        let text = strip_markers(&line);
        // A missing key localizes to `<?int?Section.Key?>`.
        if text.is_empty() || text.starts_with("<?") {
            return Ok(());
        }
        let duration = crate::sound_owner::get_duration(a, sound.cast())?;
        let duration = if duration > 0.0 { duration } else { text.chars().count() as f32 * 0.01 + 3.0 };
        ui_vm::invoke_virtual(a, hud, "SetSubtitleText", &[("text", V::S(text)), ("duration", V::F(duration))], &[], std::ptr::null_mut())?;
    }
    Ok(())
}

/// Runs the pure option, palette and dialogue-recognition contracts under `ui_hud::tests`.
#[cfg(test)]
pub(crate) fn assert_contracts() {
    fn percent_and_bool_parsing_default_to_original_behaviour() {
        assert_eq!(parse_percent("150"), 150);
        assert_eq!(parse_percent(" 175% "), 175);
        assert_eq!(parse_percent("50"), 100);
        assert_eq!(parse_percent("900"), 200);
        assert_eq!(parse_percent("junk"), 100);
        assert_eq!(parse_percent("nan"), 100);
        assert!(parse_bool("True", false) && parse_bool("on", false));
        assert!(!parse_bool("False", true) && !parse_bool("0", true));
        assert!(!parse_bool("???", false));
    }

    fn health_palette_keeps_retail_neutral_and_separates_loss_from_gain() {
        let white = [255, 255, 255, 255];
        assert_eq!(health_palette(white), HealthPalette { fill: white, overlay: None });
        // Loss: darker bolt plus blue; gain: unchanged gold plus orange. Never red or green.
        let loss = health_palette([255, 0, 0, 255]);
        assert_eq!(loss.fill, [0, 0, 0, 255]);
        assert_eq!(loss.overlay, Some([0, 90, 255, 255]));
        let gain = health_palette([0, 255, 0, 255]);
        assert_eq!(gain.fill, white);
        assert_eq!(gain.overlay, Some([255, 140, 0, 255]));
        for palette in [loss, gain] {
            let [r, g, b, _] = palette.overlay.unwrap();
            assert!(r < 10 || b < 10, "overlay must not be a red-green mix");
            assert!(!(g > r && g > b && r < 100), "overlay must not be green");
        }
        // Fading to white returns smoothly to the neutral retail colours.
        let faded_loss = health_palette([255, 200, 200, 255]);
        assert!(faded_loss.fill[0] >= 200 && faded_loss.overlay.unwrap()[2] < 80);
        let faded_gain = health_palette([200, 255, 200, 128]);
        assert_eq!(faded_gain.fill, [255, 255, 255, 128]);
        assert_eq!(faded_gain.overlay.map(|c| c[3]), Some(128));
        assert!(faded_gain.overlay.unwrap()[0] < 80);
    }

    fn dialogue_sounds_are_recognized_by_package() {
        assert_eq!(dialog_id("Sound AllDialog.Harry_01"), Some("Harry_01"));
        assert_eq!(dialog_id("Sound HPSounds.AllDialog.Ron_04"), Some("Ron_04"));
        assert_eq!(dialog_id("Sound HPSounds.Spells.Lumos"), None);
        assert_eq!(dialog_id("Sound AllDialog"), None);
    }

    fn localized_marker_prefixes_are_removed_from_subtitles() {
        assert_eq!(strip_markers("<smile> Hello there. "), "Hello there.");
        assert_eq!(strip_markers("<a><b>Text"), "Text");
        assert_eq!(strip_markers("[Normal]Haha! Beaten down by your own house!"), "Haha! Beaten down by your own house!");
        assert_eq!(strip_markers("[mad] <x> Text"), "Text");
        assert_eq!(strip_markers("[unterminated"), "[unterminated");
        assert_eq!(strip_markers("Plain"), "Plain");
        assert_eq!(strip_markers("<unterminated"), "<unterminated");
    }
    percent_and_bool_parsing_default_to_original_behaviour();
    health_palette_keeps_retail_neutral_and_separates_loss_from_gain();
    dialogue_sounds_are_recognized_by_package();
    localized_marker_prefixes_are_removed_from_subtitles();
}
