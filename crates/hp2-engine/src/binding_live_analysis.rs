//! Conservative analysis of one proposed command against a live UInput snapshot.
//! This is internal, does not edit bindings, and is not wired to any UI.
use crate::binding_analysis::{self, Alias, Target, Uncertainty};
use hp2_abi::engine::generated::{TArrayRaw, UInput};
use hp2_core::core_access::CoreAccess;
use std::{collections::BTreeSet, ptr};

const MAX_STRING_UNITS: usize = 65_536;
const MAX_SNAPSHOT_UNITS: usize = 262_144;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct KeyOverlap { pub key: u8, pub targets: Vec<Target> }
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Location { Candidate, Alias(usize), Key(u8) }
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Reason { InvalidObject, InvalidString, InvalidName, SnapshotLimit, Analysis(BTreeSet<Uncertainty>) }
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum LiveResult {
    /// Potential target overlap, not a confirmed gameplay conflict.
    PotentialOverlap(Vec<KeyOverlap>),
    /// Only states that the modeled targets have no overlap.
    NoOverlap,
    /// No decision is safe, even if some potential overlaps were already found.
    Indeterminate { location: Location, reason: Reason },
}
fn uncertain(location: Location, reason: Reason) -> LiveResult { LiveResult::Indeterminate { location, reason } }

/// Copies a bounded canonical FString. The caller guarantees a valid allocation.
unsafe fn copy_string(raw: TArrayRaw, budget: &mut usize) -> Result<String, Reason> {
    if raw.num < 0 || raw.max < raw.num || (raw.num > 0 && raw.data.is_null()) { return Err(Reason::InvalidString); }
    if raw.num == 0 { return Ok(String::new()); }
    let len = raw.num as usize;
    if len > MAX_STRING_UNITS + 1 || len > *budget { return Err(Reason::SnapshotLimit); }
    *budget -= len;
    let data = unsafe { std::slice::from_raw_parts(raw.data.cast::<u32>(), len) };
    if data.last() != Some(&0) || data[..len-1].contains(&0) { return Err(Reason::InvalidString); }
    data[..len-1].iter().copied().map(char::from_u32).collect::<Option<String>>().ok_or(Reason::InvalidString)
}

/// Compare a candidate command to all 255 current key bindings. All keys are
/// considered, including the candidate's possible current key and hidden keys;
/// this function does not know gameplay contexts or action identity. The caller
/// must not interpret a shared target as a confirmed conflict or `NoOverlap` as
/// proof about unmodeled gameplay. Any uncertain alias or binding produces
/// `Indeterminate`, even when another binding already overlaps.
///
/// # Safety
/// `input` is a live canonical UInput. Its alias names and FString allocations
/// must remain valid and unchanged for this entire call, on the engine thread.
/// No concurrent writer, destruction, or reentrant mutation is permitted.
pub(crate) unsafe fn analyze_live(access: CoreAccess<'_>, input: *const UInput, candidate: &str) -> LiveResult {
    if input.is_null() { return uncertain(Location::Candidate, Reason::InvalidObject); }
    let candidate_len = candidate.chars().take(MAX_STRING_UNITS + 1).count();
    if candidate_len > MAX_STRING_UNITS || candidate_len > MAX_SNAPSHOT_UNITS {
        return uncertain(Location::Candidate, Reason::SnapshotLimit);
    }
    let mut budget = MAX_SNAPSHOT_UNITS - candidate_len;
    let mut aliases = Vec::new();
    for index in 0..40 {
        let entry = unsafe { ptr::addr_of!((*input).aliases[index]).read_unaligned() };
        let name_index = i32::from_ne_bytes(entry.alias);
        // NAME_None slots are not searched by UInput::Exec.
        if name_index == 0 { continue; }
        let name = match access.name_text(name_index) {
            Ok(units) => match units.into_iter().map(char::from_u32).collect::<Option<String>>() {
                Some(name) => name,
                None => return uncertain(Location::Alias(index), Reason::InvalidName),
            },
            Err(_) => return uncertain(Location::Alias(index), Reason::InvalidName),
        };
        if name.chars().count() > budget { return uncertain(Location::Alias(index), Reason::SnapshotLimit); }
        budget -= name.chars().count();
        let command = match unsafe { copy_string(entry.command, &mut budget) } {
            Ok(value) => value,
            Err(reason) => return uncertain(Location::Alias(index), reason),
        };
        aliases.push((name, command));
    }
    let alias_views: Vec<_> = aliases.iter().map(|(name, command)| Alias { name, command }).collect();
    let candidate = binding_analysis::analyze(candidate, &alias_views);
    if !candidate.uncertainties.is_empty() {
        return uncertain(Location::Candidate, Reason::Analysis(candidate.uncertainties));
    }
    let mut overlaps = Vec::new();
    for key in 0..255usize {
        let raw = unsafe { ptr::addr_of!((*input).bindings[key]).read_unaligned() };
        let binding = match unsafe { copy_string(raw, &mut budget) } {
            Ok(value) => value,
            Err(reason) => return uncertain(Location::Key(key as u8), reason),
        };
        if binding.is_empty() { continue; }
        let analysis = binding_analysis::analyze(&binding, &alias_views);
        if !analysis.uncertainties.is_empty() {
            return uncertain(Location::Key(key as u8), Reason::Analysis(analysis.uncertainties));
        }
        let targets: Vec<_> = candidate.targets.intersection(&analysis.targets).cloned().collect();
        if !targets.is_empty() { overlaps.push(KeyOverlap { key: key as u8, targets }); }
    }
    if overlaps.is_empty() { LiveResult::NoOverlap } else { LiveResult::PotentialOverlap(overlaps) }
}

#[cfg(test)]
#[path = "binding_live_analysis_tests.rs"]
mod tests;
