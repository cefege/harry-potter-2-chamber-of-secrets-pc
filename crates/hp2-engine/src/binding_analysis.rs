//! Conservative, bounded analysis of canonical input command targets.
//!
//! This does not execute commands, inspect actor properties, resolve script or
//! viewport commands, or decide gameplay contexts. It is not a rebinding UI.
use std::collections::BTreeSet;
use hp2_core::{ansi::Tchar, command::parse_command, tokenize::{parse_line, parse_token}};

#[cfg(test)]
mod tests {
    #[test]
    fn engine_guard_is_active_for_nested_alias_dispatch() {
        assert!(!crate::input::in_alias());
        crate::input::with_alias_scope(|| -> Result<(), ()> {
            assert!(crate::input::in_alias());
            Ok(())
        }).unwrap();
        assert!(!crate::input::in_alias());
    }
}

const MAX_SOURCE_UNITS: usize = 65_536;
const MAX_LINES: usize = 1024;
const INPUT_LINE_SIZE: usize = 256;
const MAX_ALIASES: usize = 40;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Phase { Press, Hold, Release, Axis }
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum PropertyKind { Button, Axis }
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Target { pub phase: Phase, pub kind: PropertyKind, pub property: String }
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Uncertainty {
    OpaqueCommand(String),
    NestedAliasSuppressed(String),
    MalformedCommand,
    NonAsciiName,
    SourceLimit,
    LineLimit,
    TooManyAliases,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Analysis { pub targets: BTreeSet<Target>, pub uncertainties: BTreeSet<Uncertainty> }
#[derive(Clone, Copy, Debug)]
pub struct Alias<'a> { pub name: &'a str, pub command: &'a str }
#[derive(Debug, Eq, PartialEq)]
pub enum Comparison { PotentialOverlap { targets: Vec<Target>, uncertain: bool }, Indeterminate, NoOverlap }

/// Compare targets only after the caller checks context compatibility. An
/// overlap is potential, not proof of a gameplay conflict: axis parameters,
/// actor properties, activation conditions and script commands are not resolved.
/// `NoOverlap` means no overlap among modeled input-property targets only.
pub fn compare(a: &Analysis, b: &Analysis) -> Comparison {
    let common: Vec<_> = a.targets.intersection(&b.targets).cloned().collect();
    if !common.is_empty() { Comparison::PotentialOverlap { targets: common, uncertain: !a.uncertainties.is_empty() || !b.uncertainties.is_empty() } }
    else if !a.uncertainties.is_empty() || !b.uncertainties.is_empty() { Comparison::Indeterminate }
    else { Comparison::NoOverlap }
}

/// Analyze the canonical parser's four input phases, with aliases in actual
/// UInput array order. Only the first 40 entries exist in the engine. Alias
/// names must be the live, ordered metadata, not an inferred UI action list.
/// Unknown commands and suppressed nested aliases always make analysis uncertain.
pub fn analyze(binding: &str, aliases: &[Alias<'_>]) -> Analysis {
    let mut result = Analysis::default();
    if aliases.len() > MAX_ALIASES { result.uncertainties.insert(Uncertainty::TooManyAliases); }
    if aliases.iter().take(MAX_ALIASES).any(|a| !a.name.is_ascii() || a.name.contains('\0')) {
        result.uncertainties.insert(Uncertainty::NonAsciiName);
    }
    let mut budget = MAX_LINES;
    for phase in [Phase::Press, Phase::Hold, Phase::Release, Phase::Axis] {
        analyze_commands(binding, aliases, phase, false, &mut budget, &mut result);
    }
    result
}

fn wide(s: &str) -> Vec<Tchar> { s.chars().map(|c| c as Tchar).chain([0]).collect() }
fn text(s: &[Tchar]) -> String { s.iter().take_while(|&&c| c != 0).filter_map(|&c| char::from_u32(c as u32)).collect() }
fn matched(cursor: &mut *const Tchar, command: &str) -> bool {
    let value = wide(command);
    unsafe { parse_command(cursor, value.as_ptr()) != 0 }
}
fn token(cursor: &mut *const Tchar) -> Option<String> {
    let mut buf = [0; INPUT_LINE_SIZE];
    if unsafe { parse_token(cursor, buf.as_mut_ptr(), INPUT_LINE_SIZE as i32, false) } { Some(text(&buf)) } else { None }
}

fn analyze_commands(source: &str, aliases: &[Alias<'_>], phase: Phase, in_alias: bool, budget: &mut usize, result: &mut Analysis) {
    if source.contains('\0') || source.chars().count() > MAX_SOURCE_UNITS {
        result.uncertainties.insert(Uncertainty::SourceLimit); return;
    }
    let input = wide(source);
    let mut cursor = input.as_ptr();
    loop {
        if *budget == 0 { result.uncertainties.insert(Uncertainty::LineLimit); return; }
        let mut line = [0; INPUT_LINE_SIZE];
        let before = cursor;
        if !unsafe { parse_line(&mut cursor, line.as_mut_ptr(), INPUT_LINE_SIZE as i32, false) } { return; }
        *budget -= 1;
        let len = line.iter().position(|c| *c == 0).unwrap_or(INPUT_LINE_SIZE);
        // ParseLine's fixed buffer can split an oversized command into more
        // commands. Do not pretend such a split is fully analyzed.
        if len >= INPUT_LINE_SIZE - 1 { result.uncertainties.insert(Uncertainty::LineLimit); return; }
        if before == cursor { result.uncertainties.insert(Uncertainty::MalformedCommand); return; }
        let mut cmd = line.as_ptr();
        // ExecInputCommands strips OnRelease only on release, then routes it
        // through viewport Exec. For other phases it is an ordinary command.
        if phase == Phase::Release { matched(&mut cmd, "ONRELEASE"); }
        analyze_line(cmd, aliases, phase, in_alias, budget, result);
    }
}

fn analyze_line(mut cmd: *const Tchar, aliases: &[Alias<'_>], phase: Phase, in_alias: bool, budget: &mut usize, result: &mut Analysis) {
    let mut kind = None;
    for (name, property) in [("BUTTON", PropertyKind::Button), ("PULSE", PropertyKind::Button), ("TOGGLE", PropertyKind::Button), ("AXIS", PropertyKind::Axis)] {
        if matched(&mut cmd, name) { kind = Some(property); break; }
    }
    if let Some(kind) = kind {
        match token(&mut cmd) {
            Some(property) if property.is_ascii() => { result.targets.insert(Target { phase, kind, property: property.to_ascii_lowercase() }); }
            Some(_) => { result.uncertainties.insert(Uncertainty::NonAsciiName); }
            None => { result.uncertainties.insert(Uncertainty::MalformedCommand); }
        }
        return;
    }
    // UInput::Exec handles these before it attempts alias lookup.
    for builtin in ["KEYNAME", "KEYBINDING"] {
        if matched(&mut cmd, builtin) {
            result.uncertainties.insert(Uncertainty::OpaqueCommand(builtin.into()));
            return;
        }
    }
    let Some(name) = token(&mut cmd) else { return; };
    if !name.is_ascii() { result.uncertainties.insert(Uncertainty::NonAsciiName); return; }
    let alias = aliases.iter().take(MAX_ALIASES).find(|a| a.name.eq_ignore_ascii_case(&name));
    if let Some(alias) = alias {
        if in_alias {
            // with_alias_scope's guard blocks all nesting, including cycles.
            result.uncertainties.insert(Uncertainty::NestedAliasSuppressed(name));
        } else {
            analyze_commands(alias.command, aliases, phase, true, budget, result);
        }
    } else {
        // Viewport/console/script may handle unknown commands in some phases.
        result.uncertainties.insert(Uncertainty::OpaqueCommand(name));
    }
}
