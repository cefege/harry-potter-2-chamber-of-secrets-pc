use hp2_abi::engine::generated::{AActor, ALevelInfo, APlayerPawn, FFrame, UFunction, ULevel, UObject, UProperty, URender, UStruct, UViewport};
use hp2_core::array_storage::{self, Hp2ArraySlots};
use hp2_core::name_owner::FindName;
use std::ptr;

#[test]
fn retail_stock_handler_changes_reflected_player_and_renderer() {
    let owner = crate::render_owner::render_test_support::owner("object-detail-menu-vm");
    let access = owner.access();
    let root_path = std::path::PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").unwrap()).join(format!("render-object-detail-menu-vm-{}", std::process::id()));
    unsafe {
        access.load_package(&root_path.join("System/HGame.u"), 1).unwrap();
        let create = |name| crate::render_owner::render_test_support::construct(access, name);
        let engine = create("Engine.GameEngine");
        crate::engine_owner::init(access, std::ptr::NonNull::new(engine.cast()).unwrap()).unwrap();
        let render = create("Render.Render").cast::<URender>();
        (*engine.cast::<hp2_abi::engine::generated::UEngine>()).render = render.cast();
        let level = create("Engine.Level").cast::<ULevel>(); (*level).base.engine = engine.cast();
        let info = create("Engine.LevelInfo").cast::<ALevelInfo>(); (*info).set_b_begun_play(true);
        let pawn = create("Engine.Camera").cast::<APlayerPawn>();
        (*pawn.cast::<AActor>()).level = info.cast(); (*pawn.cast::<AActor>()).x_level = level.cast();
        let client = access.construct_object(access.native_class("SDLDrv.SDLClient").unwrap(), engine, 0, 0, ptr::null_mut()).unwrap();
        let viewport = access.construct_object(access.native_class("SDLDrv.SDLViewport").unwrap(), client, 0, 0, ptr::null_mut()).unwrap().cast::<UViewport>();
        (*viewport).actor = pawn.cast(); (*pawn).player = viewport.cast();
        let console = create("UWindow.WindowConsole");
        (*console.cast::<hp2_abi::engine::generated::UConsole>()).viewport = viewport.cast(); (*viewport).console = console.cast();
        let root = create("UWindow.UWindowRootWindow");
        let page = create("HGame.FESoundVideoPage");
        let slider = create("HGame.HPMenuOptionHSlider");
        let field = |obj: *mut UObject, name: &str, kind: &str| {
            let id = access.intern_str(name, FindName::Find).unwrap();
            let property = access.find_field((*obj).class.cast(), id, access.native_class(kind).unwrap()).cast::<UProperty>();
            assert!(!property.is_null(), "missing {name}");
            access.property_address(obj, property).unwrap()
        };
        field(root, "Console", "Core.ObjectProperty").cast::<*mut UObject>().write_unaligned(console);
        field(page, "Root", "Core.ObjectProperty").cast::<*mut UObject>().write_unaligned(root);
        field(page, "ObjectDetailSlider", "Core.ObjectProperty").cast::<*mut UObject>().write_unaligned(slider);
        let value = field(slider, "Value", "Core.FloatProperty").cast::<f32>();
        let detail = field(pawn.cast(), "ObjectDetail", "Core.ByteProperty").cast::<u8>();
        let id = access.intern_str("ObjectDetailChanged", FindName::Find).unwrap();
        let function = access.find_field((*page).class.cast(), id, access.native_class("Core.Function").unwrap()).cast::<UFunction>(); assert!(!function.is_null());
        let caller = access.construct_object(access.native_class("Core.Function").unwrap(), access.native_class("Core.Object").unwrap().cast(), 0, 0, ptr::null_mut()).unwrap().cast::<UStruct>();
        let mut code = vec![0x1c]; code.extend((*function.cast::<UObject>()).index.to_le_bytes()); code.push(0x16);
        let slot = &raw mut (*caller).script;
        array_storage::set(Hp2ArraySlots { data: &raw mut (*slot).data, num: &raw mut (*slot).num, max: &raw mut (*slot).max }, code.len() as i32, 1, ptr::null(), array_storage::system_allocator()).unwrap();
        ptr::copy_nonoverlapping(code.as_ptr(), (*slot).data.cast(), code.len());
        for (position, expected_detail, expected_lod) in [(0., 4, 1.0_f32), (1., 3, 0.8), (2., 2, 0.5), (3., 1, 0.2), (4., 0, 0.1)] {
            value.write_unaligned(position);
            let mut frame = FFrame { node: caller.cast(), object: page.cast(), code: (*slot).data.cast(), locals: ptr::null_mut(), parent: ptr::null_mut() };
            access.execute_state_frame_step(&raw mut frame, ptr::null_mut()).unwrap();
            assert_eq!(detail.read(), expected_detail, "slider {position}");
            assert_eq!((*render).global_shape_lod, expected_lod, "slider {position}");
        }
    }
}
