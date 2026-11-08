use hp2_abi::engine::generated::{AActor, FFrame, TArrayRaw, UFunction, ULevel, UObject, UProperty, UStruct};
use hp2_core::{
    array_storage::{self, Hp2ArraySlots},
    core_access::{CoreAccess, CoreOptions, RuntimeError, RuntimeResult},
    name_owner::FindName,
};
use hp2_engine::{ActorAllActorsEvent, EngineOwner};
use std::{cell::{Cell, RefCell}, ffi::c_void, path::PathBuf, ptr};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Seen { observer: u8, yielded: Option<usize>, slot: i32, exhausted: bool }
thread_local! {
    static SEEN: RefCell<Vec<Seen>> = const { RefCell::new(Vec::new()) };
    static OWNER: Cell<*const EngineOwner> = const { Cell::new(ptr::null()) };
}
fn observe(event: ActorAllActorsEvent, observer: u8) {
    SEEN.with(|seen| seen.borrow_mut().push(Seen {
        observer, yielded: event.yielded.map(|actor| actor.as_ptr() as usize),
        slot: event.slot, exhausted: event.exhausted,
    }));
}
fn rest(_: CoreAccess<'_>, event: ActorAllActorsEvent) -> RuntimeResult<()> { observe(event, 2); Ok(()) }
fn first(access: CoreAccess<'_>, event: ActorAllActorsEvent) -> RuntimeResult<()> {
    observe(event, 1);
    OWNER.with(|owner| unsafe { &*owner.get() }).publish_actor_all_actors_observer(rest)?;
    let (nested, result) = unsafe { scan(access, event.requestor.as_ptr(), access.native_class("Engine.LevelInfo")?.cast(), 0, false) };
    result?;
    assert!(nested.out.is_null());
    Ok(())
}
fn reject_second(_: CoreAccess<'_>, event: ActorAllActorsEvent) -> RuntimeResult<()> {
    observe(event, 3);
    if event.slot == 5 { return Err(RuntimeError::new("stop at second selected navigation actor")); }
    Ok(())
}
fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }
unsafe fn construct(access: CoreAccess<'_>, class: &str, outer: *mut UObject, name: &str) -> *mut UObject {
    unsafe { access.construct_object(access.native_class(class).unwrap(), outer,
        access.intern_str(name, FindName::Add).unwrap(), 0, ptr::null_mut()) }.unwrap()
}
unsafe fn set_array<T: Copy>(raw: *mut TArrayRaw, values: &[T]) {
    unsafe {
        array_storage::set(Hp2ArraySlots { data: &raw mut (*raw).data, num: &raw mut (*raw).num, max: &raw mut (*raw).max },
            values.len() as i32, size_of::<T>() as i32, ptr::null(), array_storage::system_allocator()).unwrap();
        ptr::copy_nonoverlapping(values.as_ptr(), (*raw).data.cast(), values.len());
    }
}
#[repr(C)]
struct Locals { out: *mut c_void, last: *mut c_void }
unsafe fn scan(access: CoreAccess<'_>, receiver: *mut AActor, class: *mut UObject, tag: i32, break_first: bool) -> (Locals, RuntimeResult<()>) {
    unsafe {
        let function = access.construct_object(
            access.native_class("Core.Function").unwrap(), (*receiver.cast::<UObject>()).class.cast(),
            0, 0, ptr::null_mut(),
        ).unwrap().cast::<UFunction>();
        let mut properties = [0_i32; 2];
        for (i, name) in ["Out", "Last"].into_iter().enumerate() {
            let property = construct(access, "Core.ObjectProperty", function.cast(), name).cast::<UProperty>();
            (*property).offset = (i * size_of::<*mut c_void>()) as i32;
            (*property).element_size = size_of::<*mut c_void>() as i32;
            (*property).array_dim = 1;
            properties[i] = (*property.cast::<UObject>()).index;
        }
        let local = |code: &mut Vec<u8>, property: i32| { code.push(0); code.extend(property.to_le_bytes()); };
        let mut code = vec![0x2f, 0x61, 0x30]; // Source Iterator marker, native304.
        if class.is_null() { code.push(0x2a); } else { code.push(0x20); code.extend((*class).index.to_le_bytes()); }
        local(&mut code, properties[0]);
        code.push(0x21); code.extend(tag.to_le_bytes()); code.push(0x16);
        let word = code.len(); code.extend([0, 0]);
        code.push(0x0f); local(&mut code, properties[1]); local(&mut code, properties[0]);
        let break_word = if break_first { code.push(0x06); let word = code.len(); code.extend([0, 0]); Some(word) } else { None };
        code.push(0x31); let pop = code.len(); code.push(0x30);
        code[word..word + 2].copy_from_slice(&(pop as u16).to_le_bytes());
        if let Some(word) = break_word { code[word..word + 2].copy_from_slice(&(pop as u16).to_le_bytes()); }
        code.push(0x0b);
        set_array(ptr::addr_of_mut!((*function.cast::<UStruct>()).script).cast(), &code);
        let script = (*function.cast::<UStruct>()).script.data.cast::<u8>();
        let mut locals = Locals { out: ptr::null_mut(), last: ptr::null_mut() };
        let mut frame = FFrame { node: function.cast(), object: receiver.cast(), code: script, locals: (&raw mut locals).cast(), parent: ptr::null_mut() };
        let result = access.execute_state_frame_step(&raw mut frame, ptr::null_mut())
            .and_then(|()| access.execute_state_frame_step(&raw mut frame, ptr::null_mut()));
        if result.is_ok() { assert_eq!(frame.code, script.add(pop + 1)); }
        (locals, result)
    }
}

/// Actual canonical Core/Engine consumer, also exercised outside the test harness.
pub fn exercise_all_actors() {
    let artifact = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifact directory"));
    let root = artifact.join(format!("all-actors-owner-{}", std::process::id()));
    std::fs::create_dir_all(root.join("System")).unwrap();
    std::fs::write(root.join("System/Default.ini"), "").unwrap();
    std::fs::write(root.join("System/DefUser.ini"), "").unwrap();
    let owner = EngineOwner::new(CoreOptions { data_root: root.clone(), system_ini: root.join("System/Default.ini"), user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(), output, log_file: Some(std::fs::File::create(root.join("runtime.log")).unwrap()) }).unwrap();
    OWNER.with(|slot| slot.set(&raw const owner));
    let access = owner.access();
    unsafe {
        let actor_class = access.native_class("Engine.Actor").unwrap();
        let tag_name = access.intern_str("Tag", FindName::Add).unwrap();
        if access.find_field(actor_class.cast(), tag_name, access.native_class("Core.Property").unwrap()).is_null() {
            access.add_cpp_property(actor_class.cast(), "NameProperty", tag_name, 0,
                std::mem::offset_of!(AActor, tag), 1, 0, 0, ptr::null_mut()).unwrap();
            access.link_cpp_struct(actor_class.cast()).unwrap();
        }
        let level = construct(access, "Engine.Level", access.transient_package(), "IteratorWorld").cast::<ULevel>();
        let info = construct(access, "Engine.LevelInfo", level.cast(), "WorldInfo").cast::<AActor>();
        let receiver = construct(access, "Engine.Actor", level.cast(), "IteratorRequestor").cast::<AActor>();
        let nav_a = construct(access, "Engine.NavigationPoint", level.cast(), "NavA").cast::<AActor>();
        let nav_b = construct(access, "Engine.NavigationPoint", level.cast(), "NavB").cast::<AActor>();
        let unrelated = construct(access, "Engine.Actor", level.cast(), "Unrelated").cast::<AActor>();
        let actors = [info, ptr::null_mut(), nav_a, receiver, unrelated, nav_b];
        set_array(ptr::addr_of_mut!((*level).base.actors).cast(), &actors);
        for actor in actors.into_iter().filter(|actor| !actor.is_null()) { (*actor).x_level = level.cast(); (*actor).level = info.cast(); }
        let match_tag = access.intern_str("NavTag", FindName::Add).unwrap();
        ptr::addr_of_mut!((*nav_a).tag).cast::<i32>().write_unaligned(match_tag);
        ptr::addr_of_mut!((*nav_b).tag).cast::<i32>().write_unaligned(match_tag);
        let nav_class = access.native_class("Engine.NavigationPoint").unwrap().cast();
        owner.publish_actor_all_actors_observer(first).unwrap();
        let (locals, result) = scan(access, receiver, nav_class, 0, false);
        result.unwrap();
        assert!(locals.out.is_null()); assert_eq!(locals.last, nav_b.cast());
        assert_eq!(SEEN.with(|seen| seen.take()), vec![
            Seen { observer: 1, yielded: Some(nav_a as usize), slot: 2, exhausted: false },
            Seen { observer: 2, yielded: Some(info as usize), slot: 0, exhausted: false },
            Seen { observer: 2, yielded: None, slot: -1, exhausted: true },
            Seen { observer: 2, yielded: Some(nav_b as usize), slot: 5, exhausted: false },
            Seen { observer: 2, yielded: None, slot: -1, exhausted: true },
        ], "replacement and nested cursors must not snapshot the observer or disturb the outer scan");
        let (locals, result) = scan(access, receiver, nav_class, 0, true);
        result.unwrap(); assert_eq!(locals.out, nav_a.cast()); assert_eq!(locals.last, nav_a.cast());
        assert_eq!(SEEN.with(|seen| seen.take()), vec![Seen { observer: 2, yielded: Some(nav_a as usize), slot: 2, exhausted: false }], "break does not synthesize exhaustion");
        owner.publish_actor_all_actors_observer(reject_second).unwrap();
        let (locals, result) = scan(access, receiver, nav_class, 0, false);
        assert!(result.unwrap_err().to_string().contains("stop at second selected navigation actor"));
        assert_eq!(locals.out, nav_b.cast()); assert_eq!(locals.last, nav_a.cast(), "checked observer error stops before the second body");
        assert_eq!(SEEN.with(|seen| seen.take()), vec![
            Seen { observer: 3, yielded: Some(nav_a as usize), slot: 2, exhausted: false },
            Seen { observer: 3, yielded: Some(nav_b as usize), slot: 5, exhausted: false },
        ], "no terminal notification follows the checked error");
        owner.clear_actor_all_actors_observer().unwrap();
        ptr::addr_of_mut!((*nav_a).tag).cast::<i32>().write_unaligned(0);
        let (locals, result) = scan(access, receiver, nav_class, match_tag, false);
        result.unwrap(); assert!(locals.out.is_null()); assert_eq!(locals.last, nav_b.cast());
        let (locals, result) = scan(access, receiver, ptr::null_mut(), 0, true);
        result.unwrap(); assert_eq!(locals.out, info.cast(), "null class resolves to Actor without changing scan order");
    }
    OWNER.with(|slot| slot.set(ptr::null()));
    owner.shutdown().unwrap();
    // Use the repository's Python toolchain as an independent JSON consumer; no runtime
    // JSON dependency or serializer-specific key ordering is introduced into Engine.
    let checked = std::process::Command::new("python3").arg("-c").arg(r#"
import json, sys
with open(sys.argv[1], encoding="utf-8") as stream:
    rows = [json.loads(line) for line in stream]
expected = [("NavA", 2, 0), ("NavB", 5, 1), ("NavA", 2, 0), ("NavA", 2, 0), ("NavB", 5, 1), ("NavB", 5, 0)]
assert len(rows) == len(expected), rows
for row, (name, slot, order) in zip(rows, expected):
    assert row["version"] == 1 and row["kind"] == "all_actors", row
    assert row["requested_class"] == "Engine.NavigationPoint", row
    assert (row["yielded"]["name"], row["yielded"]["slot"], row["yielded"]["order"]) == (name, slot, order), row
    assert isinstance(row["yielded"]["tag"], str), row  # Exported property, not JSON null.
    assert row["firstPatrolPointObjectName"] is None, row
assert rows[0]["match_tag"] is None and rows[5]["match_tag"] == "NavTag"
"#).arg(std::env::var_os("HP2_PATROL_POINT_TRACE").unwrap()).output().unwrap();
    std::fs::write(root.join("patrol-json-check.log"), &checked.stderr).unwrap();
    assert!(checked.status.success(), "canonical patrol consumer failed: {}", String::from_utf8_lossy(&checked.stderr));
    std::fs::write(root.join("report.txt"), "data_profile=data-none\nactual_all_actors_bytecode=ok\nobserver_replacement_and_nested_reentry=ok\nchecked_error_stops_body_and_terminal=ok\nbreak_preserves_out_and_has_no_terminal=ok\nnull_class_and_tag_filter=ok\nreal_patrol_json_and_reflected_name_export=ok\nshutdown=ok\n").unwrap();
}

#[test]
fn canonical_all_actors_observers_and_patrol_preserve_live_iteration() {
    const CHILD: &str = "HP2_ALL_ACTORS_OWNER_CHILD";
    if std::env::var_os(CHILD).is_some() { exercise_all_actors(); return; }
    let artifact = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifact directory"));
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "canonical_all_actors_observers_and_patrol_preserve_live_iteration", "--nocapture"])
        .env(CHILD, "1").env("HP2_PATROL_POINT_TRACE", artifact.join("canonical-all-actors-patrol.jsonl"))
        .env("HP2_PATROL_POINT_TRACE_ACTOR", "IteratorRequestor").status().unwrap();
    assert!(status.success(), "isolated canonical consumer failed: {status}");
}
