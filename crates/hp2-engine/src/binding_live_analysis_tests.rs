use super::*;
use crate::render_owner::render_test_support;
use hp2_core::name_owner::FindName;

unsafe fn set_string(a: CoreAccess<'_>, raw: *mut TArrayRaw, text: &str) {
    let wide: Vec<u32> = text.chars().map(u32::from).chain([0]).collect();
    unsafe { a.assign_fstring(raw, &wide).unwrap(); }
}
unsafe fn clean(a: CoreAccess<'_>, input: *mut UInput) {
    for i in 0..255 { unsafe { set_string(a, ptr::addr_of_mut!((*input).bindings[i]), ""); } }
    for i in 0..40 {
        unsafe {
            ptr::addr_of_mut!((*input).aliases[i].alias).write_unaligned(0i32.to_ne_bytes());
            set_string(a, ptr::addr_of_mut!((*input).aliases[i].command), "");
        }
    }
}
unsafe fn alias(a: CoreAccess<'_>, input: *mut UInput, index: usize, name: &str, command: &str) {
    let id = a.intern_str(name, FindName::Add).unwrap();
    unsafe {
        ptr::addr_of_mut!((*input).aliases[index].alias).write_unaligned(id.to_ne_bytes());
        set_string(a, ptr::addr_of_mut!((*input).aliases[index].command), command);
    }
}
unsafe fn key(a: CoreAccess<'_>, input: *mut UInput, index: usize, command: &str) {
    unsafe { set_string(a, ptr::addr_of_mut!((*input).bindings[index]), command); }
}
fn make(a: CoreAccess<'_>) -> *mut UInput {
    let input = render_test_support::construct(a, "Engine.Input").cast();
    unsafe { clean(a, input); }
    input
}

#[test]
fn ordered_real_aliases_and_hidden_keys_are_included() {
    let owner = render_test_support::owner("live-analysis-order");
    let a = owner.access(); let input = make(a);
    unsafe {
        alias(a, input, 0, "Go", "Button bFirst");
        alias(a, input, 1, "Go", "Button bSecond");
        key(a, input, 65, "Go");
        key(a, input, 200, "Button bFirst|Axis aForward Speed=1");
    }
    let LiveResult::PotentialOverlap(keys) = (unsafe { analyze_live(a, input, "Button bFirst") }) else { panic!("expected potential overlap"); };
    assert_eq!(keys.iter().map(|v| v.key).collect::<Vec<_>>(), [65, 200]);
    assert_eq!(unsafe { analyze_live(a, input, "Button bSecond") }, LiveResult::NoOverlap);
}

#[test]
fn unknown_live_binding_overrides_previously_found_overlap() {
    let owner = render_test_support::owner("live-analysis-unknown");
    let a = owner.access(); let input = make(a);
    unsafe { key(a, input, 10, "Button bJump"); key(a, input, 200, "ScriptCommand Foo"); }
    assert!(matches!(unsafe { analyze_live(a, input, "Button bJump") }, LiveResult::Indeterminate { location: Location::Key(200), reason: Reason::Analysis(_) }));
}

#[test]
fn real_alias_cycle_and_release_command_are_indeterminate() {
    let owner = render_test_support::owner("live-analysis-cycle");
    let a = owner.access(); let input = make(a);
    unsafe { alias(a, input, 2, "Cycle", "Button bJump|Cycle"); key(a, input, 65, "Cycle"); }
    assert!(matches!(unsafe { analyze_live(a, input, "Button bJump") }, LiveResult::Indeterminate { location: Location::Key(65), reason: Reason::Analysis(_) }));
    unsafe { key(a, input, 65, "OnRelease Button bJump"); }
    assert!(matches!(unsafe { analyze_live(a, input, "Button bJump") }, LiveResult::Indeterminate { location: Location::Key(65), reason: Reason::Analysis(_) }));
}

#[test]
fn snapshot_limits_and_candidate_uncertainty_are_indeterminate() {
    let owner = render_test_support::owner("live-analysis-limits");
    let a = owner.access(); let input = make(a);
    assert!(matches!(unsafe { analyze_live(a, input, "UnknownCommand") }, LiveResult::Indeterminate { location: Location::Candidate, .. }));
    unsafe { key(a, input, 254, &"X".repeat(MAX_STRING_UNITS + 1)); }
    assert_eq!(unsafe { analyze_live(a, input, "Button bJump") }, LiveResult::Indeterminate { location: Location::Key(254), reason: Reason::SnapshotLimit });
}
