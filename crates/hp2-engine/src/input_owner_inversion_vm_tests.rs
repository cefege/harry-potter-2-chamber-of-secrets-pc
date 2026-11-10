use super::*;
use hp2_abi::engine::generated::{ALevelInfo, APlayerPawn, FFrame, UBoolProperty, ULevel, UFunction};
use hp2_core::array_storage::{self, Hp2ArraySlots};

fn wide(s: &str) -> Vec<u32> { s.chars().map(u32::from).chain([0]).collect() }
unsafe fn load(access: CoreAccess<'_>, name: &str) -> *mut hp2_abi::engine::generated::UClass {
    unsafe { access.static_load_class(access.native_class("Core.Object").unwrap(), ptr::null_mut(), &wide(name), 1).unwrap() }
}
unsafe fn create(access: CoreAccess<'_>, name: &str) -> *mut UObject {
    unsafe { access.construct_object(load(access, name), access.transient_package(), 0, 0, ptr::null_mut()).unwrap() }
}
unsafe fn field(access: CoreAccess<'_>, object: *mut UObject, name: &str, kind: &str) -> *mut UProperty {
    let class = unsafe { (*object).class };
    let name = access.intern_str(name, FindName::Find).unwrap();
    let p = unsafe { access.find_field(class.cast(), name, access.native_class(kind).unwrap()).cast::<UProperty>() };
    assert!(!p.is_null()); p
}
unsafe fn set_object(access: CoreAccess<'_>, object: *mut UObject, name: &str, value: *mut UObject) {
    unsafe { let p = field(access, object, name, "Core.ObjectProperty"); object.cast::<u8>().add((*p).offset as usize).cast::<*mut UObject>().write_unaligned(value); }
}
unsafe fn set_bool(access: CoreAccess<'_>, object: *mut UObject, name: &str, value: bool) {
    unsafe { let p = field(access, object, name, "Core.BoolProperty").cast::<UBoolProperty>(); let slot = object.cast::<u8>().add((*p.cast::<UProperty>()).offset as usize).cast::<u32>(); let word = slot.read_unaligned(); slot.write_unaligned(if value {word | (*p).bit_mask} else {word & !(*p).bit_mask}); }
}
unsafe fn read_bool(access: CoreAccess<'_>, object: *mut UObject, name: &str) -> bool {
    unsafe { let p = field(access, object, name, "Core.BoolProperty").cast::<UBoolProperty>(); let slot = object.cast::<u8>().add((*p.cast::<UProperty>()).offset as usize).cast::<u32>(); slot.read_unaligned() & (*p).bit_mask != 0 }
}
unsafe fn execute(access: CoreAccess<'_>, object: *mut UObject, function: *mut UFunction) -> RuntimeResult<()> {
    unsafe {
        let caller = access.construct_object(access.native_class("Core.Function")?, access.native_class("Core.Object")?.cast(), 0, 0, ptr::null_mut())?.cast::<UStruct>();
        let mut code = vec![0x1c]; code.extend((*function.cast::<UObject>()).index.to_le_bytes()); code.push(0x16);
        let slot = &raw mut (*caller).script;
        array_storage::set(Hp2ArraySlots { data: &raw mut (*slot).data, num: &raw mut (*slot).num, max: &raw mut (*slot).max }, code.len() as i32, 1, ptr::null(), array_storage::system_allocator()).unwrap();
        ptr::copy_nonoverlapping(code.as_ptr(), (*slot).data.cast(), code.len());
        let mut frame = FFrame { node: caller.cast(), object: object.cast(), code: (*slot).data.cast(), locals: ptr::null_mut(), parent: ptr::null_mut() };
        access.execute_state_frame_step(&raw mut frame, ptr::null_mut())
    }
}

#[test]
fn loaded_stock_page_inversion_ignores_auto_center_checkbox() {
    let owner = crate::render_owner::render_test_support::owner("stock-inversion-vm");
    let access = owner.access();
    let root_path = std::path::PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").unwrap()).join(format!("render-stock-inversion-vm-{}", std::process::id()));
    unsafe {
        access.load_package(&root_path.join("System/HGame.u"), 1).unwrap();
        let engine = create(access, "Engine.GameEngine");
        crate::engine_owner::init(access, std::ptr::NonNull::new(engine.cast()).unwrap()).unwrap();
        let level = create(access, "Engine.Level").cast::<ULevel>(); (*level).base.engine = engine.cast();
        let info = create(access, "Engine.LevelInfo").cast::<ALevelInfo>(); (*info).set_b_begun_play(true);
        let pawn = create(access, "Engine.Camera").cast::<APlayerPawn>();
        (*pawn.cast::<AActor>()).level = info.cast(); (*pawn.cast::<AActor>()).x_level = level.cast();
        let client = access.construct_object(access.native_class("SDLDrv.SDLClient").unwrap(), engine, 0, 0, ptr::null_mut()).unwrap();
        let viewport = access.construct_object(access.native_class("SDLDrv.SDLViewport").unwrap(), client, 0, 0, ptr::null_mut()).unwrap().cast::<UViewport>();
        (*viewport).actor = pawn.cast(); (*pawn).player = viewport.cast();
        let console = create(access, "UWindow.WindowConsole");
        (*console.cast::<hp2_abi::engine::generated::UConsole>()).viewport = viewport.cast(); (*viewport).console = console.cast();
        let root = create(access, "UWindow.UWindowRootWindow"); set_object(access, root, "Console", console);
        let page = create(access, "HGame.FEInputPage"); set_object(access, page, "Root", root);
        let mouse = create(access, "UWindow.UWindowCheckbox"); let center = create(access, "UWindow.UWindowCheckbox");
        set_object(access, page, "MouseInvertCheck", mouse); set_object(access, page, "AutoCenterCamCheck", center);
        let id = access.intern_str("MouseInvertChanged", FindName::Find).unwrap();
        let function = access.find_field((*page).class.cast(), id, access.native_class("Core.Function").unwrap()).cast::<UFunction>(); assert!(!function.is_null());
        for (inverted, auto_center) in [(false, true), (true, false), (false, false), (true, true)] {
            set_bool(access, mouse, "bChecked", inverted); set_bool(access, center, "bChecked", auto_center);
            execute(access, page, function).unwrap();
            let mut reopened = hp2_core::CoreConfig::new(root_path.clone(), root_path.join("System/Default.ini"), root_path.join("System/DefUser.ini"));
            let value = reopened.get(Some(&wide("User")[..4]), &wide("Engine.PlayerPawn")[..17], &wide("bInvertMouse")[..12]).unwrap().unwrap();
            let text: String = value.into_iter().map(|c| char::from_u32(c).unwrap()).collect();
            assert_eq!(text.to_ascii_lowercase(), if inverted { "true" } else { "false" }, "center={auto_center}");
            assert_eq!(read_bool(access, pawn.cast(), "bInvertMouse"), inverted);
            assert_eq!(read_bool(access, mouse, "bChecked"), inverted);
            assert_eq!(read_bool(access, center, "bChecked"), auto_center);
        }
    }
}
