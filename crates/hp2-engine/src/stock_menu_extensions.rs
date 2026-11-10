//! Scoped engine-thread extensions for the stock sound/video page.
//!
//! The stock FPS adapter owns the single VM post-call hook. Independent
//! extensions can observe Created and Notify without replacing that hook or
//! editing its dispatcher. Each must validate its fields and control identities.
use std::{cell::{Cell, RefCell}, marker::PhantomData, rc::Rc};
use hp2_abi::engine::generated::UObject;
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PageEvent { Created, Notify }

/// The page and parameter frame are borrowed only for this synchronous call.
pub(crate) type Listener = for<'a> unsafe fn(CoreAccess<'a>, *mut UObject, PageEvent, *const u8, usize) -> RuntimeResult<()>;
thread_local! {
    static LISTENERS: RefCell<Vec<(&'static str, Listener)>> = const { RefCell::new(Vec::new()) };
    static TICK_ACTIVE: Cell<bool> = const { Cell::new(false) };
}

pub(crate) struct Guard { key: &'static str, _thread: PhantomData<Rc<()>> }
impl Drop for Guard {
    fn drop(&mut self) { LISTENERS.with(|slot| slot.borrow_mut().retain(|(key, _)| *key != self.key)); }
}

/// Register a module's listener on the engine thread. Keep its guard in the
/// module's owner state and drop it before unloading that owner or its packages.
/// A duplicate key fails instead of replacing a listener. Callbacks must not
/// retain page or parameter pointers. Delivery occurs only inside a tick with
/// the stock hook active. Controls must be hidden or disabled if it is not.
pub(crate) fn install(key: &'static str, listener: Listener) -> RuntimeResult<Guard> {
    LISTENERS.with(|slot| {
        let mut list = slot.borrow_mut();
        if list.iter().any(|(name, _)| *name == key) { return Err(RuntimeError::new("Stock page listener key already installed")); }
        list.push((key, listener));
        Ok(Guard { key, _thread: PhantomData })
    })
}

/// Whether the stock script hook is active for the current engine tick.
pub(crate) fn active() -> bool { TICK_ACTIVE.with(Cell::get) }

pub(crate) struct TickGuard { _thread: PhantomData<Rc<()>> }
impl Drop for TickGuard { fn drop(&mut self) { TICK_ACTIVE.with(|active| active.set(false)); } }

/// Begin delivery only after the stock hook has successfully been installed.
/// The tick guard must be dropped before the hook guard.
pub(crate) fn begin_tick() -> RuntimeResult<TickGuard> {
    TICK_ACTIVE.with(|active| {
        if active.replace(true) { return Err(RuntimeError::new("Nested stock menu tick")); }
        Ok(TickGuard { _thread: PhantomData })
    })
}

/// Calls every listener even if another fails. Returns the first failure.
pub(crate) unsafe fn dispatch(access: CoreAccess<'_>, page: *mut UObject, event: PageEvent, params: *const u8, size: usize) -> RuntimeResult<()> {
    if !active() { return Ok(()); }
    let listeners = LISTENERS.with(|slot| slot.borrow().clone());
    let mut first_error = None;
    for (_, listener) in listeners {
        if let Err(error) = unsafe { listener(access, page, event, params, size) } {
            if first_error.is_none() { first_error = Some(error); }
        }
    }
    first_error.map_or(Ok(()), Err)
}

#[cfg(test)]
mod tests {
    use super::*;
    unsafe fn listener(_: CoreAccess<'_>, _: *mut UObject, _: PageEvent, _: *const u8, _: usize) -> RuntimeResult<()> { Ok(()) }
    #[test]
    fn tick_delivery_scope_rejects_nesting() {
        assert!(!active());
        let guard = begin_tick().unwrap();
        assert!(active());
        assert!(begin_tick().is_err());
        drop(guard);
        assert!(!active());
    }
    #[test]
    fn listeners_are_keyed_and_scoped() {
        let a = install("a", listener).unwrap();
        let b = install("b", listener).unwrap();
        assert!(install("a", listener).is_err());
        drop(a);
        let replacement = install("a", listener).unwrap();
        drop(b);
        assert!(install("b", listener).is_ok());
        drop(replacement);
    }
}
