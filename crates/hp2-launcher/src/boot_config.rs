//! Replaces `ApplyPortableConfig`
//! (`HarryPotter2/Unreal/SDLLaunch/Src/SDLLaunch.cpp`, formerly lines
//! 703-755): rewrites stale pre-portable render/audio/viewport device
//! bindings to their shipped portable equivalents before the engine loads
//! its classes. Native services retain `GConfig`/`debugf` and every TCHAR
//! buffer and conversion; Rust owns the comparison and migration decisions.

use std::ffi::c_void;

const LEGACY_RENDER_PREFIXES: &[&[u8]] = &[
    b"D3DDrv.",
    b"GlideDrv.",
    b"SoftDrv.",
    b"OpenGLDrv.",
    b"XOpenGLDrv.",
    b"VulkanDrv.",
    b"WinDrv.",
];

/// Replaces the anonymous `IsLegacyRenderDevice` lambda (705-723): an
/// ASCII case-insensitive *prefix* match (matching `appStrnicmp`, which
/// compares only `appStrlen(Prefix)` characters, i.e. a shorter `value`
/// never matches) against any of the stock-UE driver bindings this tree
/// does not ship.
fn is_legacy_render_device(value: &[u8]) -> bool {
    LEGACY_RENDER_PREFIXES
        .iter()
        .any(|prefix| value.len() >= prefix.len() && value[..prefix.len()].eq_ignore_ascii_case(prefix))
}

/// One native `[Engine.Engine]` key in the `System` ini, named rather than
/// spelled out so every leaf reuses the one fixed section/filename pair.
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2LauncherConfigKey {
    ViewportManager,
    GameRenderDevice,
    WindowedRenderDevice,
    AudioDevice,
}

impl Hp2LauncherConfigKey {
    /// The literal key name `debugf`'s `%s` prints for the two render keys
    /// (736): `RenderKeys`' own `TCHAR*` entries, spelled out in UTF-8.
    fn display_name(self) -> &'static str {
        match self {
            Hp2LauncherConfigKey::ViewportManager => "ViewportManager",
            Hp2LauncherConfigKey::GameRenderDevice => "GameRenderDevice",
            Hp2LauncherConfigKey::WindowedRenderDevice => "WindowedRenderDevice",
            Hp2LauncherConfigKey::AudioDevice => "AudioDevice",
        }
    }
}

/// Native `GConfig`/`debugf` access, scoped to exactly what `ApplyPortableConfig`
/// used. Every string crossing this boundary is UTF-8; native performs the
/// TCHAR conversion both ways, exactly as every other launcher leaf does.
///
/// Native callbacks contain C++ exceptions and guarded signals below Rust.
/// Every callback returns zero on success or its exact nonzero captured
/// failure.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2LauncherConfigServices {
    pub context: *mut c_void,
    /// `GConfig->GetString(TEXT("Engine.Engine"), <key>, Current,
    /// ARRAY_COUNT(Current), TEXT("System"))` (729, 739, 749): on a found
    /// key, UTF-8 bytes are written into `buf` (capacity `cap`) and
    /// `*out_len` receives the written length; on a key not found,
    /// `*out_len` is written as `-1` and `buf` is untouched.
    pub get_string: unsafe extern "C" fn(
        *mut c_void,
        Hp2LauncherConfigKey,
        *mut u8,
        usize,
        *mut i32,
    ) -> i32,
    /// `GConfig->SetString(TEXT("Engine.Engine"), <key>, <value>,
    /// TEXT("System"))` (732, 742, 752).
    pub set_string:
        unsafe extern "C" fn(*mut c_void, Hp2LauncherConfigKey, *const u8, usize) -> i32,
    /// `debugf(NAME_Init, TEXT("%s"), Buffer)` (733, 743, 753) after
    /// converting the already fully formatted UTF-8 message.
    pub log_init: unsafe extern "C" fn(*mut c_void, *const u8, usize) -> i32,
}

fn checked(status: i32) -> Result<(), i32> {
    if status == 0 { Ok(()) } else { Err(status) }
}

impl Hp2LauncherConfigServices {
    fn get(&self, key: Hp2LauncherConfigKey) -> Result<Option<Vec<u8>>, i32> {
        // `TCHAR Current[256]` holds at most 255 UTF-32 code points, each at
        // most 4 UTF-8 bytes, so every value the original could read fits.
        let mut buf = [0u8; 255 * 4];
        let mut len: i32 = -1;
        checked(unsafe { (self.get_string)(self.context, key, buf.as_mut_ptr(), buf.len(), &mut len) })?;
        if len < 0 {
            Ok(None)
        } else {
            Ok(Some(buf[..len as usize].to_vec()))
        }
    }

    fn set(&self, key: Hp2LauncherConfigKey, value: &[u8]) -> Result<(), i32> {
        checked(unsafe { (self.set_string)(self.context, key, value.as_ptr(), value.len()) })
    }

    fn log(&self, message: &str) -> Result<(), i32> {
        checked(unsafe { (self.log_init)(self.context, message.as_ptr(), message.len()) })
    }
}

/// Replaces `ApplyPortableConfig`'s full body (703-755).
/// # Safety
/// Services satisfy the checked callback contract; `get_string`'s buffer
/// argument is always a fresh Rust-owned stack buffer for the call's
/// duration.
pub unsafe fn apply(services: &Hp2LauncherConfigServices) -> Result<(), i32> {
    use Hp2LauncherConfigKey::*;

    // The only shipped viewport manager is SDLDrv.SDLClient (729-734).
    let viewport = services.get(ViewportManager)?;
    let viewport_needs_binding = match &viewport {
        None => true,
        Some(current) => !current.eq_ignore_ascii_case(b"SDLDrv.SDLClient"),
    };
    if viewport_needs_binding {
        services.set(ViewportManager, b"SDLDrv.SDLClient")?;
        services.log("Bound [Engine.Engine] ViewportManager=SDLDrv.SDLClient")?;
    }

    // Any legacy render device binding is rewritten to the shipped portable
    // one (736-745).
    for key in [GameRenderDevice, WindowedRenderDevice] {
        if let Some(current) = services.get(key)? {
            if is_legacy_render_device(&current) {
                services.set(key, b"WgpuDrv.WgpuRenderDevice")?;
                services.log(&format!(
                    "Migrated [Engine.Engine] {}={} to WgpuDrv.WgpuRenderDevice",
                    key.display_name(),
                    String::from_utf8_lossy(&current)
                ))?;
            }
        }
    }

    // ALAudio is the only shipped audio subsystem (747-754).
    if let Some(current) = services.get(AudioDevice)? {
        if !current.eq_ignore_ascii_case(b"ALAudio.ALAudioSubsystem") {
            services.set(AudioDevice, b"ALAudio.ALAudioSubsystem")?;
            services.log(&format!(
                "Migrated [Engine.Engine] AudioDevice={} to ALAudio.ALAudioSubsystem",
                String::from_utf8_lossy(&current)
            ))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_render_device_matches_every_stock_prefix_case_insensitively() {
        for prefix in LEGACY_RENDER_PREFIXES {
            let upper: Vec<u8> = prefix.iter().map(|b| b.to_ascii_uppercase()).collect();
            assert!(is_legacy_render_device(prefix));
            assert!(is_legacy_render_device(&upper));
            let mut with_suffix = prefix.to_vec();
            with_suffix.extend_from_slice(b"Foo");
            assert!(is_legacy_render_device(&with_suffix));
        }
    }

    #[test]
    fn legacy_render_device_rejects_non_legacy_and_short_values() {
        assert!(!is_legacy_render_device(b"WgpuDrv.WgpuRenderDevice"));
        assert!(!is_legacy_render_device(b"SDLDrv.SDLClient"));
        assert!(!is_legacy_render_device(b""));
        // Shorter than every prefix: appStrnicmp's length-bounded compare
        // never matches a value too short to contain the prefix.
        assert!(!is_legacy_render_device(b"D3D"));
    }

    struct FakeStore {
        viewport: Option<&'static [u8]>,
        game_render: Option<&'static [u8]>,
        windowed_render: Option<&'static [u8]>,
        audio: Option<&'static [u8]>,
    }

    thread_local! {
        static STORE: std::cell::RefCell<FakeStore> = std::cell::RefCell::new(FakeStore {
            viewport: None,
            game_render: None,
            windowed_render: None,
            audio: None,
        });
        static SETS: std::cell::RefCell<Vec<(&'static str, Vec<u8>)>> = const { std::cell::RefCell::new(Vec::new()) };
        static LOGS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    unsafe extern "C" fn fake_get(
        _ctx: *mut c_void,
        key: Hp2LauncherConfigKey,
        buf: *mut u8,
        cap: usize,
        out_len: *mut i32,
    ) -> i32 {
        STORE.with(|s| {
            let s = s.borrow();
            let value = match key {
                Hp2LauncherConfigKey::ViewportManager => s.viewport,
                Hp2LauncherConfigKey::GameRenderDevice => s.game_render,
                Hp2LauncherConfigKey::WindowedRenderDevice => s.windowed_render,
                Hp2LauncherConfigKey::AudioDevice => s.audio,
            };
            match value {
                None => unsafe { out_len.write(-1) },
                Some(v) => {
                    assert!(v.len() <= cap);
                    unsafe { std::ptr::copy_nonoverlapping(v.as_ptr(), buf, v.len()) };
                    unsafe { out_len.write(v.len() as i32) };
                }
            }
        });
        0
    }

    unsafe extern "C" fn fake_set(
        _ctx: *mut c_void,
        key: Hp2LauncherConfigKey,
        value: *const u8,
        value_len: usize,
    ) -> i32 {
        let bytes = unsafe { std::slice::from_raw_parts(value, value_len) }.to_vec();
        SETS.with(|s| s.borrow_mut().push((key.display_name(), bytes)));
        0
    }

    unsafe extern "C" fn fake_log(_ctx: *mut c_void, text: *const u8, len: usize) -> i32 {
        let text = unsafe { std::slice::from_raw_parts(text, len) };
        LOGS.with(|l| l.borrow_mut().push(String::from_utf8_lossy(text).into_owned()));
        0
    }

    fn run(store: FakeStore) -> (Vec<(&'static str, Vec<u8>)>, Vec<String>) {
        STORE.with(|s| *s.borrow_mut() = store);
        SETS.with(|s| s.borrow_mut().clear());
        LOGS.with(|l| l.borrow_mut().clear());
        let services = Hp2LauncherConfigServices {
            context: std::ptr::null_mut(),
            get_string: fake_get,
            set_string: fake_set,
            log_init: fake_log,
        };
        unsafe { apply(&services) }.unwrap();
        (SETS.with(|s| s.borrow().clone()), LOGS.with(|l| l.borrow().clone()))
    }

    #[test]
    fn already_portable_config_makes_no_changes() {
        let (sets, logs) = run(FakeStore {
            viewport: Some(b"SDLDrv.SDLClient"),
            game_render: Some(b"WgpuDrv.WgpuRenderDevice"),
            windowed_render: Some(b"WgpuDrv.WgpuRenderDevice"),
            audio: Some(b"ALAudio.ALAudioSubsystem"),
        });
        assert!(sets.is_empty());
        assert!(logs.is_empty());
    }

    #[test]
    fn missing_viewport_manager_is_bound() {
        let (sets, logs) = run(FakeStore {
            viewport: None,
            game_render: None,
            windowed_render: None,
            audio: None,
        });
        assert_eq!(sets, vec![("ViewportManager", b"SDLDrv.SDLClient".to_vec())]);
        assert_eq!(logs, vec!["Bound [Engine.Engine] ViewportManager=SDLDrv.SDLClient"]);
    }

    #[test]
    fn legacy_render_devices_migrate_with_original_value_in_the_log() {
        let (sets, logs) = run(FakeStore {
            viewport: Some(b"SDLDrv.SDLClient"),
            game_render: Some(b"D3DDrv.D3DRenderDevice"),
            windowed_render: Some(b"OpenGLDrv.OpenGLRenderDevice"),
            audio: Some(b"ALAudio.ALAudioSubsystem"),
        });
        assert_eq!(
            sets,
            vec![
                ("GameRenderDevice", b"WgpuDrv.WgpuRenderDevice".to_vec()),
                ("WindowedRenderDevice", b"WgpuDrv.WgpuRenderDevice".to_vec()),
            ]
        );
        assert_eq!(
            logs,
            vec![
                "Migrated [Engine.Engine] GameRenderDevice=D3DDrv.D3DRenderDevice to WgpuDrv.WgpuRenderDevice",
                "Migrated [Engine.Engine] WindowedRenderDevice=OpenGLDrv.OpenGLRenderDevice to WgpuDrv.WgpuRenderDevice",
            ]
        );
    }

    #[test]
    fn non_legacy_render_device_binding_is_left_alone() {
        let (sets, logs) = run(FakeStore {
            viewport: Some(b"SDLDrv.SDLClient"),
            game_render: Some(b"SomeFutureRenderDevice"),
            windowed_render: None,
            audio: Some(b"ALAudio.ALAudioSubsystem"),
        });
        assert!(sets.is_empty());
        assert!(logs.is_empty());
    }

    #[test]
    fn legacy_audio_device_migrates() {
        let (sets, logs) = run(FakeStore {
            viewport: Some(b"SDLDrv.SDLClient"),
            game_render: Some(b"WgpuDrv.WgpuRenderDevice"),
            windowed_render: Some(b"WgpuDrv.WgpuRenderDevice"),
            audio: Some(b"WinDrv.WindowsAudioDevice"),
        });
        assert_eq!(sets, vec![("AudioDevice", b"ALAudio.ALAudioSubsystem".to_vec())]);
        assert_eq!(
            logs,
            vec!["Migrated [Engine.Engine] AudioDevice=WinDrv.WindowsAudioDevice to ALAudio.ALAudioSubsystem"]
        );
    }

    #[test]
    fn viewport_comparison_is_case_insensitive() {
        let (sets, logs) = run(FakeStore {
            viewport: Some(b"sdldrv.sdlclient"),
            game_render: Some(b"WgpuDrv.WgpuRenderDevice"),
            windowed_render: Some(b"WgpuDrv.WgpuRenderDevice"),
            audio: Some(b"ALAudio.ALAudioSubsystem"),
        });
        assert!(sets.is_empty());
        assert!(logs.is_empty());
    }
}
