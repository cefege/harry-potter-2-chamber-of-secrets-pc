//! Unwired live-binding transaction. Not a UI or complete rebinding API.
//!
//! `rebind` conservatively checks live aliases and keys before editing. It is
//! not a complete gameplay conflict resolver and is not connected to any UI.
use crate::{binding_foundation, binding_live_analysis::{self, KeyOverlap, LiveResult, Location, Reason}};
use hp2_abi::engine::generated::{FName, TArrayRaw, UEnum, UInput};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use std::ptr;

#[cfg(test)]
#[path = "input_binding_runtime_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "input_binding_rebind_tests.rs"]
mod rebind_tests;

/// A temporary Core-allocator FString; dropping frees whichever buffer it owns.
struct OwnedString(TArrayRaw);
impl OwnedString {
    fn empty() -> Self { Self(TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }) }
}
impl Drop for OwnedString {
    fn drop(&mut self) { unsafe { libc::free(self.0.data); } }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum RebindOutcome {
    Applied,
    Unchanged,
    PotentialConflicts(Vec<KeyOverlap>),
    Indeterminate { location: Location, reason: Reason },
}

/// Analyze every live binding before editing one key. Potential overlaps and
/// indeterminate analysis never mutate the object or configuration. The caller
/// may not bypass these outcomes; this remains unwired to the UI. Contexts,
/// alias-side scripts, and actor-level effects are not fully understood. Moving
/// an existing command across keys requires a multi-key atomic transaction;
/// this single-key operation does not implement that transaction.
///
/// # Safety
/// `input` must be a live canonical UInput, exclusively owned by this engine
/// thread throughout analysis and publication. Config writers must be serialized.
pub(crate) unsafe fn rebind(
    access: CoreAccess<'_>, input: *mut UInput, key: usize,
    remove: &[&str], add: &[&str],
) -> RuntimeResult<RebindOutcome> {
    if add.is_empty() { return Err(RuntimeError::new("Rebinding requires an added command")); }
    let old = unsafe { read_binding(input, key)? };
    let mut planned = old.clone();
    for command in remove {
        planned = binding_foundation::remove_exact(&planned, command)
            .map_err(|_| RuntimeError::new("Missing, ambiguous, or unsupported binding command"))?;
    }
    let mut new_commands = Vec::new();
    for command in add {
        let next = binding_foundation::add_exact(&planned, command)
            .map_err(|_| RuntimeError::new("Ambiguous or unsupported binding command"))?;
        if next != planned { new_commands.push(*command); }
        planned = next;
    }
    if planned == old { return Ok(RebindOutcome::Unchanged); }
    // Existing commands on the destination are not new conflicts with
    // themselves. All newly added commands are checked against every key.
    let proposed = new_commands.join("|");
    match unsafe { binding_live_analysis::analyze_live(access, input, &proposed) } {
        LiveResult::Indeterminate { location, reason } => Ok(RebindOutcome::Indeterminate { location, reason }),
        LiveResult::PotentialOverlap(conflicts) => Ok(RebindOutcome::PotentialConflicts(conflicts)),
        LiveResult::NoOverlap => match unsafe { edit(access, input, key, remove, add)? } {
            true => Ok(RebindOutcome::Applied),
            false => Ok(RebindOutcome::Unchanged),
        },
    }
}

/// Snapshot a live string while preserving the original allocation.
unsafe fn read_binding(input: *const UInput, key: usize) -> RuntimeResult<String> {
    if input.is_null() || key >= 255 { return Err(RuntimeError::new("Invalid input binding target")); }
    let raw = unsafe { ptr::addr_of!((*input).bindings[key]).read_unaligned() };
    if raw.num < 0 || raw.max < raw.num || (raw.num > 0 && raw.data.is_null()) || raw.num > 65_537 {
        return Err(RuntimeError::new("Invalid UInput binding storage"));
    }
    if raw.num == 0 { return Ok(String::new()); }
    let data = unsafe { std::slice::from_raw_parts(raw.data.cast::<u32>(), raw.num as usize) };
    if data.last() != Some(&0) || data[..data.len()-1].contains(&0) {
        return Err(RuntimeError::new("Invalid UInput binding termination"));
    }
    data[..data.len()-1].iter().copied().map(char::from_u32).collect::<Option<String>>()
        .ok_or_else(|| RuntimeError::new("Invalid UInput binding text"))
}

/// Updates one live binding and the canonical User cache, without resolving
/// aliases or conflicts. A missing, duplicate, or stale cached key is rejected.
/// This remains crate-private and must not be exposed as a complete rebinder.
///
/// # Safety
/// The same live-object, allocator, exclusivity and writer-serialization
/// requirements as `edit_with_publisher` apply.
pub(crate) unsafe fn edit(
    access: CoreAccess<'_>, input: *mut UInput, key: usize,
    remove: &[&str], add: &[&str],
) -> RuntimeResult<bool> {
    unsafe { edit_with_publisher(access, input, key, remove, add, |name, old, new| {
        // CoreConfig intentionally treats case-only value changes as unchanged.
        // Reject those, and conservatively reject equal-length non-ASCII edits,
        // so successful publication cannot leave object and cache different.
        if old.eq_ignore_ascii_case(new) || (old.chars().count() == new.chars().count() && (!old.is_ascii() || !new.is_ascii())) {
            return Err(RuntimeError::new("Binding change cannot be persisted safely"));
        }
        let wide = |s: &str| s.chars().map(u32::from).collect::<Vec<_>>();
        let section = wide("Engine.Input");
        let user = wide("User");
        let entries = access.config_section_units(&section, Some(&user))?;
        let mut matches = entries.iter().filter(|(k, _)| {
            let key: Option<String> = k.iter().copied().map(char::from_u32).collect();
            key.is_some_and(|key| key.eq_ignore_ascii_case(name))
        });
        let (_, value) = matches.next().ok_or_else(|| RuntimeError::new("User input key is absent"))?;
        if matches.next().is_some() || *value != wide(old) {
            return Err(RuntimeError::new("User input key is duplicate or differs from live binding"));
        }
        access.config_update_user_staged(&[(section, wide(name), wide(new))])
    }) }
}

/// Edits one live binding after the publisher confirms a durable config update.
/// This is deliberately crate-private and is not connected to input routing.
///
/// # Safety
/// `input` must remain a live, exclusively accessed canonical `UInput` through
/// the callback and swap. Its FString buffers must use Core's system allocator.
/// The callback must not reenter engine/object code or mutate/invalidate `input`.
/// It must atomically validate `old` against the canonical User config cache,
/// publish `new` to that cache and disk, and return an error without changing
/// either on failure. External configuration writers must be serialized.
/// Success does not imply aliases or conflicts have been checked.
pub(crate) unsafe fn edit_with_publisher(
    access: CoreAccess<'_>, input: *mut UInput, key: usize,
    remove: &[&str], add: &[&str],
    publish: impl FnOnce(&str, &str, &str) -> RuntimeResult<()>,
) -> RuntimeResult<bool> {
    if input.is_null() || key >= 255 { return Err(RuntimeError::new("Invalid input binding target")); }
    let metadata = unsafe { ptr::addr_of!((*input).input_keys).read_unaligned().cast::<UEnum>() };
    if metadata.is_null() { return Err(RuntimeError::new("UInput key metadata is unavailable")); }
    let names = unsafe { ptr::addr_of!((*metadata).names).read_unaligned() };
    if names.num != 255 || names.max < names.num || names.data.is_null() {
        return Err(RuntimeError::new("Invalid UInput key metadata"));
    }
    let name = unsafe { names.data.cast::<FName>().add(key).read_unaligned() };
    if name.index == 0 { return Err(RuntimeError::new("Unnamed UInput key")); }
    let text = access.name_text(name.index)?;
    let full: String = text.iter().copied().map(char::from_u32).collect::<Option<_>>()
        .ok_or_else(|| RuntimeError::new("Invalid UInput key name"))?;
    // StaticInitInput names the reflected config property with this suffix.
    let key_name = full.strip_prefix("IK_").or_else(|| full.strip_prefix("UK_"))
        .filter(|name| !name.is_empty())
        .ok_or_else(|| RuntimeError::new("Invalid UInput key name prefix"))?;
    let slot = unsafe { ptr::addr_of_mut!((*input).bindings[key]) };
    let old_raw = unsafe { slot.read_unaligned() };
    let old = unsafe { read_binding(input, key)? };
    let mut new = old.clone();
    for command in remove { new = binding_foundation::remove_exact(&new, command).map_err(|_| RuntimeError::new("Missing, ambiguous, or unsupported binding command"))?; }
    for command in add { new = binding_foundation::add_exact(&new, command).map_err(|_| RuntimeError::new("Ambiguous or unsupported binding command"))?; }
    if new == old { return Ok(false); }
    let units: Vec<u32> = new.chars().map(u32::from).chain([0]).collect();
    let mut prepared = OwnedString::empty();
    unsafe { access.assign_fstring(&mut prepared.0, &units)?; }
    // No engine object has changed before this call. On error, Drop frees the
    // prepared buffer. After success only infallible swaps and free remain.
    publish(key_name, &old, &new)?;
    let replacement = prepared.0;
    unsafe { slot.write_unaligned(replacement); }
    prepared.0 = old_raw;
    Ok(true)
}
