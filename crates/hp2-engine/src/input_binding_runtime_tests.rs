use super::*;
use crate::render_owner::render_test_support;
use std::{fs, ptr};

fn wide(s: &str) -> Vec<u32> { s.chars().map(u32::from).collect() }
unsafe fn set(access: CoreAccess<'_>, input: *mut UInput, key: usize, value: &str) {
    let v: Vec<u32> = value.chars().map(u32::from).chain([0]).collect();
    unsafe { access.assign_fstring(ptr::addr_of_mut!((*input).bindings[key]), &v).unwrap(); }
}
unsafe fn get(input: *mut UInput, key: usize) -> String {
    let s = unsafe { ptr::addr_of!((*input).bindings[key]).read_unaligned() };
    if s.num == 0 { return String::new(); }
    unsafe { std::slice::from_raw_parts(s.data.cast::<u32>(), (s.num - 1) as usize) }
        .iter().map(|c| char::from_u32(*c).unwrap()).collect()
}
fn stage(access: CoreAccess<'_>, key: &str, value: &str) {
    access.config_update_user_staged(&[(wide("Engine.Input"), wide(key), wide(value))]).unwrap();
}
fn input(access: CoreAccess<'_>) -> *mut UInput {
    render_test_support::construct(access, "Engine.Input").cast()
}

#[test]
fn live_transaction_preserves_other_bindings_and_persists_in_user_cache() {
    let owner = render_test_support::owner("binding-runtime-success");
    let a = owner.access();
    let input = input(a);
    let original = "Button Jump|Say \"A|B\"|Button JumpHigher";
    let hidden = "Axis aForward Speed=2";
    stage(a, "A", original);
    stage(a, "B", hidden);
    unsafe { set(a, input, 65, original); set(a, input, 66, hidden); }
    assert!(unsafe { edit(a, input, 65, &["Button Jump"], &["Button Duck"]) }.unwrap());
    let expected = "Say \"A|B\"|Button JumpHigher|Button Duck";
    assert_eq!(unsafe { get(input, 65) }, expected);
    assert_eq!(unsafe { get(input, 66) }, hidden);
    assert_eq!(a.config_get_units_from(Some(&wide("User")), &wide("Engine.Input"), &wide("A")).unwrap(), Some(wide(expected)));
    let mut reload = a.new_config_cache();
    assert_eq!(reload.get(Some(&wide("User")), &wide("Engine.Input"), &wide("A")).unwrap(), Some(wide(expected)));
    assert_eq!(reload.get(Some(&wide("User")), &wide("Engine.Input"), &wide("B")).unwrap(), Some(wide(hidden)));
}

#[test]
fn failed_publish_preserves_real_object_and_canonical_cache() {
    let owner = render_test_support::owner("binding-runtime-failure");
    let a = owner.access();
    let input = input(a);
    let original = "Button Jump|Custom";
    stage(a, "A", original);
    unsafe { set(a, input, 65, original); }
    let path = std::path::PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").unwrap())
        .join(format!("render-binding-runtime-failure-{}/System/DefUser.ini", std::process::id()));
    let mut bytes = fs::read(&path).unwrap();
    bytes.extend_from_slice(b"\n; external change\n");
    // Prototype files can be read-only. Model an external atomic replacement
    // rather than changing permissions on the copied fixture.
    let external = path.with_extension("external");
    fs::write(&external, &bytes).unwrap();
    fs::rename(&external, &path).unwrap();
    assert!(unsafe { edit(a, input, 65, &["Button Jump"], &["Duck"]) }.is_err());
    assert_eq!(unsafe { get(input, 65) }, original);
    assert_eq!(a.config_get_units_from(Some(&wide("User")), &wide("Engine.Input"), &wide("A")).unwrap(), Some(wide(original)));
    assert_eq!(fs::read(path).unwrap(), bytes);
}

#[test]
fn mismatched_cache_and_case_only_changes_are_rejected() {
    let owner = render_test_support::owner("binding-runtime-stale");
    let a = owner.access();
    let input = input(a);
    stage(a, "A", "Jump|Other");
    unsafe { set(a, input, 65, "Jump|Different"); }
    assert!(unsafe { edit(a, input, 65, &["Jump"], &["Duck"]) }.is_err());
    assert_eq!(unsafe { get(input, 65) }, "Jump|Different");
    stage(a, "A", "Jump");
    unsafe { set(a, input, 65, "Jump"); }
    assert!(unsafe { edit(a, input, 65, &["Jump"], &["JUMP"]) }.is_err());
    assert_eq!(unsafe { get(input, 65) }, "Jump");
}
