//! Pure command-line launcher policy, ported from
//! `HarryPotter2/Unreal/SDLLaunch/Src/HP2LaunchPolicy.cpp`.
//!
//! The port freezes the oracle-tested contracts:
//! - `ShouldRunNativeLauncher` bypass classification (LauncherTests
//!   `TestPolicyClassification`),
//! - `-datadir=` single-dash recognition (`TestDataDirectoryArgumentContract`),
//! - `BuildSelectedCommand` exact prefixes,
//! - the launch-selection serialization contract with whole-store Quit
//!   fallbacks (`TestLaunchSelectionSerializationContract`).

use std::fmt;

// ------------------------------------------------------------------ model

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchAction {
    Continue,
    NewGame,
    Quit,
    Error,
}

impl LaunchAction {
    /// Canonical persisted spelling; `Error` has none.
    pub fn as_name(self) -> Option<&'static str> {
        match self {
            Self::Continue => Some("Continue"),
            Self::NewGame => Some("NewGame"),
            Self::Quit => Some("Quit"),
            Self::Error => None,
        }
    }
}

/// Minimal save coordinate carried by a launch selection. Runtime-derived
/// metadata (paths, labels, timestamps) is deliberately not part of the
/// persistence contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveCoordinate {
    pub save_index: i32,
    pub uses_slot_directory: bool,
    /// `-1` when the save does not live in a slot directory
    /// (`HP2LauncherModel.h`).
    pub slot: i32,
}

impl Default for SaveCoordinate {
    fn default() -> Self {
        Self {
            save_index: 0,
            uses_slot_directory: false,
            slot: -1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchSelection {
    pub action: LaunchAction,
    pub has_save: bool,
    pub save: SaveCoordinate,
}

impl Default for LaunchSelection {
    fn default() -> Self {
        Self {
            action: LaunchAction::Quit,
            has_save: false,
            save: SaveCoordinate::default(),
        }
    }
}

impl fmt::Display for LaunchAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

fn eq_ignore_case(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

/// Builds the engine command prefix for a selection. Ported verbatim from
/// `BuildSelectedCommand`.
pub fn build_selected_command(selection: &LaunchSelection) -> Result<String, String> {
    match selection.action {
        LaunchAction::Quit => Ok(String::new()),
        LaunchAction::NewGame => Ok("PrivetDr.unr".to_string()),
        LaunchAction::Continue => {
            if !selection.has_save {
                return Err("Continue requires a selected save.".to_string());
            }
            if selection.save.save_index < 0 {
                return Err("The selected save index is invalid.".to_string());
            }
            if selection.save.uses_slot_directory && selection.save.slot < 0 {
                return Err("The selected save slot is invalid.".to_string());
            }
            let mut command = format!("Startup.unr -LOAD={}", selection.save.save_index);
            if selection.save.uses_slot_directory {
                command.push_str(&format!(" -SAVESLOT={}", selection.save.slot));
            }
            Ok(command)
        }
        LaunchAction::Error => Err("The launcher did not return a valid action.".to_string()),
    }
}

// ------------------------------------------------------- serialization

/// One canonical `[LastLaunch]` field row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionField {
    pub key: &'static str,
    pub value: String,
}

/// Serializes a selection into canonical field rows. Validation happens
/// before any row is emitted so a rejected selection never leaves a partial
/// field list behind.
pub fn serialize_launch_selection(
    selection: &LaunchSelection,
) -> Result<Vec<SelectionField>, String> {
    let Some(name) = selection.action.as_name() else {
        return Err("The launch selection contains an invalid action.".to_string());
    };
    if selection.action == LaunchAction::Continue {
        if !selection.has_save {
            return Err("Continue requires a selected save.".to_string());
        }
        if selection.save.save_index < 0 {
            return Err("The selected save index is invalid.".to_string());
        }
        if selection.save.uses_slot_directory && selection.save.slot < 0 {
            return Err("The selected save slot is invalid.".to_string());
        }
    }
    let mut fields = vec![SelectionField {
        key: "Action",
        value: name.to_string(),
    }];
    if selection.action == LaunchAction::Continue {
        fields.push(SelectionField {
            key: "HasSave",
            value: "True".to_string(),
        });
        fields.push(SelectionField {
            key: "SaveIndex",
            value: selection.save.save_index.to_string(),
        });
        if selection.save.uses_slot_directory {
            fields.push(SelectionField {
                key: "SaveSlot",
                value: selection.save.slot.to_string(),
            });
        }
    }
    Ok(fields)
}

/// Strict digits-only index parse with an `i32` overflow guard (ported from
/// `ParseSelectionIndex`).
fn parse_selection_index(text: &str) -> Option<i32> {
    if text.is_empty() {
        return None;
    }
    let mut result: i64 = 0;
    for byte in text.bytes() {
        if !byte.is_ascii_digit() {
            return None;
        }
        result = result * 10 + i64::from(byte - b'0');
        if result > i32::MAX as i64 {
            return None;
        }
    }
    Some(result as i32)
}

/// Outcome of parsing persisted field rows: on malformed stores the
/// selection is still populated with the safe Quit fallback and the error
/// explains why.
#[derive(Debug, Clone)]
pub struct DeserializedSelection {
    pub selection: LaunchSelection,
    pub error: Option<String>,
}

/// Parses persisted field rows. Row order is free, the final duplicated row
/// wins (matching INI lookup semantics), unrelated rows are ignored, and
/// malformed stores fall back as a whole to Quit.
pub fn deserialize_launch_selection(fields: &[SelectionField]) -> DeserializedSelection {
    let fallback = |error: String| DeserializedSelection {
        selection: LaunchSelection::default(),
        error: Some(error),
    };

    let mut action = None;
    let mut has_save = None;
    let mut save_index = None;
    let mut save_slot = None;
    for field in fields {
        if eq_ignore_case(field.key, "Action") {
            action = Some(field.value.clone());
        } else if eq_ignore_case(field.key, "HasSave") {
            has_save = Some(field.value.clone());
        } else if eq_ignore_case(field.key, "SaveIndex") {
            save_index = Some(field.value.clone());
        } else if eq_ignore_case(field.key, "SaveSlot") {
            save_slot = Some(field.value.clone());
        }
    }
    let Some(action) = action else {
        return fallback("No persisted launch action was found.".to_string());
    };
    let parsed_action = if eq_ignore_case(&action, "Continue") {
        LaunchAction::Continue
    } else if eq_ignore_case(&action, "NewGame") {
        LaunchAction::NewGame
    } else if eq_ignore_case(&action, "Quit") {
        LaunchAction::Quit
    } else {
        return fallback(format!(
            "The persisted launch action is unknown: '{action}'."
        ));
    };
    let mut parsed = LaunchSelection {
        action: parsed_action,
        ..LaunchSelection::default()
    };
    if parsed_action != LaunchAction::Continue {
        return DeserializedSelection {
            selection: parsed,
            error: None,
        };
    }

    match has_save.as_deref() {
        Some(value) if eq_ignore_case(value, "True") => {}
        _ => {
            return fallback(
                "A persisted Continue selection is missing its save marker.".to_string(),
            );
        }
    }
    parsed.has_save = true;
    let Some(index_text) = save_index else {
        return fallback("A persisted Continue selection has an invalid save index.".to_string());
    };
    let Some(index) = parse_selection_index(&index_text) else {
        return fallback("A persisted Continue selection has an invalid save index.".to_string());
    };
    parsed.save.save_index = index;
    if let Some(slot_text) = save_slot {
        parsed.save.uses_slot_directory = true;
        let Some(slot) = parse_selection_index(&slot_text) else {
            return fallback(
                "A persisted Continue selection has an invalid save slot.".to_string(),
            );
        };
        parsed.save.slot = slot;
    }
    DeserializedSelection {
        selection: parsed,
        error: None,
    }
}

// ------------------------------------------------- launcher-bypass policy

/// Option-name extraction: strips one-or-more leading dashes up to `=`
/// (ported from `OptionName`). Empty for non-options.
pub fn option_name(argument: &str) -> &str {
    let Some(rest) = argument.strip_prefix('-') else {
        return "";
    };
    let rest = rest.trim_start_matches('-');
    if rest.is_empty() {
        return "";
    }
    match rest.find('=') {
        Some(at) => &rest[..at],
        None => rest,
    }
}

const BYPASS_OPTIONS: [&str; 12] = [
    "LOAD",
    "NOFRONTEND",
    "TESTTICKS",
    "TESTRENDEV",
    "TESTNATIVETEXT",
    "SERVER",
    "REPLAY",
    "RECORD",
    "BENCHMARK",
    "COMMANDLET",
    "MAKE",
    "EXEC",
];

/// True when the argument is a recognized explicit data-root override:
/// single-dash `-datadir=` (case-insensitive prefix). Double-dash spellings
/// stay unrecognized interactive options.
pub fn is_data_directory_argument(argument: &str) -> bool {
    const PREFIX: &[u8] = b"datadir=";
    let bytes = argument.as_bytes();
    if bytes.first() != Some(&b'-') || bytes.get(1) == Some(&b'-') {
        return false;
    }
    PREFIX.iter().enumerate().all(|(index, expected)| {
        bytes
            .get(index + 1)
            .is_some_and(|actual| actual.to_ascii_lowercase() == *expected)
    })
}

/// Value of a recognized `-datadir=` argument, verbatim.
pub fn data_directory_value(argument: &str) -> Option<&str> {
    if !is_data_directory_argument(argument) {
        return None;
    }
    argument.split_once('=').map(|(_, value)| value)
}

/// Ported from `ShouldRunNativeLauncher`: any non-option argument or
/// launcher-bypass option means the command line targets the engine
/// directly; otherwise the interactive launcher runs.
pub fn should_run_launcher(arguments: &[String]) -> bool {
    for argument in arguments {
        if argument.is_empty() {
            continue;
        }
        if !argument.starts_with('-') {
            return false;
        }
        if is_data_directory_argument(argument) {
            continue;
        }
        let name = option_name(argument);
        for bypass in BYPASS_OPTIONS {
            if eq_ignore_case(name, bypass) {
                return false;
            }
        }
    }
    true
}
