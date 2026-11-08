//! Pure building blocks for a future binding editor. This module does not route input.
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingError {
    UnsupportedSyntax,
    InvalidCommand,
    AmbiguousCommand,
    CommandNotFound,
}

/// Split only unquoted pipes. Reject syntax whose engine interpretation would
/// make a lossless edit ambiguous (comments, newlines, NUL, unclosed quotes).
fn segments(s: &str) -> Result<Vec<&str>, BindingError> {
    let mut quoted = false;
    let mut start = 0;
    let mut out = Vec::new();
    let mut it = s.char_indices().peekable();
    while let Some((i, ch)) = it.next() {
        match ch {
            '\0' | '\n' | '\r' => return Err(BindingError::UnsupportedSyntax),
            '"' => quoted = !quoted,
            '/' if !quoted && it.peek().is_some_and(|(_, c)| *c == '/') => {
                return Err(BindingError::UnsupportedSyntax);
            }
            '|' if !quoted => { out.push(&s[start..i]); start = i + 1; }
            _ => {}
        }
    }
    if quoted { return Err(BindingError::UnsupportedSyntax); }
    out.push(&s[start..]);
    Ok(out)
}

fn validate_command(command: &str) -> Result<&str, BindingError> {
    let command = command.trim();
    let parts = segments(command)?;
    if command.is_empty() || parts.len() != 1 { return Err(BindingError::InvalidCommand); }
    Ok(command)
}

/// Exact, ASCII-case-insensitive command matching; arguments are part of the
/// command. Whitespace within a command is significant.
pub fn contains(binding: &str, command: &str) -> Result<bool, BindingError> {
    let command = validate_command(command)?;
    Ok(segments(binding)?.iter().any(|s| s.trim().eq_ignore_ascii_case(command)))
}

/// Remove complete matching commands only. Untouched bindings are returned
/// byte-for-byte; retained segments retain their original spelling and spacing.
pub fn remove(binding: &str, command: &str) -> Result<String, BindingError> {
    let command = validate_command(command)?;
    let parts = segments(binding)?;
    if !parts.iter().any(|p| p.trim().eq_ignore_ascii_case(command)) { return Ok(binding.to_owned()); }
    Ok(parts.into_iter().filter(|p| !p.trim().eq_ignore_ascii_case(command)).collect::<Vec<_>>().join("|"))
}

/// Remove exactly one byte-exact command. Refuse missing or duplicate matches.
/// This is intended for edits where unrelated quoted arguments must survive.
pub fn remove_exact(binding: &str, command: &str) -> Result<String, BindingError> {
    let command = validate_command(command)?;
    let parts = segments(binding)?;
    let count = parts.iter().filter(|part| part.trim() == command).count();
    if count == 0 { return Err(BindingError::CommandNotFound); }
    if count != 1 { return Err(BindingError::AmbiguousCommand); }
    Ok(parts.into_iter().filter(|part| part.trim() != command).collect::<Vec<_>>().join("|"))
}

/// Add an exact command, refusing case-only ambiguities instead of silently
/// treating potentially case-sensitive arguments as equivalent.
pub fn add_exact(binding: &str, command: &str) -> Result<String, BindingError> {
    let command = validate_command(command)?;
    let parts = segments(binding)?;
    if parts.iter().any(|p| p.trim() == command) { return Ok(binding.to_owned()); }
    if parts.iter().any(|p| p.trim().eq_ignore_ascii_case(command)) { return Err(BindingError::AmbiguousCommand); }
    if binding.trim().is_empty() { return Ok(command.to_owned()); }
    Ok(format!("{binding}|{command}"))
}

pub fn add(binding: &str, command: &str) -> Result<String, BindingError> {
    let command = validate_command(command)?;
    if contains(binding, command)? { return Ok(binding.to_owned()); }
    if binding.trim().is_empty() { return Ok(command.to_owned()); }
    Ok(format!("{binding}|{command}"))
}

/// The caller supplies action definitions, including those not shown in the UI.
/// Contexts are explicit: actions conflict only when their context sets overlap.
/// This is literal command detection, not complete runtime conflict detection:
/// it does not expand aliases, follow nested commands, or interpret `OnRelease`.
/// Callers must supply explicit command forms or resolve those separately.
#[derive(Debug, Clone)]
pub struct Action<'a> {
    pub id: &'a str,
    pub commands: &'a [&'a str],
    pub contexts: &'a [&'a str],
}

pub fn conflicts<'a>(binding: &str, selected: &Action<'_>, actions: &'a [Action<'a>]) -> Result<Vec<&'a str>, BindingError> {
    // Validate even when no contexts overlap.
    segments(binding)?;
    let mut result = Vec::new();
    for action in actions {
        if action.id == selected.id || !action.contexts.iter().any(|c| selected.contexts.contains(c)) { continue; }
        for command in action.commands {
            if contains(binding, command)? { result.push(action.id); break; }
        }
    }
    Ok(result)
}

/// Replace only the specified commands on one key. All other keys remain intact.
/// Callers decide which conflicts to remove, after presenting them to the user.
pub fn edit_key(bindings: &mut BTreeMap<String, String>, key: &str, remove_commands: &[&str], add_commands: &[&str]) -> Result<(), BindingError> {
    let original = bindings.get(key).map(String::as_str).unwrap_or("");
    let mut value = original.to_owned();
    for command in remove_commands { value = remove(&value, command)?; }
    for command in add_commands { value = add(&value, command)?; }
    if value != original { bindings.insert(key.to_owned(), value); }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureResult { Consume, Captured(u16), Cancelled, Pass }

/// The input owner must deliver events in order and retain this state through
/// capture and draining. Keys whose presses reached gameplay must be supplied as
/// `forwarded`; the activating click belongs in `suppressed` only if its press
/// was consumed. The sets must be disjoint. A previously forwarded release is
/// passed even during capture, so gameplay can clear its held state.
#[derive(Debug, Default)]
pub struct Capture { active: bool, forwarded: BTreeSet<u16>, suppressed: BTreeSet<u16> }
impl Capture {
    pub fn begin(&mut self, forwarded: impl IntoIterator<Item = u16>, suppressed: impl IntoIterator<Item = u16>) {
        let mut forwarded: BTreeSet<_> = forwarded.into_iter().collect();
        let mut suppressed: BTreeSet<_> = suppressed.into_iter().collect();
        forwarded.extend(&self.forwarded);
        suppressed.extend(&self.suppressed);
        assert!(forwarded.is_disjoint(&suppressed), "capture key cannot be both forwarded and suppressed");
        self.active = true;
        self.forwarded = forwarded;
        self.suppressed = suppressed;
    }
    pub fn cancel(&mut self) { self.active = false; }
    pub fn event(&mut self, key: u16, pressed: bool, cancel_key: bool) -> CaptureResult {
        if self.forwarded.contains(&key) {
            if !pressed { self.forwarded.remove(&key); return CaptureResult::Pass; }
            return CaptureResult::Consume;
        }
        if self.suppressed.contains(&key) {
            if !pressed { self.suppressed.remove(&key); }
            return CaptureResult::Consume;
        }
        if self.active {
            if !pressed { return CaptureResult::Consume; }
            self.suppressed.insert(key);
            self.active = false;
            return if cancel_key { CaptureResult::Cancelled } else { CaptureResult::Captured(key) };
        }
        CaptureResult::Pass
    }
}
