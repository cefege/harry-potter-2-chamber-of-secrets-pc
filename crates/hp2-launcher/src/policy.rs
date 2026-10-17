//! Replaces `HP2LaunchPolicy.cpp`'s real (implemented, called)
//! functions: `ShouldRunNativeLauncher`, `BuildSelectedCommand`,
//! `SerializeLaunchSelection`, `DeserializeLaunchSelection`. See this
//! crate's root doc for the two declared-but-never-implemented
//! functions this module does not port.

use crate::model::{LaunchAction, LaunchSelection};

/// Replaces the anonymous-namespace `EqualAsciiInsensitive`: ASCII-only
/// case-insensitive comparison, byte for byte (not full Unicode
/// casefolding -- matches the C++'s own `std::tolower` per-byte loop
/// exactly, including its behavior on non-ASCII bytes, which
/// `std::tolower` leaves unchanged when not in the "C" locale's ASCII
/// range).
fn equal_ascii_insensitive(left: &str, right: &str) -> bool {
    left.len() == right.len()
        && left
            .bytes()
            .zip(right.bytes())
            .all(|(a, b)| a.eq_ignore_ascii_case(&b))
}

/// Replaces `OptionName`: strips a leading run of `-` characters and
/// everything from `=` onward. Empty for a non-option argument (no
/// leading `-`) or an argument that is only dashes.
fn option_name(argument: &str) -> String {
    if !argument.starts_with('-') {
        return String::new();
    }
    let after_dashes = argument.trim_start_matches('-');
    if after_dashes.is_empty() {
        return String::new();
    }
    match after_dashes.find('=') {
        Some(eq) => after_dashes[..eq].to_string(),
        None => after_dashes.to_string(),
    }
}

/// Replaces `IsDataDirectoryArgument`: a single-dash (not `--`)
/// argument whose body starts with `datadir=`, matched case-
/// insensitively byte by byte like the C++'s own loop.
fn is_data_directory_argument(argument: &str) -> bool {
    let bytes = argument.as_bytes();
    if bytes.first() != Some(&b'-') || bytes.get(1) == Some(&b'-') {
        return false;
    }
    const PREFIX: &[u8] = b"datadir=";
    let body = &bytes[1..];
    body.len() >= PREFIX.len()
        && body[..PREFIX.len()]
            .iter()
            .zip(PREFIX)
            .all(|(a, b)| a.to_ascii_lowercase() == *b)
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

/// Replaces `ShouldRunNativeLauncher`. `args` is the full argument
/// list including `argv[0]`, matching the C++ signature's `argc`/
/// `argv` (index 0 skipped the same way).
pub fn should_run_native_launcher(args: &[String]) -> bool {
    for argument in args.iter().skip(1) {
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
        if BYPASS_OPTIONS
            .iter()
            .any(|bypass| equal_ascii_insensitive(&name, bypass))
        {
            return false;
        }
    }
    true
}

/// Replaces `BuildSelectedCommand`.
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
            let mut prefix = format!("Startup.unr -LOAD={}", selection.save.save_index);
            if selection.save.uses_slot_directory {
                prefix.push_str(&format!(" -SAVESLOT={}", selection.save.slot));
            }
            Ok(prefix)
        }
        LaunchAction::Error => Err("The launcher did not return a valid action.".to_string()),
    }
}

/// A persisted `[LastLaunch]` field row: replaces `LaunchSelectionField`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchSelectionField {
    pub key: String,
    pub value: String,
}

fn selection_action_name(action: LaunchAction) -> Option<&'static str> {
    match action {
        LaunchAction::Continue => Some("Continue"),
        LaunchAction::NewGame => Some("NewGame"),
        LaunchAction::Quit => Some("Quit"),
        LaunchAction::Error => None,
    }
}

/// Replaces `SerializeLaunchSelection`.
pub fn serialize_launch_selection(
    selection: &LaunchSelection,
) -> Result<Vec<LaunchSelectionField>, String> {
    let Some(name) = selection_action_name(selection.action) else {
        return Err("The launch selection contains an invalid action.".to_string());
    };
    // Validate completely before emitting rows so a rejected selection
    // never leaves a partial field list behind.
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
    let mut fields = vec![LaunchSelectionField {
        key: "Action".to_string(),
        value: name.to_string(),
    }];
    if selection.action != LaunchAction::Continue {
        return Ok(fields);
    }
    fields.push(LaunchSelectionField {
        key: "HasSave".to_string(),
        value: "True".to_string(),
    });
    fields.push(LaunchSelectionField {
        key: "SaveIndex".to_string(),
        value: selection.save.save_index.to_string(),
    });
    if selection.save.uses_slot_directory {
        fields.push(LaunchSelectionField {
            key: "SaveSlot".to_string(),
            value: selection.save.slot.to_string(),
        });
    }
    Ok(fields)
}

/// Replaces the anonymous-namespace `ParseSelectionIndex`: a decimal,
/// non-negative integer with no leading sign, matching the C++'s
/// digit-by-digit accumulation exactly (including its overflow guard
/// against `i32::MAX`, checked after each digit the same way the C++
/// checks after each `result = result * 10 + digit`).
fn parse_selection_index(text: &str) -> Option<i32> {
    if text.is_empty() {
        return None;
    }
    let mut result: i64 = 0;
    for c in text.chars() {
        if !c.is_ascii_digit() {
            return None;
        }
        result = result * 10 + i64::from(c as u8 - b'0');
        if result > i64::from(i32::MAX) {
            return None;
        }
    }
    Some(result as i32)
}

/// Replaces `DeserializeLaunchSelection`. The real C++ always populates
/// `selection` (the parsed value on success, the `Quit` fallback on
/// failure) in addition to the bool/error pair; this returns
/// `Ok(selection)` or `Err(message)` instead -- callers that need the
/// C++'s "still get a usable (Quit) selection on failure" behavior use
/// `LaunchSelection::default()` (which is `Quit`, see that type's own
/// doc) on `Err`, the same fallback value the C++ would have left in
/// place.
pub fn deserialize_launch_selection(
    fields: &[LaunchSelectionField],
) -> Result<LaunchSelection, String> {
    let mut action: Option<&str> = None;
    let mut has_save: Option<&str> = None;
    let mut save_index: Option<&str> = None;
    let mut save_slot: Option<&str> = None;
    for field in fields {
        if equal_ascii_insensitive(&field.key, "Action") {
            action = Some(&field.value);
        } else if equal_ascii_insensitive(&field.key, "HasSave") {
            has_save = Some(&field.value);
        } else if equal_ascii_insensitive(&field.key, "SaveIndex") {
            save_index = Some(&field.value);
        } else if equal_ascii_insensitive(&field.key, "SaveSlot") {
            save_slot = Some(&field.value);
        }
    }
    let Some(action) = action else {
        return Err("No persisted launch action was found.".to_string());
    };

    let parsed = if equal_ascii_insensitive(action, "Continue") {
        LaunchAction::Continue
    } else if equal_ascii_insensitive(action, "NewGame") {
        LaunchAction::NewGame
    } else if equal_ascii_insensitive(action, "Quit") {
        LaunchAction::Quit
    } else {
        return Err(format!(
            "The persisted launch action is unknown: '{action}'."
        ));
    };
    if parsed != LaunchAction::Continue {
        return Ok(LaunchSelection {
            action: parsed,
            ..LaunchSelection::default()
        });
    }

    if !has_save.is_some_and(|v| equal_ascii_insensitive(v, "True")) {
        return Err("A persisted Continue selection is missing its save marker.".to_string());
    }
    let mut selection = LaunchSelection {
        action: parsed,
        has_save: true,
        ..LaunchSelection::default()
    };
    match save_index.and_then(parse_selection_index) {
        Some(index) => selection.save.save_index = index,
        None => {
            return Err("A persisted Continue selection has an invalid save index.".to_string());
        }
    }
    if let Some(slot_text) = save_slot {
        selection.save.uses_slot_directory = true;
        match parse_selection_index(slot_text) {
            Some(slot) => selection.save.slot = slot,
            None => {
                return Err("A persisted Continue selection has an invalid save slot.".to_string());
            }
        }
    }
    Ok(selection)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SaveRecord;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    /// Every case here confirmed against the real compiled
    /// `ShouldRunNativeLauncher`/`IsDataDirectoryArgument`/`OptionName`
    /// (external linkage, called directly from a throwaway probe TU).
    #[test]
    fn should_run_native_launcher_matches_cpp_cases() {
        assert!(!should_run_native_launcher(&args(&["p", "-LOAD=1"])));
        assert!(!should_run_native_launcher(&args(&["p", "-load=1"])));
        assert!(!should_run_native_launcher(&args(&["p", "-nofrontend"])));
        assert!(should_run_native_launcher(&args(&["p", "--datadir=foo"])));
        assert!(should_run_native_launcher(&args(&["p", "-DataDir=foo"])));
        assert!(!should_run_native_launcher(&args(&["p", "foo"])));
        assert!(should_run_native_launcher(&args(&["p", "-foo"])));
        assert!(!should_run_native_launcher(&args(&["p", "-exec", "-load"])));
        assert!(should_run_native_launcher(&args(&["p"])));
    }

    fn continue_selection(has_save: bool, save_index: i32, slot: Option<i32>) -> LaunchSelection {
        LaunchSelection {
            action: LaunchAction::Continue,
            has_save,
            save: SaveRecord {
                save_index,
                uses_slot_directory: slot.is_some(),
                slot: slot.unwrap_or(-1),
                ..SaveRecord::default()
            },
        }
    }

    #[test]
    fn build_selected_command_matches_cpp_cases() {
        assert_eq!(
            build_selected_command(&LaunchSelection::default()).unwrap(),
            ""
        );
        assert_eq!(
            build_selected_command(&LaunchSelection {
                action: LaunchAction::NewGame,
                ..Default::default()
            })
            .unwrap(),
            "PrivetDr.unr"
        );
        let err = build_selected_command(&LaunchSelection {
            action: LaunchAction::Error,
            ..Default::default()
        })
        .unwrap_err();
        assert_eq!(err, "The launcher did not return a valid action.");
        let err = build_selected_command(&continue_selection(false, 0, None)).unwrap_err();
        assert_eq!(err, "Continue requires a selected save.");
        let err = build_selected_command(&continue_selection(true, -1, None)).unwrap_err();
        assert_eq!(err, "The selected save index is invalid.");
        let err = build_selected_command(&continue_selection(true, 5, Some(-1))).unwrap_err();
        assert_eq!(err, "The selected save slot is invalid.");
        assert_eq!(
            build_selected_command(&continue_selection(true, 5, None)).unwrap(),
            "Startup.unr -LOAD=5"
        );
        assert_eq!(
            build_selected_command(&continue_selection(true, 5, Some(3))).unwrap(),
            "Startup.unr -LOAD=5 -SAVESLOT=3"
        );
    }

    #[test]
    fn serialize_launch_selection_matches_cpp_cases() {
        let fields = serialize_launch_selection(&LaunchSelection::default()).unwrap();
        assert_eq!(
            fields,
            vec![LaunchSelectionField {
                key: "Action".into(),
                value: "Quit".into()
            }]
        );
        let fields = serialize_launch_selection(&continue_selection(true, 5, Some(3))).unwrap();
        assert_eq!(
            fields,
            vec![
                LaunchSelectionField {
                    key: "Action".into(),
                    value: "Continue".into()
                },
                LaunchSelectionField {
                    key: "HasSave".into(),
                    value: "True".into()
                },
                LaunchSelectionField {
                    key: "SaveIndex".into(),
                    value: "5".into()
                },
                LaunchSelectionField {
                    key: "SaveSlot".into(),
                    value: "3".into()
                },
            ]
        );
        let err = serialize_launch_selection(&LaunchSelection {
            action: LaunchAction::Error,
            ..Default::default()
        })
        .unwrap_err();
        assert_eq!(err, "The launch selection contains an invalid action.");
    }

    fn field(key: &str, value: &str) -> LaunchSelectionField {
        LaunchSelectionField {
            key: key.to_string(),
            value: value.to_string(),
        }
    }

    /// This specific case caught a real bug during this port: an
    /// earlier version compared the `HasSave` row's *value* against the
    /// literal string `"True"` with Rust's `==` (case-sensitive), but
    /// the real C++ uses `EqualAsciiInsensitive(hasSave, "True")` for
    /// that value too, not just for key matching -- confirmed directly
    /// against the real compiled `DeserializeLaunchSelection`, which
    /// accepts `hassave=true` (all-lowercase value) for a Continue
    /// selection. Fixed by routing the value comparison through the
    /// same `equal_ascii_insensitive` helper the key lookups already
    /// used.
    #[test]
    fn deserialize_accepts_a_lowercase_hassave_value() {
        let selection = deserialize_launch_selection(&[
            field("Action", "continue"),
            field("hassave", "true"),
            field("saveindex", "7"),
        ])
        .unwrap();
        assert_eq!(selection.action, LaunchAction::Continue);
        assert!(selection.has_save);
        assert_eq!(selection.save.save_index, 7);
    }

    #[test]
    fn deserialize_launch_selection_matches_cpp_cases() {
        assert_eq!(
            deserialize_launch_selection(&[field("Action", "Quit")])
                .unwrap()
                .action,
            LaunchAction::Quit
        );
        assert_eq!(
            deserialize_launch_selection(&[field("Action", "NewGame")])
                .unwrap()
                .action,
            LaunchAction::NewGame
        );
        let selection = deserialize_launch_selection(&[
            field("Action", "Continue"),
            field("HasSave", "True"),
            field("SaveIndex", "5"),
            field("SaveSlot", "3"),
        ])
        .unwrap();
        assert_eq!(selection.action, LaunchAction::Continue);
        assert!(selection.has_save);
        assert_eq!(selection.save.save_index, 5);
        assert_eq!(selection.save.slot, 3);
        assert!(selection.save.uses_slot_directory);

        // Last duplicated row wins, matching INI lookup semantics.
        let selection =
            deserialize_launch_selection(&[field("Action", "Quit"), field("Action", "NewGame")])
                .unwrap();
        assert_eq!(selection.action, LaunchAction::NewGame);

        let err = deserialize_launch_selection(&[field("Action", "Bogus")]).unwrap_err();
        assert_eq!(err, "The persisted launch action is unknown: 'Bogus'.");
        let err = deserialize_launch_selection(&[]).unwrap_err();
        assert_eq!(err, "No persisted launch action was found.");
        let err = deserialize_launch_selection(&[field("Action", "Continue")]).unwrap_err();
        assert_eq!(
            err,
            "A persisted Continue selection is missing its save marker."
        );
        let err = deserialize_launch_selection(&[
            field("Action", "Continue"),
            field("HasSave", "True"),
            field("SaveIndex", "-5"),
        ])
        .unwrap_err();
        assert_eq!(
            err,
            "A persisted Continue selection has an invalid save index."
        );
    }

    /// `Err` leaves the caller to apply the `Quit` fallback via
    /// `LaunchSelection::default()`, matching the C++'s own
    /// always-populate-something contract (see this function's doc).
    #[test]
    fn deserialize_error_fallback_is_quit() {
        assert_eq!(LaunchSelection::default().action, LaunchAction::Quit);
    }
}
