use super::*;

fn wide(s: &str) -> Vec<u32> { s.chars().map(u32::from).chain([0]).collect() }

#[test]
fn stock_key_commands_edit_live_binding_and_user_cache() {
    let owner = crate::render_owner::render_test_support::owner("stock-set-input");
    let access = owner.access();
    let input = crate::render_owner::render_test_support::construct(access, "Engine.Input").cast::<UInput>();
    let mut viewport: UViewport = unsafe { mem::zeroed() };
    let actor = crate::render_owner::render_test_support::construct(access, "Engine.PlayerPawn");
    unsafe {
        (*input).viewport = (&raw mut viewport).cast();
        viewport.input = input.cast();
        viewport.actor = actor.cast();
    }
    let mut output = Vec::<u32>::new();
    let mut run = |text: &str| {
        output.clear();
        let mut sink = |units: &[u32]| { output.extend_from_slice(units); Ok(()) };
        let result = unsafe { exec(access, input, wide(text).as_ptr(), &mut sink) };
        (result, output.iter().copied().map(|u| char::from_u32(u).unwrap()).collect::<String>())
    };
    let (index, _) = unsafe { find_key(access, input, &wide("W")[..1]).unwrap().expect("W key") };
    assert_eq!(run(&format!("KEYNAME {index}")).1, "W");
    {
        let mut sink = |_: &[u32]| Ok(());
        let command = wide("SET Input W MoveForward|button bBroomPitchUp|button bSpellLessonUp");
        assert!(unsafe { crate::camera_owner::exec(access, &raw mut viewport, command.as_ptr(), &mut sink) }.unwrap());
    }
    assert_eq!(run("KEYBINDING w").1, "MoveForward|button bBroomPitchUp|button bSpellLessonUp");
    let cached = access.config_get_units_from(Some(&wide("User")[..4]), &wide("Engine.Input")[..12], &wide("W")[..1]).unwrap().unwrap();
    assert_eq!(cached.iter().copied().map(|u| char::from_u32(u).unwrap()).collect::<String>(), "MoveForward|button bBroomPitchUp|button bSpellLessonUp");
    let path = std::path::PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").unwrap())
        .join(format!("render-stock-set-input-{}/System/DefUser.ini", std::process::id()));
    let on_disk = std::fs::read_to_string(&path).unwrap();
    assert!(on_disk.contains("W=MoveForward|button bBroomPitchUp|button bSpellLessonUp"));
    assert!(run("SET Input W ").0.unwrap());
    assert_eq!(run("KEYBINDING W").1, "");
    assert!(!run("SET Other W Jump").0.unwrap());
    assert!(!run("SET Input NotAKey Jump").0.unwrap());
    // An external writer makes publication fail. The cache and live binding
    // must retain the previous value rather than accepting a partial edit.
    let mut external = std::fs::read(&path).unwrap();
    external.extend_from_slice(b"\n; external change\n");
    let replacement = path.with_extension("external");
    std::fs::write(&replacement, external).unwrap();
    std::fs::rename(replacement, &path).unwrap();
    assert!(run("SET Input W Jump").0.is_err());
    assert_eq!(run("KEYBINDING W").1, "");
    let cached = access.config_get_units_from(Some(&wide("User")[..4]), &wide("Engine.Input")[..12], &wide("W")[..1]).unwrap().unwrap();
    assert!(cached.is_empty());
    let defaults = unsafe { access.default_object((*input.cast::<UObject>()).class.cast()).unwrap().cast::<UInput>() };
    let default_text = unsafe { hp2_core::native_text::code_units(hp2_core::native_text::fstring_units(&(*defaults).bindings[index])) };
    assert!(default_text.is_empty(), "failed publication leaves class defaults unchanged");
}
