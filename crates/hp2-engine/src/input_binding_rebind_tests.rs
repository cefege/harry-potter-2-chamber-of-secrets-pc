use super::*;
use crate::render_owner::render_test_support;
use hp2_core::name_owner::FindName;
use std::{fs, ptr};
fn wide(s: &str) -> Vec<u32> { s.chars().map(u32::from).collect() }
unsafe fn set(a: CoreAccess<'_>, input: *mut UInput, index: usize, s: &str) {
    let v: Vec<u32> = s.chars().map(u32::from).chain([0]).collect();
    unsafe { a.assign_fstring(ptr::addr_of_mut!((*input).bindings[index]), &v).unwrap(); }
}
unsafe fn get(input: *mut UInput, index: usize) -> String {
    let s = unsafe { ptr::addr_of!((*input).bindings[index]).read_unaligned() };
    if s.num == 0 { return String::new(); }
    unsafe { std::slice::from_raw_parts(s.data.cast::<u32>(), (s.num-1) as usize) }.iter().map(|v| char::from_u32(*v).unwrap()).collect()
}
fn stage(a: CoreAccess<'_>, key: &str, value: &str) {
    a.config_update_user_staged(&[(wide("Engine.Input"), wide(key), wide(value))]).unwrap();
}
fn setup(a: CoreAccess<'_>) -> *mut UInput {
    let input: *mut UInput = render_test_support::construct(a, "Engine.Input").cast();
    unsafe {
        for i in 0..255 { set(a, input, i, ""); }
        for i in 0..40 {
            ptr::addr_of_mut!((*input).aliases[i].alias).write_unaligned(0i32.to_ne_bytes());
            a.assign_fstring(ptr::addr_of_mut!((*input).aliases[i].command), &[0]).unwrap();
        }
    }
    input
}
unsafe fn alias(a: CoreAccess<'_>, input: *mut UInput, index: usize, name: &str, command: &str) {
    let id = a.intern_str(name, FindName::Add).unwrap();
    let v: Vec<u32> = command.chars().map(u32::from).chain([0]).collect();
    unsafe {
        ptr::addr_of_mut!((*input).aliases[index].alias).write_unaligned(id.to_ne_bytes());
        a.assign_fstring(ptr::addr_of_mut!((*input).aliases[index].command), &v).unwrap();
    }
}

#[test]
fn safe_rebind_preserves_compound_and_hidden_bindings_in_object_and_disk() {
    let owner = render_test_support::owner("rebind-success"); let a = owner.access(); let input = setup(a);
    let old = "Button bJump|Axis aTurn Speed=2"; let hidden = "Axis aStrafe Speed=3";
    stage(a, "A", old); stage(a, "B", hidden);
    unsafe { set(a, input, 65, old); set(a, input, 66, hidden); }
    assert_eq!(unsafe { rebind(a, input, 65, &["Button bJump"], &["Button bDuck"]) }.unwrap(), RebindOutcome::Applied);
    assert_eq!(unsafe { get(input, 65) }, "Axis aTurn Speed=2|Button bDuck");
    assert_eq!(unsafe { get(input, 66) }, hidden);
    let mut cache = a.new_config_cache();
    assert_eq!(cache.get(Some(&wide("User")), &wide("Engine.Input"), &wide("A")).unwrap(), Some(wide("Axis aTurn Speed=2|Button bDuck")));
    assert_eq!(cache.get(Some(&wide("User")), &wide("Engine.Input"), &wide("B")).unwrap(), Some(wide(hidden)));
}

#[test]
fn existing_destination_command_is_unchanged_or_not_a_self_conflict() {
    let owner = render_test_support::owner("rebind-self"); let a = owner.access(); let input = setup(a);
    let old = "Button bJump|Axis aTurn Speed=2"; stage(a, "A", old);
    unsafe { set(a, input, 65, old); }
    assert_eq!(unsafe { rebind(a, input, 65, &[], &["Button bJump"]) }.unwrap(), RebindOutcome::Unchanged);
    assert_eq!(unsafe { get(input, 65) }, old);
    assert_eq!(unsafe { rebind(a, input, 65, &[], &["Button bJump", "Button bDuck"]) }.unwrap(), RebindOutcome::Applied);
    assert_eq!(unsafe { get(input, 65) }, "Button bJump|Axis aTurn Speed=2|Button bDuck");
}

#[test]
fn alias_overlap_reports_hidden_key_without_editing_anything() {
    let owner = render_test_support::owner("rebind-conflict"); let a = owner.access(); let input = setup(a);
    let old = "Button bJump|Axis aTurn Speed=2"; stage(a, "A", old);
    unsafe { set(a, input, 65, old); alias(a, input, 0, "FireAction", "Button bFire"); set(a, input, 200, "FireAction"); }
    let result = unsafe { rebind(a, input, 65, &["Button bJump"], &["Button bFire"]) }.unwrap();
    let RebindOutcome::PotentialConflicts(conflicts) = result else { panic!("expected conflict"); };
    assert_eq!(conflicts.iter().map(|c| c.key).collect::<Vec<_>>(), [200]);
    assert_eq!(unsafe { get(input, 65) }, old); assert_eq!(unsafe { get(input, 200) }, "FireAction");
    assert_eq!(a.config_get_units_from(Some(&wide("User")), &wide("Engine.Input"), &wide("A")).unwrap(), Some(wide(old)));
}

#[test]
fn cycle_or_unknown_command_refuses_rebinding_even_if_overlap_exists() {
    let owner = render_test_support::owner("rebind-uncertain"); let a = owner.access(); let input = setup(a);
    let old = "Button bJump"; stage(a, "A", old);
    unsafe { set(a, input, 65, old); alias(a, input, 0, "Loop", "Button bFire|Loop"); set(a, input, 200, "Loop"); }
    assert!(matches!(unsafe { rebind(a, input, 65, &["Button bJump"], &["Button bFire"]) }.unwrap(), RebindOutcome::Indeterminate { .. }));
    assert_eq!(unsafe { get(input, 65) }, old); assert_eq!(unsafe { get(input, 200) }, "Loop");
}

#[test]
fn stale_cache_and_external_publish_failure_leave_live_input_unchanged() {
    let owner = render_test_support::owner("rebind-stale"); let a = owner.access(); let input = setup(a);
    stage(a, "A", "Button bJump"); unsafe { set(a, input, 65, "Button bDifferent"); }
    assert!(unsafe { rebind(a, input, 65, &["Button bDifferent"], &["Button bFire"]) }.is_err());
    assert_eq!(unsafe { get(input, 65) }, "Button bDifferent");
    unsafe { set(a, input, 65, "Button bJump"); }
    let path = std::path::PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").unwrap())
        .join(format!("render-rebind-stale-{}/System/DefUser.ini", std::process::id()));
    let mut bytes = fs::read(&path).unwrap(); bytes.extend_from_slice(b"\n; external change\n");
    let external = path.with_extension("external"); fs::write(&external, &bytes).unwrap(); fs::rename(&external, &path).unwrap();
    assert!(unsafe { rebind(a, input, 65, &["Button bJump"], &["Button bFire"]) }.is_err());
    assert_eq!(unsafe { get(input, 65) }, "Button bJump");
    assert_eq!(a.config_get_units_from(Some(&wide("User")), &wide("Engine.Input"), &wide("A")).unwrap(), Some(wide("Button bJump")));
    assert_eq!(fs::read(path).unwrap(), bytes);
}
