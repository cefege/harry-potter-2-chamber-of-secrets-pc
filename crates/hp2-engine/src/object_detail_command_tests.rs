use hp2_abi::engine::generated::{AActor, ALevelInfo, APlayerPawn, TArrayRaw, UEngine, ULevel, UObject, URender, UViewport};
use std::ptr;

#[test]
fn retail_menu_mlod_commands_reach_live_renderer_and_reflected_pawn_property() {
    let owner = crate::render_owner::render_test_support::owner("object-detail-command");
    let access = owner.access();
    unsafe {
        let make = |name| crate::render_owner::render_test_support::construct(access, name);
        let pawn = make("Engine.Camera").cast::<APlayerPawn>();
        let level = make("Engine.Level").cast::<ULevel>();
        let info = make("Engine.LevelInfo").cast::<ALevelInfo>();
        let engine = make("Engine.GameEngine").cast::<UEngine>();
        let render = make("Render.Render").cast::<URender>();
        let client = make("SDLDrv.SDLClient");
        let viewport = access.construct_object(access.native_class("SDLDrv.SDLViewport").unwrap(), client, 0, 0x4000, ptr::null_mut()).unwrap().cast::<UViewport>();
        (*info).set_b_begun_play(true);
        (*pawn.cast::<AActor>()).level = info.cast();
        (*pawn.cast::<AActor>()).x_level = level.cast();
        (*level).base.engine = engine.cast();
        (*engine).render = render.cast();
        (*viewport).actor = pawn.cast();
        (*pawn).player = viewport.cast();
        let name = access.intern_str("ObjectDetail", hp2_core::name_owner::FindName::Find).unwrap();
        let property = access.find_field((*pawn.cast::<UObject>()).class.cast(), name, access.native_class("Core.ByteProperty").unwrap());
        assert!(!property.is_null());
        let field = access.property_address(pawn.cast(), property.cast()).unwrap().cast::<u8>();
        let run = |text: &str| {
            let command: Vec<u32> = text.chars().map(u32::from).chain([0]).collect();
            let mut result = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
            crate::script_console_owner::player_console_command(access, pawn, command.as_ptr().cast(), (&raw mut result).cast()).unwrap();
            access.destroy_owned_fstring(&raw mut result).unwrap();
        };
        for (value, command, expected) in [(0, "MLOD 0.1", 0.1_f32), (1, "MLOD 0.2", 0.2), (2, "MLOD 0.5", 0.5), (3, "MLOD 0.8", 0.8), (4, "MLOD 1.0", 1.0)] {
            field.write(value);
            assert_eq!(field.read(), value);
            run(command);
            assert_eq!((*render).global_shape_lod, expected, "{command}");
        }
        run("MLOD 0.37");
        assert_eq!((*render).global_shape_lod, 0.37);
    }
}
