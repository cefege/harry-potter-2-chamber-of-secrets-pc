// Compile the component independently of the shared engine hook registration.
use hp2_engine::{settings_config, settings_policy};
#[path = "../src/stock_graphics_settings.rs"]
mod stock_graphics_settings;

use hp2_abi::engine::generated::{UGameEngine, USDLClient};
use hp2_core::core_access::{CoreAccess, CoreOptions, CoreOwner, RuntimeResult};
use stock_graphics_settings::{apply, Change};

fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }
#[repr(align(8))]
struct AlignedClient(USDLClient);

#[test]
fn real_cache_save_live_apply_and_reload() {
    let root = std::path::PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR required"))
        .join(format!("stock-graphics-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("Game.ini"), b"[SDLDrv.SDLClient]\nOther=keep\n").unwrap();
    let owner = || CoreOwner::new(CoreOptions { data_root: root.clone(), system_ini: root.join("Game.ini"),
        user_ini: root.join("User.ini"), command_line: vec![], output, log_file: None }).unwrap();
    let runtime = owner();
    let mut engine: UGameEngine = unsafe { std::mem::zeroed() };
    let mut client = AlignedClient(unsafe { std::mem::zeroed() });
    let c = &raw mut client.0;
    engine.client = c.cast();
    for change in [Change::FrameLimit(144), Change::UiScale(150), Change::VerticalFov(true),
        Change::NativeText(false), Change::ScreenFlashes(false)] {
        unsafe { apply(runtime.access(), &raw mut engine, c, change).unwrap() };
    }
    assert_eq!(engine.frame_rate_limit, 144.0);
    assert_eq!(unsafe { std::ptr::addr_of!((*c).ui_scale).read_unaligned() }, 1.5);
    runtime.shutdown().unwrap();
    let runtime = owner();
    let snapshot = settings_config::read(runtime.access()).unwrap();
    for (key, expected) in [("FrameRateLimit", "144"), ("UIScale", "1.5"), ("MaintainVerticalFOV", "True"),
        ("NativeText", "False"), ("ScreenFlashes", "False")] {
        let actual = &snapshot.values.iter().find(|(k, _)| k.key == key).unwrap().1;
        assert_eq!(actual.as_deref(), Some(expected), "{key}");
    }
    assert!(std::fs::read_to_string(root.join("Game.ini")).unwrap().contains("Other=keep"));
    runtime.shutdown().unwrap();
}

#[test]
fn real_cache_conflict_does_not_apply_live_value() {
    let root = std::path::PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("HP2_ARTIFACT_DIR required"))
        .join(format!("stock-graphics-conflict-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let ini = root.join("Game.ini");
    std::fs::write(&ini, b"[Engine.GameEngine]\nFrameRateLimit=60\n").unwrap();
    let runtime = CoreOwner::new(CoreOptions { data_root: root.clone(), system_ini: ini.clone(),
        user_ini: root.join("User.ini"), command_line: vec![], output, log_file: None }).unwrap();
    settings_config::read(runtime.access()).unwrap();
    let external = b"[Engine.GameEngine]\nFrameRateLimit=30\n";
    std::fs::write(&ini, external).unwrap();
    let mut engine: UGameEngine = unsafe { std::mem::zeroed() };
    let mut client = AlignedClient(unsafe { std::mem::zeroed() });
    engine.client = (&raw mut client.0).cast(); engine.frame_rate_limit = 60.0;
    let result = unsafe { apply(runtime.access(), &raw mut engine, &raw mut client.0, Change::FrameLimit(120)) };
    assert!(result.is_err()); assert_eq!(engine.frame_rate_limit, 60.0);
    assert_eq!(std::fs::read(&ini).unwrap(), external);
    runtime.shutdown().unwrap();
    assert_eq!(std::fs::read(&ini).unwrap(), external);
}
