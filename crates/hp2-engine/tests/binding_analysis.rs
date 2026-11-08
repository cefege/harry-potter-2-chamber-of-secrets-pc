use hp2_engine::binding_analysis::{self as b, Alias, Comparison, Phase, PropertyKind, Target, Uncertainty};
use std::cell::RefCell;

fn target(phase: Phase, kind: PropertyKind, property: &str) -> Target {
    Target { phase, kind, property: property.into() }
}
#[test]
fn first_alias_expands_and_nested_aliases_are_reported_not_recursed() {
    let aliases = [Alias { name: "Go", command: "Button bRun|Next" }, Alias { name: "Next", command: "Axis aForward Speed=1|Go" }];
    let a = b::analyze("go", &aliases);
    assert!(a.targets.contains(&target(Phase::Press, PropertyKind::Button, "brun")));
    assert!(!a.targets.contains(&target(Phase::Hold, PropertyKind::Axis, "aforward")));
    assert!(a.uncertainties.contains(&Uncertainty::NestedAliasSuppressed("Next".into())));
    let cycle = [Alias { name: "Go", command: "GO" }];
    assert!(b::analyze("Go", &cycle).uncertainties.contains(&Uncertainty::NestedAliasSuppressed("GO".into())));
}
#[test]
fn first_duplicate_alias_and_builtins_match_engine_priority() {
    let aliases = [Alias { name: "A", command: "Button bFirst" }, Alias { name: "a", command: "Button bSecond" }, Alias { name: "KEYNAME", command: "Button bWrong" }];
    let a = b::analyze("A|KEYNAME 3", &aliases);
    assert!(a.targets.contains(&target(Phase::Press, PropertyKind::Button, "bfirst")));
    assert!(!a.targets.contains(&target(Phase::Press, PropertyKind::Button, "bsecond")));
    assert!(!a.targets.contains(&target(Phase::Press, PropertyKind::Button, "bwrong")));
    assert!(a.uncertainties.contains(&Uncertainty::OpaqueCommand("KEYNAME".into())));
}
#[test]
fn release_prefix_and_quoted_pipe_follow_the_actual_engine_command_loop() {
    // ExecInputCommands uses the real parser callbacks and strips OnRelease
    // only for release before routing to viewport Exec.
    fn route(action: i32) -> Vec<(char, String)> {
        let source: Vec<u32> = "OnRelease Button bDone|Say \"x|y\"|Button bNow".chars().map(u32::from).chain([0]).collect();
        let mut input: hp2_abi::engine::generated::UInput = unsafe { std::mem::zeroed() };
        input.action = action;
        let seen = RefCell::new(Vec::new());
        let capture = |kind, p: *const u32| {
            let mut n = 0; unsafe { while *p.add(n) != 0 { n += 1; } }
            let s: String = unsafe { std::slice::from_raw_parts(p, n) }.iter().map(|c| char::from_u32(*c).unwrap()).collect();
            seen.borrow_mut().push((kind, s));
            Ok::<(), ()>(())
        };
        unsafe { hp2_engine::input::exec_input_commands_with(
            &mut input, source.as_ptr(),
            |c, line, len| Ok(hp2_core::tokenize::parse_line(c.cast(), line.cast(), len, false)),
            |c, name| Ok(hp2_core::command::parse_command(c.cast(), name.cast()) != 0),
            |_, line| capture('v', line), |_, line| capture('i', line),
        ).unwrap(); }
        seen.into_inner()
    }
    assert_eq!(route(3), [('v', "Button bDone".into()), ('i', "Say \"x|y\"".into()), ('i', "Button bNow".into())]);
    assert_eq!(route(1)[0], ('v', "OnRelease Button bDone".into()));
    let a = b::analyze("OnRelease Button bDone|Say \"x|y\"|Button bNow", &[]);
    assert!(a.targets.contains(&target(Phase::Release, PropertyKind::Button, "bdone")));
    assert!(!a.targets.contains(&target(Phase::Press, PropertyKind::Button, "bdone")));
    assert!(a.uncertainties.contains(&Uncertainty::OpaqueCommand("Say".into())));
    assert!(a.uncertainties.contains(&Uncertainty::OpaqueCommand("OnRelease".into())));
}
#[test]
fn comparisons_expose_partial_results_and_unknowns() {
    let a = b::analyze("Button bJump|Custom", &[]);
    let b1 = b::analyze("PULSE bJump", &[]);
    assert!(matches!(b::compare(&a, &b1), Comparison::PotentialOverlap { uncertain: true, .. }));
    assert_eq!(b::compare(&a, &b::analyze("Axis aLook", &[])), Comparison::Indeterminate);
    assert_eq!(b::compare(&b::analyze("Button bJump", &[]), &b::analyze("Axis aLook", &[])), Comparison::NoOverlap);
}
#[test]
fn resource_and_unicode_limits_fail_closed() {
    assert!(b::analyze(&"x".repeat(256), &[]).uncertainties.contains(&Uncertainty::LineLimit));
    assert!(b::analyze("Button β", &[]).uncertainties.contains(&Uncertainty::NonAsciiName));
    assert!(b::analyze("Button x\0|Button y", &[]).uncertainties.contains(&Uncertainty::SourceLimit));
    let aliases = vec![Alias { name: "x", command: "Button b" }; 41];
    assert!(b::analyze("x", &aliases).uncertainties.contains(&Uncertainty::TooManyAliases));
}
