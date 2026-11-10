//! Live stock-menu graphics settings which do not require a GPU transaction.
//!
//! The caller must validate object identities and serialize configuration writes
//! on the engine thread. The save is staged before the infallible field write:
//! a failed save therefore cannot change the runtime or the displayed value.
//! Native text off only clears the client flag; it does not alter bitmap paths.
use hp2_abi::engine::generated::{UClient, UGameEngine, USDLClient};
use hp2_core::core_access::CoreAccess;
use crate::{settings_config, settings_policy::SettingsPatch};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Change {
    FrameLimit(i32),
    /// Percent, matching the launcher's discrete UI scale choices.
    UiScale(u16),
    VerticalFov(bool),
    NativeText(bool),
    ScreenFlashes(bool),
}

impl Change {
    pub(crate) fn patch(self) -> SettingsPatch {
        let mut patch = SettingsPatch::default();
        match self {
            Self::FrameLimit(v) => patch.frame_rate_limit = Some(v),
            Self::UiScale(v) => patch.ui_scale = Some(f64::from(v) / 100.0),
            Self::VerticalFov(v) => patch.maintain_vertical_fov = Some(v),
            Self::NativeText(v) => patch.native_text = Some(v),
            Self::ScreenFlashes(v) => patch.screen_flashes = Some(v),
        }
        patch
    }
}

#[derive(Debug)]
pub(crate) enum ApplyError<E> { Invalid, Save(E) }

/// Live fields are borrowed only for this synchronous transaction. `client`
/// must be the actual SDL client belonging to `engine`; both objects must stay
/// alive and exclusively accessible until the call returns.
unsafe fn apply_with<E>(engine: *mut UGameEngine, client: *mut USDLClient, change: Change,
    save: impl FnOnce(&SettingsPatch) -> Result<(), E>) -> Result<(), ApplyError<E>> {
    if engine.is_null() || client.is_null() || !(client as usize).is_multiple_of(std::mem::align_of::<UClient>()) || unsafe { (*engine).client } != client.cast() {
        return Err(ApplyError::Invalid);
    }
    let patch = change.patch();
    patch.plan().map_err(|_| ApplyError::Invalid)?;
    save(&patch).map_err(ApplyError::Save)?;
    // No fallible operation or callback may occur between save and field write.
    unsafe {
        match change {
            Change::FrameLimit(v) => (*engine).frame_rate_limit = v as f32,
            Change::UiScale(v) => std::ptr::addr_of_mut!((*client).ui_scale).write_unaligned(f32::from(v) / 100.0),
            Change::VerticalFov(v) => (*client.cast::<UClient>()).set_maintain_vertical_fov(v),
            Change::NativeText(v) => (*client.cast::<UClient>()).set_native_text(v),
            Change::ScreenFlashes(v) => (*client.cast::<UClient>()).set_screen_flashes(v),
        }
    }
    Ok(())
}

/// Persist and apply one validated setting. The caller must validate the engine
/// and SDL client classes and lifetimes before passing the pointers.
pub(crate) unsafe fn apply(access: CoreAccess<'_>, engine: *mut UGameEngine, client: *mut USDLClient,
    change: Change) -> Result<(), ApplyError<settings_config::SettingsError>> {
    unsafe { apply_with(engine, client, change, |patch| settings_config::apply(access, patch).map(|_| ())) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[repr(align(8))]
    struct AlignedClient(USDLClient);

    #[test]
    fn concrete_fields_follow_successful_saves_and_failure_leaves_them_unchanged() {
        // These ABI mirrors contain only integers, raw pointers, and floats.
        let mut engine: UGameEngine = unsafe { std::mem::zeroed() };
        let mut client = AlignedClient(unsafe { std::mem::zeroed() });
        engine.client = (&raw mut client.0).cast();
        let e = &raw mut engine; let c = &raw mut client.0;
        let changes = [Change::FrameLimit(120), Change::UiScale(125), Change::VerticalFov(true),
            Change::NativeText(true), Change::ScreenFlashes(true)];
        for change in changes {
            unsafe { apply_with(e, c, change, |_| Ok::<_, ()>(())).unwrap(); }
        }
        assert_eq!(engine.frame_rate_limit, 120.0);
        assert_eq!(unsafe { std::ptr::addr_of!((*c).ui_scale).read_unaligned() }, 1.25);
        let base = unsafe { &*c.cast::<UClient>() };
        assert!(base.maintain_vertical_fov() && base.native_text() && base.screen_flashes());
        for change in [Change::FrameLimit(30), Change::UiScale(75), Change::VerticalFov(false),
            Change::NativeText(false), Change::ScreenFlashes(false)] {
            assert!(matches!(unsafe { apply_with(e, c, change, |_| Err("disk")) }, Err(ApplyError::Save("disk"))));
        }
        assert_eq!(engine.frame_rate_limit, 120.0);
        assert_eq!(unsafe { std::ptr::addr_of!((*c).ui_scale).read_unaligned() }, 1.25);
        let base = unsafe { &*c.cast::<UClient>() };
        assert!(base.maintain_vertical_fov() && base.native_text() && base.screen_flashes());
        unsafe { apply_with(e, c, Change::NativeText(false), |_| Ok::<_, ()>(())).unwrap(); }
        assert!(!unsafe { (*c.cast::<UClient>()).native_text() });
    }

    #[test]
    fn invalid_values_and_mismatched_client_do_not_save() {
        let mut engine: UGameEngine = unsafe { std::mem::zeroed() };
        let mut client = AlignedClient(unsafe { std::mem::zeroed() });
        let e = &raw mut engine; let c = &raw mut client.0;
        assert!(matches!(unsafe { apply_with(e, c, Change::FrameLimit(60), |_| -> Result<(), ()> { panic!("must not save") }) }, Err(ApplyError::Invalid)));
        engine.client = c.cast();
        for change in [Change::FrameLimit(61), Change::UiScale(99)] {
            assert!(matches!(unsafe { apply_with(e, c, change, |_| -> Result<(), ()> { panic!("must not save") }) }, Err(ApplyError::Invalid)));
        }
    }
}
