//! Experimental settings pages backed by the canonical engine config cache.
use hp2_abi::engine::generated::UClient;
use hp2_core::core_access::CoreAccess;
use crate::in_game_overlay::{self, Action, Content};
use crate::{settings_config, settings_policy::SettingsPatch};

const ROOT: &str = "Settings";
const DISPLAY: &str = "Display";
const PERFORMANCE: &str = "Performance";
const SAVE_ERROR: &str = "Could not save setting; value unchanged.";
const READ_ERROR: &str = "Could not read configuration.";
const LIMITS: [i32; 5] = [0, 30, 60, 120, 144];

fn action(id: &str, label: String, enabled: bool) -> Action {
    Action { id: id.into(), label, enabled }
}
fn value<'a>(snapshot: &'a settings_config::SettingsSnapshot, key: &str) -> Option<&'a str> {
    snapshot.values.iter().find(|(k, _)| k.key == key).and_then(|(_, v)| v.as_deref())
}
fn limit(value: Option<&str>) -> Option<i32> {
    let n: f64 = value?.trim().parse().ok()?;
    LIMITS.iter().copied().find(|v| n == f64::from(*v))
}
fn next_limit(value: Option<&str>) -> Option<i32> {
    limit(value).and_then(|v| LIMITS.iter().position(|x| *x == v))
        .map(|i| LIMITS[(i + 1) % LIMITS.len()])
}
fn next_vsync(value: Option<&str>) -> Option<bool> {
    let v = value?.trim();
    if ["on", "true", "yes", "1"].iter().any(|s| v.eq_ignore_ascii_case(s)) { Some(false) }
    else if ["off", "false", "no", "0", "adaptive"].iter().any(|s| v.eq_ignore_ascii_case(s)) { Some(true) }
    else { None }
}
fn stored(value: Option<&str>) -> String {
    match value { None => "not set".into(), Some(v) if v.len() <= 32 && !v.chars().any(char::is_control) => v.into(), Some(_) => "invalid".into() }
}
fn content(page: &str, client: &UClient, config: Option<&settings_config::SettingsSnapshot>, error: Option<&str>) -> Content {
    let mut status = vec!["Up/Down: select | Enter: choose | F10/Escape: close".into()];
    if page == PERFORMANCE { status.push("Stored values; restart required to apply.".into()); }
    if let Some(e) = error { status.push(e.into()); }
    let actions = match page {
        DISPLAY => vec![action("fps", format!("FPS counter: {}", if client.show_fps() { "On" } else { "Off" }), true), action("back", "Back to categories".into(), true)],
        PERFORMANCE => vec![
            action("vsync", format!("VSync (restart): {}", stored(config.and_then(|s| value(s, "UseVSync")))), next_vsync(config.and_then(|s| value(s, "UseVSync"))).is_some()),
            action("limit", format!("Frame limit (restart): {}", stored(config.and_then(|s| value(s, "FrameRateLimit")))), next_limit(config.and_then(|s| value(s, "FrameRateLimit"))).is_some()),
            action("back", "Back to categories".into(), true),
        ],
        _ => vec![action("display", "Display".into(), true), action("performance", "Performance".into(), true)],
    };
    Content { title: page.into(), status, actions }
}

/// Publish and commit config before touching the live FPS flag. Other settings
/// only change configuration. Call on the engine thread with the owning client.
pub(crate) fn sync(access: CoreAccess<'_>, viewport: usize, client: &mut UClient) {
    let previous = in_game_overlay::snapshot(viewport);
    let mut page = match previous.as_ref().map(|(p, _)| p.title.as_str()) { Some(DISPLAY) => DISPLAY, Some(PERFORMANCE) => PERFORMANCE, _ => ROOT };
    let mut error = previous.as_ref().and_then(|(p, _)| p.status.iter().find(|s| s.as_str() == SAVE_ERROR).map(|_| SAVE_ERROR));
    let mut config = settings_config::read(access).ok();
    while let Some(id) = in_game_overlay::take_action(viewport) {
        match id.as_str() {
            "display" => { page = DISPLAY; error = None; },
            "performance" => { page = PERFORMANCE; error = None; },
            "back" => { page = ROOT; error = None; },
            "fps" => {
                let v = !client.show_fps();
                if settings_config::apply(access, &SettingsPatch { show_fps: Some(v), ..Default::default() }).is_ok() {
                    client.set_show_fps(v); error = None;
                } else { error = Some(SAVE_ERROR); }
            },
            "vsync" | "limit" => {
                if let Some(snapshot) = config.as_ref() {
                    let patch = if id == "vsync" {
                        next_vsync(value(snapshot, "UseVSync")).map(|v| SettingsPatch { vertical_sync: Some(v), ..Default::default() })
                    } else {
                        next_limit(value(snapshot, "FrameRateLimit")).map(|v| SettingsPatch { frame_rate_limit: Some(v), ..Default::default() })
                    };
                    if let Some(patch) = patch {
                        if settings_config::apply(access, &patch).is_ok() {
                            config = settings_config::read(access).ok(); error = None;
                        } else { error = Some(SAVE_ERROR); }
                    }
                } else { error = Some(READ_ERROR); }
            },
            _ => {},
        }
    }
    if config.is_none() && error.is_none() { error = Some(READ_ERROR); }
    in_game_overlay::set_content(viewport, content(page, client, config.as_ref(), error));
}

#[cfg(test)]
mod tests {
    use super::*;
    use hp2_core::core_access::{CoreOptions, CoreOwner, RuntimeResult};
    use std::{fs, path::PathBuf, sync::atomic::{AtomicUsize, Ordering}};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }
    fn owner(root: &PathBuf) -> CoreOwner {
        CoreOwner::new(CoreOptions { data_root: root.clone(), system_ini: root.join("Game.ini"), user_ini: root.join("User.ini"), command_line: vec![], output, log_file: None }).unwrap()
    }
    fn setup() -> (PathBuf, CoreOwner) {
        let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR required"))
            .join(format!("overlay-provider-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("Game.ini"), b"[SDLDrv.SDLClient]\nShowFPS=False\nOther=keep\n[Engine.GameEngine]\nFrameRateLimit=60.000000\n[WgpuDrv.WgpuRenderDevice]\nUseVSync=Adaptive\n").unwrap();
        let owner = owner(&root); (root, owner)
    }
    fn client() -> UClient { unsafe { std::mem::zeroed() } }
    fn event(viewport: usize, access: CoreAccess<'_>, client: &mut UClient, key: i32) { assert!(in_game_overlay::event(viewport, key, 1)); sync(access, viewport, client); assert!(in_game_overlay::event(viewport, key, 3)); }
    fn open(viewport: usize, access: CoreAccess<'_>, client: &mut UClient) { event(viewport, access, client, 121); }
    fn enter(viewport: usize, access: CoreAccess<'_>, client: &mut UClient) { event(viewport, access, client, 13); }
    fn down(viewport: usize, access: CoreAccess<'_>, client: &mut UClient) { event(viewport, access, client, 40); }
    fn page(viewport: usize) -> Content { in_game_overlay::snapshot(viewport).unwrap().0 }
    #[test]
    fn overlay_settings_integration_success() {
        let (root, owner) = setup(); let access = owner.access(); let vp = usize::MAX - 87;
        let mut client = client(); open(vp, access, &mut client); enter(vp, access, &mut client); enter(vp, access, &mut client);
        assert!(client.show_fps()); assert!(page(vp).actions[0].label.contains("On"));
        let file = fs::read_to_string(root.join("Game.ini")).unwrap(); assert!(file.contains("ShowFPS=True")); assert!(file.contains("Other=keep"));
        in_game_overlay::destroy_viewport(vp); owner.shutdown().unwrap();
    }
    #[test]
    fn overlay_settings_integration_restart_settings_persist_across_owner_restart() {
        let (root, first) = setup(); let access = first.access(); let vp = usize::MAX - 89;
        let mut client = client(); open(vp, access, &mut client); down(vp, access, &mut client); enter(vp, access, &mut client);
        assert_eq!(page(vp).title, PERFORMANCE); assert_eq!(in_game_overlay::snapshot(vp).unwrap().1, 0); assert!(page(vp).status.iter().any(|s| s.contains("restart required")));
        assert!(page(vp).actions[0].label.contains("Adaptive")); enter(vp, access, &mut client);
        down(vp, access, &mut client); enter(vp, access, &mut client);
        assert!(!client.show_fps()); assert!(page(vp).actions[0].label.contains("On")); assert!(page(vp).actions[1].label.contains("120"));
        down(vp, access, &mut client); enter(vp, access, &mut client);
        assert_eq!(page(vp).title, ROOT); assert_eq!(in_game_overlay::snapshot(vp).unwrap().1, 0);
        in_game_overlay::destroy_viewport(vp); first.shutdown().unwrap();
        let second = owner(&root); let snapshot = settings_config::read(second.access()).unwrap();
        assert_eq!(value(&snapshot, "UseVSync"), Some("On")); assert_eq!(value(&snapshot, "FrameRateLimit"), Some("120")); second.shutdown().unwrap();
    }
    #[test]
    fn overlay_settings_integration_conflict_keeps_values_and_shows_error() {
        let (root, owner) = setup(); let access = owner.access(); let vp = usize::MAX - 88;
        let mut client = client(); open(vp, access, &mut client); down(vp, access, &mut client); enter(vp, access, &mut client);
        let external = b"[SDLDrv.SDLClient]\nShowFPS=False\nOther=external\n"; fs::write(root.join("Game.ini"), external).unwrap();
        enter(vp, access, &mut client); assert!(!client.show_fps());
        assert!(page(vp).status.iter().any(|s| s == SAVE_ERROR)); assert!(page(vp).actions[0].label.contains("Adaptive"));
        down(vp, access, &mut client); enter(vp, access, &mut client); assert!(page(vp).status.iter().any(|s| s == SAVE_ERROR));
        assert!(page(vp).actions[1].label.contains("60.000000")); assert_eq!(fs::read(root.join("Game.ini")).unwrap(), external);
        in_game_overlay::destroy_viewport(vp); owner.shutdown().unwrap(); assert_eq!(fs::read(root.join("Game.ini")).unwrap(), external);
    }
    #[test]
    fn overlay_settings_integration_fps_conflict_preserves_live_flag() {
        let (root, owner) = setup(); let access = owner.access(); let vp = usize::MAX - 91;
        let mut client = client(); open(vp, access, &mut client); enter(vp, access, &mut client);
        let external = b"[SDLDrv.SDLClient]\nShowFPS=False\nOther=external\n";
        fs::write(root.join("Game.ini"), external).unwrap(); enter(vp, access, &mut client);
        assert!(!client.show_fps()); assert!(page(vp).status.iter().any(|s| s == SAVE_ERROR));
        assert_eq!(fs::read(root.join("Game.ini")).unwrap(), external);
        in_game_overlay::destroy_viewport(vp); owner.shutdown().unwrap();
    }
    #[test]
    fn overlay_settings_integration_invalid_values_are_disabled_and_unchanged() {
        let (root, first) = setup(); first.shutdown().unwrap();
        let original = b"[WgpuDrv.WgpuRenderDevice]\nUseVSync=Surprise\n[Engine.GameEngine]\nFrameRateLimit=999\n";
        fs::write(root.join("Game.ini"), original).unwrap();
        let owner = owner(&root); let access = owner.access(); let vp = usize::MAX - 90; let mut client = client();
        open(vp, access, &mut client); down(vp, access, &mut client); enter(vp, access, &mut client);
        let p = page(vp); assert!(!p.actions[0].enabled); assert!(!p.actions[1].enabled);
        enter(vp, access, &mut client); down(vp, access, &mut client); enter(vp, access, &mut client);
        assert_eq!(fs::read(root.join("Game.ini")).unwrap(), original);
        in_game_overlay::destroy_viewport(vp); owner.shutdown().unwrap();
    }
    #[test] fn stored_parsing_does_not_invent_values() {
        assert_eq!(limit(Some("144.000000")), Some(144)); assert_eq!(limit(Some("NaN")), None);
        assert_eq!(stored(None), "not set"); assert_eq!(next_limit(Some("unexpected")), None);
        for v in ["on", "TRUE", "Yes", "1"] { assert_eq!(next_vsync(Some(v)), Some(false)); }
        for v in ["off", "FALSE", "No", "0", "Adaptive"] { assert_eq!(next_vsync(Some(v)), Some(true)); }
        for v in [None, Some("bad"), Some("")] { assert_eq!(next_vsync(v), None); }
    }
}
