use super::*;
use hp2_abi::engine::generated::{APlayerPawn, FFrame, UFunction};
use hp2_core::array_storage::{self, Hp2ArraySlots};

fn wide(s: &str) -> Vec<u32> { s.chars().map(u32::from).chain([0]).collect() }

unsafe fn console_command(access: CoreAccess<'_>, pawn: *mut APlayerPawn, value: &str) -> RuntimeResult<String> {
    unsafe {
        let class = access.native_class("Engine.PlayerPawn")?;
        let function = access.construct_object(access.native_class("Core.Function")?, class.cast(), access.intern_str("ConsoleCommand", FindName::Add)?, 0, ptr::null_mut())?.cast::<UFunction>();
        (*function).function_flags = 0x400;
        (*function).return_value_offset = u16::MAX;
        (*function.cast::<UStruct>()).properties_align = 1;
        (*function.cast::<UObject>()).object_flags |= hp2_core::object_lifecycle::RF_NEED_POST_LOAD;
        access.conditional_post_load(function.cast())?;
        let caller = access.construct_object(access.native_class("Core.Function")?, class.cast(), 0, 0, ptr::null_mut())?.cast::<UStruct>();
        let mut code = vec![0x1c];
        code.extend((*function.cast::<UObject>()).index.to_le_bytes());
        code.push(0x34);
        for unit in value.encode_utf16().chain([0]) { code.extend(unit.to_le_bytes()); }
        code.push(0x16);
        let script = &raw mut (*caller).script;
        array_storage::set(Hp2ArraySlots { data: &raw mut (*script).data, num: &raw mut (*script).num, max: &raw mut (*script).max }, code.len() as i32, 1, ptr::null(), array_storage::system_allocator()).unwrap();
        ptr::copy_nonoverlapping(code.as_ptr(), (*script).data.cast(), code.len());
        let start = (*script).data.cast::<u8>();
        let mut frame = FFrame { node: caller.cast(), object: pawn.cast(), code: start, locals: ptr::null_mut(), parent: ptr::null_mut() };
        let mut result: TArrayRaw = mem::zeroed();
        let operation = access.execute_state_frame_step(&raw mut frame, (&raw mut result).cast());
        let text: String = hp2_core::native_text::code_units(hp2_core::native_text::fstring_units(&raw const result)).iter().map(|&u| char::from_u32(u).unwrap()).collect();
        access.destroy_owned_fstring(&raw mut result)?;
        operation?;
        assert_eq!(frame.code, start.add(code.len()));
        Ok(text)
    }
}

#[test]
fn pawn_vm_console_command_routes_to_viewport_and_persists_binding() {
    let owner = crate::render_owner::render_test_support::owner("stock-vm-input");
    let access = owner.access();
    (|| -> RuntimeResult<()> { unsafe {
        let pawn = crate::render_owner::render_test_support::construct(access, "Engine.PlayerPawn").cast::<APlayerPawn>();
        let engine = crate::render_owner::render_test_support::construct(access, "Engine.GameEngine");
        crate::engine_owner::init(access, std::ptr::NonNull::new(engine.cast()).unwrap())?;
        static_init_input(access)?;
        let client = access.construct_object(access.native_class("SDLDrv.SDLClient")?, engine, 0, 0, ptr::null_mut())?;
        let viewport = access.construct_object(access.native_class("SDLDrv.SDLViewport")?, client, 0, 0, ptr::null_mut())?.cast::<UViewport>();
        let input = (*viewport).input.cast::<UInput>();
        assert!(!input.is_null());
        (*viewport).actor = pawn.cast();
        (*pawn).player = viewport.cast(); (*input).viewport = viewport.cast();
        let (index, _) = find_key(access, input, &wide("W")[..1])?.expect("W");
        assert_eq!(console_command(access, pawn, &format!("KEYNAME {index}"))?, "W");
        console_command(access, pawn, "SET Input W MoveForward|button bBroomPitchUp")?;
        assert_eq!(console_command(access, pawn, "KEYBINDING W")?, "MoveForward|button bBroomPitchUp");
        let root = std::path::PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").unwrap()).join(format!("render-stock-vm-input-{}", std::process::id()));
        let mut reopened = hp2_core::CoreConfig::new(root.clone(), root.join("System/Default.ini"), root.join("System/DefUser.ini"));
        let persisted = reopened.get(Some(&wide("User")[..4]), &wide("Engine.Input")[..12], &wide("W")[..1])?.unwrap();
        let expected = wide("MoveForward|button bBroomPitchUp");
        assert_eq!(persisted, expected[..expected.len() - 1]);
        let new_input = access.construct_object(access.native_class("Engine.Input")?, viewport.cast(), 0, 0, ptr::null_mut())?.cast::<UInput>();
        let binding = &(*new_input).bindings[index];
        let reloaded = hp2_core::native_text::code_units(hp2_core::native_text::fstring_units(binding));
        assert_eq!(reloaded, &expected[..expected.len() - 1], "new input must inherit the edited binding");
        Ok(())
    } })().unwrap();
}
