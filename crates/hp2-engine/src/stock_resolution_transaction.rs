//! Connect stock script scopes to the SDL resolution preview transaction.
use hp2_abi::engine::generated::{TArrayRaw, UBoolProperty, UClass, UConsole, UFunction, UObject, UProperty, UViewport};
use hp2_core::{core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName, ScriptScopePhase};
use std::{cell::RefCell, ptr};

use crate::stock_resolution_lifecycle::{self, Event};
use crate::camera_owner::sdl_display_exec as backend;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Answer { Yes, No, Timeout }
#[derive(Debug, PartialEq, Eq)]
enum Finish { Confirm, FinishRollback, Rollback }
fn answer(result: &[u32], yes: &[u32], timeout: bool) -> Answer {
    if timeout { Answer::Timeout } else if !result.is_empty() && !yes.is_empty() && result == yes { Answer::Yes } else { Answer::No }
}
fn finish(answer: Answer, succeeded: bool) -> Finish {
    match answer { Answer::Yes if succeeded => Finish::Confirm, Answer::No => Finish::FinishRollback, _ => Finish::Rollback }
}
fn complete(
    action: Finish,
    confirm: impl FnOnce() -> RuntimeResult<()>,
    finish_rollback: impl FnOnce() -> RuntimeResult<()>,
    rollback: impl FnOnce() -> RuntimeResult<()>,
) -> RuntimeResult<()> {
    match action {
        Finish::Confirm => confirm(),
        Finish::FinishRollback => finish_rollback(),
        Finish::Rollback => rollback(),
    }
}
fn parse_setres(text: &str) -> RuntimeResult<Option<(i32, i32)>> {
    let (verb, args) = text.trim().split_once(char::is_whitespace).unwrap_or((text.trim(), ""));
    if !verb.eq_ignore_ascii_case("SetRes") { return Ok(None); }
    let parts: Vec<_> = args.trim().split(['x','X']).collect();
    let (width, height, depth) = if parts.len() == 3 {
        (parts[0].parse::<i32>().ok(), parts[1].parse::<i32>().ok(), parts[2].parse::<i32>().ok())
    } else { (None, None, None) };
    let (Some(width), Some(height), Some(depth)) = (width, height, depth) else { return Err(RuntimeError::new("Invalid stock SetRes")); };
    if width <= 0 || height <= 0 || depth <= 0 { return Err(RuntimeError::new("Invalid stock SetRes dimensions")); }
    Ok(Some((width, height)))
}
enum Frame {
    Ignore,
    Settings { viewport: *mut UViewport, viewport_index: i32, begun: bool },
    Done { viewport: *mut UViewport, viewport_index: i32, answer: Answer,
        console: *mut UObject, console_index: i32, timeout_before: Vec<u32>, old_settings: Vec<u32> },
}
thread_local! {
    static FRAMES: RefCell<Vec<Frame>> = const { RefCell::new(Vec::new()) };
    static TIMEOUTS: RefCell<Vec<(*mut UViewport, i32, usize)>> = const { RefCell::new(Vec::new()) };
}
fn live(a: CoreAccess<'_>, object: *mut UObject) -> bool {
    !object.is_null() && (0..a.object_count()).any(|i| a.object_at(i) == object.cast())
}
unsafe fn property(a: CoreAccess<'_>, object: *mut UObject, name: &str, kind: &str, width: usize) -> RuntimeResult<(*mut UProperty, *mut u8)> {
    if !live(a, object) { return Err(RuntimeError::new("Resolution transaction object is not live")); }
    let class = unsafe { (*object).class.cast::<UClass>() };
    let p = unsafe { a.find_field(class.cast(), a.intern_str(name, FindName::Find)?, a.native_class(kind)?) }.cast::<UProperty>();
    if p.is_null() { return Err(RuntimeError::new("Resolution transaction property missing")); }
    let offset = usize::try_from(unsafe { (*p).offset }).map_err(|_| RuntimeError::new("Resolution transaction property offset"))?;
    if unsafe { (*p).array_dim != 1 || (*p).element_size as usize != width }
        || offset.checked_add(width).is_none_or(|n| n > unsafe { (*class).properties_size.max(0) as usize }) {
        return Err(RuntimeError::new("Resolution transaction property layout"));
    }
    Ok((p, unsafe { a.property_address(object, p)? }.cast()))
}
unsafe fn object(a: CoreAccess<'_>, owner: *mut UObject, name: &str) -> RuntimeResult<*mut UObject> {
    let (_, p) = unsafe { property(a, owner, name, "Core.ObjectProperty", std::mem::size_of::<*mut UObject>())? };
    Ok(unsafe { p.cast::<*mut UObject>().read_unaligned() })
}
unsafe fn string(a: CoreAccess<'_>, owner: *mut UObject, name: &str) -> RuntimeResult<Vec<u32>> {
    let (_, p) = unsafe { property(a, owner, name, "Core.StrProperty", std::mem::size_of::<TArrayRaw>())? };
    let v = unsafe { p.cast::<TArrayRaw>().read_unaligned() };
    if v.num < 0 || v.max < v.num || v.num > 4096 || (v.num > 0 && v.data.is_null()) { return Err(RuntimeError::new("Invalid resolution transaction string")); }
    if v.num == 0 { return Ok(Vec::new()); }
    let units = unsafe { std::slice::from_raw_parts(v.data.cast::<u32>(), v.num as usize) };
    if units.last() != Some(&0) || units[..units.len()-1].contains(&0) { return Err(RuntimeError::new("Invalid resolution transaction string terminator")); }
    Ok(units[..units.len()-1].to_vec())
}
unsafe fn boolean(a: CoreAccess<'_>, owner: *mut UObject, name: &str) -> RuntimeResult<bool> {
    let (p, data) = unsafe { property(a, owner, name, "Core.BoolProperty", 4)? };
    let mask = unsafe { (*p.cast::<UBoolProperty>()).bit_mask };
    if mask.count_ones() != 1 { return Err(RuntimeError::new("Invalid resolution transaction boolean")); }
    Ok(unsafe { data.cast::<u32>().read_unaligned() } & mask != 0)
}
unsafe fn page_viewport(a: CoreAccess<'_>, page: *mut UObject) -> RuntimeResult<(*mut UViewport, *mut UObject)> {
    let root = unsafe { object(a, page, "Root")? };
    let console = unsafe { object(a, root, "Console")? };
    if !live(a, console) || !unsafe { hp2_core::object_model::is_a(console.cast(), a.native_class("Engine.Console")?.cast()) } { return Err(RuntimeError::new("Resolution page console unavailable")); }
    let viewport = unsafe { (*console.cast::<UConsole>()).viewport.cast::<UViewport>() };
    if !live(a, viewport.cast()) || unsafe { (*viewport).console.cast::<UObject>() } != console
        || !unsafe { hp2_core::object_model::is_a(viewport.cast(), a.native_class("SDLDrv.SDLViewport")?.cast()) } {
        return Err(RuntimeError::new("Resolution page viewport unavailable"));
    }
    Ok((viewport, console))
}
fn owner_id(a: CoreAccess<'_>) -> RuntimeResult<usize> {
    a.owner_context::<crate::core_owner::EngineState>().map(|owner| owner as *const _ as usize)
        .ok_or_else(|| RuntimeError::new("Resolution engine owner unavailable"))
}
fn viewport_live(a: CoreAccess<'_>, viewport: *mut UViewport, index: i32) -> bool {
    index >= 0 && a.object_at(index) == viewport.cast()
}
fn warn(a: CoreAccess<'_>, text: &str) {
    if let Ok(name) = a.intern_str("Warning", FindName::Add) { let _ = a.emit_log(name, &format!("Resolution transaction: {text}").chars().map(u32::from).collect::<Vec<_>>()); }
}

/// Called immediately before viewport command dispatch. Only a SetRes issued
/// inside this exact page's SettingsChanged scope can begin a transaction.
pub(crate) unsafe fn before_command(a: CoreAccess<'_>, viewport: *mut UViewport, command: *const u32) -> RuntimeResult<()> {
    let eligible = FRAMES.with(|frames| matches!(frames.borrow().last(), Some(Frame::Settings { viewport: p, viewport_index: i, begun: false }) if *p == viewport && viewport_live(a, *p, *i)));
    if !eligible { return Ok(()); }
    let mut units = Vec::new();
    for i in 0..4096 { let c = unsafe { command.add(i).read() }; if c == 0 { break; } units.push(c); }
    let Some(text) = units.iter().copied().map(char::from_u32).collect::<Option<String>>() else { return Err(RuntimeError::new("Invalid stock resolution command")); };
    let Some((width, height)) = parse_setres(&text)? else { return Ok(()); };
    unsafe { backend::begin_preview(a, viewport, width, height)?; }
    FRAMES.with(|frames| if let Some(Frame::Settings { begun, .. }) = frames.borrow_mut().last_mut() { *begun = true; });
    Ok(())
}

pub(crate) unsafe fn callback(a: CoreAccess<'_>, page: *mut UObject, f: *mut UFunction, params: *const u8, size: usize, phase: ScriptScopePhase) -> RuntimeResult<()> {
    match phase {
        ScriptScopePhase::Enter => {
            let event = unsafe { stock_resolution_lifecycle::classify(a, page, f, params, size)? };
            let frame = match event {
                Some(Event::SettingsChanged) => {
                    if !unsafe { boolean(a, page, "bInitialized")? } { Frame::Ignore } else {
                        let (viewport, _) = unsafe { page_viewport(a, page)? };
                        Frame::Settings { viewport, viewport_index: unsafe { (*viewport.cast::<UObject>()).index }, begun: false }
                    }
                }
                Some(Event::WindowDone { window }) => {
                    let confirm = unsafe { object(a, page, "ConfirmSettings")? };
                    if window.is_null() || window != confirm || !live(a, window) { Frame::Ignore }
                    else {
                        let (viewport, console) = unsafe { page_viewport(a, page)? };
                        if !backend::preview_pending(viewport) { Frame::Ignore } else {
                            let result = unsafe { string(a, window, "Result")? };
                            let timeout = unsafe { boolean(a, window, "bClosedFromTick")? };
                            let button1 = unsafe { object(a, window, "button1")? };
                            let yes = if live(a, button1) { unsafe { string(a, button1, "Text")? } } else { Vec::new() };
                            let answer = answer(&result, &yes, timeout);
                            let timeout_before = unsafe { string(a, console, "ResTimeOutSettings")? };
                            let old_settings = unsafe { string(a, page, "OldSettings")? };
                            if matches!(answer, Answer::No) { unsafe { backend::arm_rollback(a, viewport)?; } }
                            Frame::Done { viewport, viewport_index: unsafe { (*viewport.cast::<UObject>()).index }, answer,
                                console, console_index: unsafe { (*console).index }, timeout_before, old_settings }
                        }
                    }
                }
                None => Frame::Ignore,
            };
            FRAMES.with(|frames| frames.borrow_mut().push(frame));
            Ok(())
        }
        ScriptScopePhase::Exit { succeeded } => {
            let frame = FRAMES.with(|frames| frames.borrow_mut().pop()).ok_or_else(|| RuntimeError::new("Unbalanced resolution lifecycle scope"))?;
            match frame {
                Frame::Settings { viewport, viewport_index, begun: true } if !succeeded && viewport_live(a, viewport, viewport_index) && backend::preview_pending(viewport) => {
                    if let Err(e) = unsafe { backend::rollback_preview(a, viewport) } { warn(a, &format!("preview cleanup: {e}")); }
                }
                Frame::Done { viewport, viewport_index, answer, console, console_index, timeout_before, old_settings } if viewport_live(a, viewport, viewport_index) => {
                    if matches!(answer, Answer::Timeout) && console_index >= 0 && a.object_at(console_index) == console.cast() {
                        // The stock timeout path writes a deferred SetRes. Our
                        // backend owns the restore and any retry; preserve any
                        // unrelated earlier deferred request.
                        if unsafe { string(a, console, "ResTimeOutSettings")? } == old_settings {
                            let (_, p) = unsafe { property(a, console, "ResTimeOutSettings", "Core.StrProperty", std::mem::size_of::<TArrayRaw>())? };
                            let mut value = unsafe { p.cast::<TArrayRaw>().read_unaligned() };
                            let units: Vec<i32> = timeout_before.iter().map(|&unit| unit as i32).collect();
                            let result = unsafe { a.write_vm_fstring_result(&mut value, &units) };
                            unsafe { p.cast::<TArrayRaw>().write_unaligned(value); }
                            result?;
                        }
                    }
                    // Timeout originates inside AfterPaint. Defer the actual
                    // resize to the next post-tick safe point, like stock.
                    if matches!(answer, Answer::Timeout) {
                        let key = (viewport, viewport_index, owner_id(a)?);
                        TIMEOUTS.with(|pending| { let mut pending = pending.borrow_mut(); if !pending.contains(&key) { pending.push(key); } });
                    } else {
                        let result = complete(finish(answer, succeeded),
                            || unsafe { backend::confirm_preview(a, viewport) },
                            || unsafe { backend::finish_rollback(a, viewport) },
                            || unsafe { backend::rollback_preview(a, viewport) });
                        // The backend already attempted rollback and owns its
                        // retry queue when confirmation fails. Do not retry it
                        // synchronously or disturb that pending state here.
                        if let Err(e) = result { warn(a, &format!("completion: {e}")); }
                    }
                }
                _ => {}
            }
            Ok(())
        }
    }
}

/// Drain timeout restores only after the game tick, never from Paint.
pub(crate) unsafe fn process_timeouts(a: CoreAccess<'_>) {
    let Ok(owner) = owner_id(a) else { return; };
    let pending = TIMEOUTS.with(|slot| {
        let mut slot = slot.borrow_mut();
        let mut pending = Vec::new();
        slot.retain(|&(viewport, index, entry_owner)| {
            if entry_owner == owner { pending.push((viewport, index)); false } else { true }
        });
        pending
    });
    for (viewport, index) in pending {
        if viewport_live(a, viewport, index) && backend::preview_pending(viewport) {
            if let Err(error) = unsafe { backend::rollback_preview(a, viewport) } { warn(a, &format!("timeout rollback: {error}")); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_parser_only_begins_on_valid_setres() {
        assert_eq!(parse_setres("GetCurrentRes").unwrap(), None);
        assert_eq!(parse_setres("SetRes 1920x1080x32").unwrap(), Some((1920, 1080)));
        assert!(parse_setres("SetRes 0x720x32").is_err());
        assert!(parse_setres("SetRes 1280x720").is_err());
    }
    #[test]
    fn failed_confirmation_keeps_backend_retry_queued_without_second_rollback() {
        use std::cell::Cell;
        let confirms = Cell::new(0);
        let rollbacks = Cell::new(0);
        let queued = Cell::new(false);
        let result = complete(Finish::Confirm,
            || { confirms.set(confirms.get() + 1); queued.set(true); Err(RuntimeError::new("injected save and rollback failure")) },
            || { rollbacks.set(rollbacks.get() + 1); Ok(()) },
            || { rollbacks.set(rollbacks.get() + 1); Ok(()) });
        assert!(result.is_err());
        assert_eq!(confirms.get(), 1);
        assert_eq!(rollbacks.get(), 0);
        assert!(queued.get(), "backend retry remains queued");
    }
    #[test]
    fn stock_outcomes_have_one_backend_finish_path() {
        let yes = [89, 101, 115]; let no = [78, 111];
        assert_eq!(answer(&yes, &yes, false), Answer::Yes);
        assert_eq!(answer(&no, &yes, false), Answer::No);
        assert_eq!(answer(&[], &yes, true), Answer::Timeout);
        assert_eq!(answer(&[63], &yes, false), Answer::No);
        assert_eq!(finish(Answer::Yes, true), Finish::Confirm);
        assert_eq!(finish(Answer::Yes, false), Finish::Rollback);
        assert_eq!(finish(Answer::No, true), Finish::FinishRollback);
        assert_eq!(finish(Answer::No, false), Finish::FinishRollback);
        assert_eq!(finish(Answer::Timeout, true), Finish::Rollback);
    }
}
