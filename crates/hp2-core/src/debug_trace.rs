//! Env-gated runtime tracing for gameplay state and trigger diagnosis.
//!
//! `HP2_TRACE_STATES=1` prints every script `GotoState` (object, old state,
//! new state, label), every resolved VM function call (object, function),
//! and every `TriggerEvent` dispatch to stderr, so a live play session shows
//! exactly which state transitions and trigger targets the engine executes.
//! Off by default; the check is a cached atomic read.

use crate::core_access::CoreAccess;
use hp2_abi::engine::generated::{AActor, UObject};
use std::ffi::c_void;
use std::sync::atomic::{AtomicU8, Ordering};

static ENABLED: AtomicU8 = AtomicU8::new(0);

pub fn enabled() -> bool {
    match ENABLED.load(Ordering::Relaxed) {
        1 => true,
        2 => false,
        _ => {
            let on = std::env::var("HP2_TRACE_STATES").map(|v| !v.is_empty()).unwrap_or(false);
            ENABLED.store(if on { 1 } else { 2 }, Ordering::Relaxed);
            on
        }
    }
}

fn text(access: CoreAccess<'_>, name: i32) -> String {
    let Ok(units) = access.name_text(name) else {
        return String::new();
    };
    let mut out = String::new();
    for index in 0..units.len() {
        if units[index] == 0 {
            break;
        }
        if let Some(unit) = char::from_u32(units[index]) {
            out.push(unit);
        }
    }
    out
}

fn object_text(access: CoreAccess<'_>, object: *mut c_void) -> String {
    if object.is_null() {
        return "None".to_string();
    }
    let Ok(units) = (unsafe { access.object_full_name(object.cast()) }) else {
        return String::new();
    };
    let mut out = String::new();
    for index in 0..units.len() {
        if units[index] == 0 {
            break;
        }
        if let Some(unit) = char::from_u32(units[index]) {
            out.push(unit);
        }
    }
    out
}

pub fn goto_state(
    access: CoreAccess<'_>,
    object: *mut c_void,
    old_state: i32,
    new_state: i32,
    label: i32,
) {
    if !enabled() {
        return;
    }
    eprintln!(
        "HP2_STATE {} {} -> {}{}",
        object_text(access, object),
        text(access, old_state),
        text(access, new_state),
        if label != 0 { format!(" label={}", text(access, label)) } else { String::new() }
    );
}

pub fn vm_call(access: CoreAccess<'_>, object: *mut c_void, function: *mut c_void) {
    if !enabled() || function.is_null() {
        return;
    }
    let name = unsafe { (*(function.cast::<UObject>())).name };
    let function_name = text(access, i32::from_ne_bytes(name));
    let object_name = object_text(access, object);
    // Keep a play session readable: spell handlers, trigger plumbing, the
    // LumosLight TurnOn/Tick/TurnOff countdown, and the wand CastSpell path.
    let trace = function_name.contains("Trigger")
        || function_name.contains("Touch")
        || function_name.starts_with("Handle")
        || function_name == "TurnOn"
        || function_name == "TurnOff"
        || function_name == "CastSpell"
        || (function_name == "Tick" && object_name.contains("LumosLight"));
    if !trace {
        return;
    }
    let mut line = format!("HP2_CALL {object_name}::{function_name}");
    if function_name == "CastSpell" && !object.is_null() {
        let location = unsafe { (*(object.cast::<AActor>())).location };
        line.push_str(&format!(
            " wand_loc=({:.2},{:.2},{:.2})",
            location.x, location.y, location.z
        ));
    }
    eprintln!("{line}");
}

pub fn trigger_event(
    access: CoreAccess<'_>,
    instigator: *mut c_void,
    event: i32,
    target: *mut c_void,
) {
    if !enabled() {
        return;
    }
    eprintln!(
        "HP2_TRIGGER event={} from={} -> {}",
        text(access, event),
        object_text(access, instigator),
        object_text(access, target)
    );
}
