//! Harness-compatible command-line parsing for the app shell.
//!
//! Contracts ported from the engine's `appConsumeCommandLineLoadSlot`
//! (`HarryPotter2/Unreal/Engine/Src/UnGame.cpp`) as pinned by
//! `Tests/AbiTests.cpp TestCommandLineLoad`:
//! - `-LOAD=<n>` / `-SAVESLOT=<n>` token scanning over a command line
//!   (whitespace/comma separators, quoted tokens, case-insensitive prefix,
//!   digits-only values, `INT` overflow guard),
//! - at-most-once consumption per process,
//! - malformed or absent arguments are ignored without touching state.
//!
//! Harness compatibility: `-xopengl` and `-vulkan` are accepted and ignored
//! as aliases (the rewrite ships a single wgpu backend); this is documented
//! in [`help_text`] so automation keeps passing the flags.

use crate::policy::{data_directory_value, is_data_directory_argument};

/// Parsed harness-relevant options. Everything unrecognized stays an
/// engine-forwarded option and is simply not recorded here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cli {
    pub no_frontend: bool,
    pub windowed: bool,
    pub nosound: bool,
    pub log: bool,
    /// `-testticks=N`, digits-only; malformed values are ignored.
    pub test_ticks: Option<i32>,
    /// `-INI=<file>` value, verbatim.
    pub ini_path: Option<String>,
    /// First valid `-SAVESLOT=n`.
    pub save_slot: Option<i32>,
    /// First valid `-LOAD=<n>`; only available after [`Cli::consume_load_slot`].
    pub load_slot: Option<i32>,
    /// Value of the first recognized single-dash `-datadir=` override.
    pub data_dir: Option<String>,
}

impl Cli {
    /// Parse argv-style arguments (without the program name).
    pub fn parse(arguments: &[String]) -> Self {
        let mut cli = Self::default();
        for argument in arguments {
            if is_data_directory_argument(argument) && cli.data_dir.is_none() {
                cli.data_dir = data_directory_value(argument).map(str::to_string);
                continue;
            }
            let Some((name, value)) = split_option(argument) else {
                continue;
            };
            match name.to_ascii_lowercase().as_str() {
                "xopengl" | "vulkan" => {
                    // Accepted-and-ignored backend alias: one wgpu backend.
                }
                "nofrontend" => cli.no_frontend = true,
                "window" => cli.windowed = true,
                "nosound" => cli.nosound = true,
                "log" => cli.log = true,
                "ini" => {
                    if cli.ini_path.is_none() {
                        cli.ini_path = Some(value.unwrap_or_default().to_string());
                    }
                }
                "testticks" => {
                    if cli.test_ticks.is_none() {
                        cli.test_ticks = parse_slot_value(value);
                    }
                }
                "saveslot" if cli.save_slot.is_none() => {
                    cli.save_slot = parse_slot_value(value);
                }
                _ => {}
            }
        }
        cli
    }

    /// Consume the startup `-LOAD=` slot at most once, mirroring
    /// `appConsumeCommandLineLoadSlot`. Returns `None` when already consumed
    /// or when no well-formed load argument exists.
    pub fn consume_load_slot(&mut self, command_line: &str) -> Option<i32> {
        self.load_slot = parse_save_slot_token(command_line, "-LOAD=");
        self.load_slot
    }
}

fn split_option(argument: &str) -> Option<(&str, Option<&str>)> {
    let rest = argument.strip_prefix('-')?;
    let rest = rest.trim_start_matches('-');
    if rest.is_empty() {
        return None;
    }
    match rest.find('=') {
        Some(at) => Some((&rest[..at], Some(&rest[at + 1..]))),
        None => Some((rest, None)),
    }
}

/// Digits-only slot value with the C++ `INT` overflow guard. `None` for
/// absent or malformed values (`-LOAD=+1`, `-12x`, overflow, empty).
fn parse_slot_value(value: Option<&str>) -> Option<i32> {
    parse_slot_digits(value?)
}

fn parse_slot_digits(text: &str) -> Option<i32> {
    if text.is_empty() {
        return None;
    }
    let mut parsed: i64 = 0;
    for byte in text.bytes() {
        if !byte.is_ascii_digit() {
            return None;
        }
        let digit = i64::from(byte - b'0');
        if parsed > (i32::MAX as i64 - digit) / 10 {
            return None;
        }
        parsed = parsed * 10 + digit;
    }
    Some(parsed as i32)
}

/// Port of `ParseCommandLineSaveSlot`: scan a command-line string for a
/// case-insensitive `<prefix><digits>` token, honoring quote-wrapped tokens
/// and optional quotes around just the value. Returns `None` on absence,
/// malformed values, unterminated quotes, or overflow.
pub fn parse_save_slot_token(command_line: &str, prefix: &str) -> Option<i32> {
    const SEPARATORS: [char; 5] = [' ', '\t', '\r', '\n', ','];
    let mut rest = command_line;
    while !rest.is_empty() {
        rest = rest.trim_start_matches(SEPARATORS);
        if rest.is_empty() {
            break;
        }
        let quoted_token = rest.starts_with('"');
        let (token, next) = if quoted_token {
            let body = &rest[1..];
            let end = body.find('"')?;
            (&body[..end], &body[end + 1..])
        } else {
            let end = rest.find(SEPARATORS).unwrap_or(rest.len());
            (&rest[..end], &rest[end..])
        };

        if token.len() >= prefix.len() && token[..prefix.len()].eq_ignore_ascii_case(prefix) {
            let mut value = &token[prefix.len()..];
            if let Some(stripped) = value.strip_prefix('"') {
                if !token.ends_with('"') {
                    return None;
                }
                value = stripped;
            }
            return parse_slot_digits(value);
        }
        rest = next;
    }
    None
}

/// Help text documenting every option the shell understands, including the
/// accept-and-ignore backend aliases.
pub fn help_text() -> &'static str {
    r#"Harry Potter 2 (hp2rs) launcher

Usage: HarryPotter2 [options] [map | unreal://url]

Launcher selection:
  (no map/URL options)     open the interactive launcher

Engine options understood by the shell:
  -LOAD=<n>                start by loading save slot n
  -SAVESLOT=<n>            write saves to slot directory n
  -testticks=N             run N ticks under test automation
  -NOFRONTEND              run without the frontend UI
  -window                  run windowed
  -nosound                 disable audio output
  -log                     enable the log file
  -INI=<file>              use an alternate INI file base name
  -datadir=<path>          transient data-root override (isolated profile)

Compatibility aliases (accepted and ignored):
  -xopengl, -vulkan        the rewrite ships a single wgpu backend
"#
}
