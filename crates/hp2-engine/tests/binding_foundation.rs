use hp2_engine::binding_foundation::{self as b, Action, BindingError, Capture, CaptureResult::*};
use std::collections::BTreeMap;

#[test]
fn complete_commands_and_quoted_pipes_are_preserved() {
    let source = " button Jump |Say \"A|B\"|Button JumpHigher|OnRelease Button Jump";
    assert!(b::contains(source, "BUTTON jump").unwrap());
    assert_eq!(b::remove(source, "button jump").unwrap(), "Say \"A|B\"|Button JumpHigher|OnRelease Button Jump");
    assert_eq!(b::remove(source, "jump").unwrap(), source);
    assert_eq!(b::add(source, "BUTTON JUMP").unwrap(), source);
    assert_eq!(b::add(source, "Axis aForward Speed=2").unwrap(), format!("{source}|Axis aForward Speed=2"));
}

#[test]
fn exact_edits_preserve_case_sensitive_arguments_and_reject_duplicates() {
    let source = "Say \"hello\"|Say \"HELLO\"|Button Jump";
    assert_eq!(b::remove_exact(source, "Say \"hello\"").unwrap(), "Say \"HELLO\"|Button Jump");
    assert!(b::remove_exact("Button X|Button X", "Button X").is_err());
    assert!(b::remove_exact(source, "missing").is_err());
    assert!(b::add_exact("Say \"hello\"", "Say \"HELLO\"").is_err());
}

#[test]
fn unsafe_or_ambiguous_syntax_fails_without_mutation() {
    let mut bindings = BTreeMap::from([("A".into(), "Button Jump|Say \"unterminated".into())]);
    let before = bindings.clone();
    assert_eq!(b::edit_key(&mut bindings, "A", &["Button Jump"], &["New"]), Err(BindingError::UnsupportedSyntax));
    assert_eq!(bindings, before);
    for text in ["A//comment|B", "A\nB", "A\0B"] { assert!(b::remove(text, "A").is_err()); }
    assert!(b::add("A", "B|C").is_err());
    // Failure in a later operation must also leave the original map intact.
    let mut map = BTreeMap::from([("A".into(), "Jump|Other".into())]);
    let before = map.clone();
    assert!(b::edit_key(&mut map, "A", &["Jump"], &["x|y"]).is_err());
    assert_eq!(map, before);
}

#[test]
fn edits_leave_hidden_keys_and_other_commands_alone() {
    let mut map = BTreeMap::from([("A".into(), "Jump|custom arg|JumpHigher".into()), ("Hidden".into(), "Jump|Secret".into())]);
    b::edit_key(&mut map, "A", &["Jump"], &["Duck"]).unwrap();
    assert_eq!(map["A"], "custom arg|JumpHigher|Duck");
    assert_eq!(map["Hidden"], "Jump|Secret");
}

#[test]
fn conflict_requires_shared_context_and_exact_command() {
    let selected = Action { id: "jump", commands: &["Jump"], contexts: &["game"] };
    let actions = [
        Action { id: "jump", commands: &["Jump"], contexts: &["game"] },
        Action { id: "duck", commands: &["Duck", "AltDuck"], contexts: &["game"] },
        Action { id: "broom", commands: &["Fly"], contexts: &["broom"] },
        Action { id: "other", commands: &["DuckExtra"], contexts: &["game"] },
    ];
    assert_eq!(b::conflicts("Duck|Fly|custom", &selected, &actions).unwrap(), vec!["duck"]);
    assert!(b::conflicts("DuckExtra", &selected, &actions).unwrap().contains(&"other"));
    // Aliases and OnRelease are not expanded or unwrapped by this API.
    assert!(b::conflicts("OnRelease Duck|AliasForDuck", &selected, &actions).unwrap().is_empty());
}

#[test]
fn capture_swallows_activation_repeats_and_matching_releases() {
    let mut c = Capture::default();
    c.begin([], [1]);
    assert_eq!(c.event(1, true, false), Consume);
    assert_eq!(c.event(42, true, false), Captured(42));
    assert_eq!(c.event(42, true, false), Consume);
    assert_eq!(c.event(1, false, false), Consume);
    assert_eq!(c.event(42, false, false), Consume);
    assert_eq!(c.event(42, true, false), Pass);
}

#[test]
fn cancellation_drains_held_events_and_reopening_keeps_drain() {
    let mut c = Capture::default();
    c.begin([], [1]);
    assert_eq!(c.event(27, true, true), Cancelled);
    c.begin([], []);
    assert_eq!(c.event(27, true, true), Consume);
    c.cancel();
    assert_eq!(c.event(27, false, true), Consume);
    assert_eq!(c.event(1, false, false), Consume);
    assert_eq!(c.event(9, true, false), Pass);
}
