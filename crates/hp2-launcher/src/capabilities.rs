//! Replaces `HP2LauncherPlatform::CurrentCapabilities`
//! (`SDLLaunch/Src/HP2LauncherPlatform.cpp:35-39`, deleted). The real C++
//! body is exactly `LauncherCapabilities Result; return Result;` -- every
//! field keeps its default member initializer (`true`), so this build never
//! varies any of the four flags. Native `HP2Launcher::LauncherCapabilities`
//! (`HP2Launcher.h`) stays the public C++ type; `HP2Launcher::CurrentCapabilities`
//! (`HP2Launcher.cpp`) converts this struct's fields into it one-for-one.

/// Field order matches `HP2Launcher::LauncherCapabilities` exactly
/// (`screenModes, resolutions, revealPaths, nativeFolderPicker`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2LauncherCapabilities {
    pub screen_modes: bool,
    pub resolutions: bool,
    pub reveal_paths: bool,
    pub native_folder_picker: bool,
}

pub fn current_capabilities() -> Hp2LauncherCapabilities {
    Hp2LauncherCapabilities {
        screen_modes: true,
        resolutions: true,
        reveal_paths: true,
        native_folder_picker: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the real C++'s default-member-initialized return value (every
    /// capability `true`), cited above.
    #[test]
    fn all_capabilities_true() {
        let caps = current_capabilities();
        assert!(caps.screen_modes);
        assert!(caps.resolutions);
        assert!(caps.reveal_paths);
        assert!(caps.native_folder_picker);
    }
}
