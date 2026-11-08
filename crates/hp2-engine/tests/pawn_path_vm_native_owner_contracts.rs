use hp2_abi::{FVector, engine::generated::{AActor, ALevelInfo, ANavigationPoint, APawn, FFrame, FReachSpec, TArrayRaw, UEngine, UFunction, ULevel, UObject, UProperty, UStruct}};
use hp2_core::{array_storage::{self, Hp2ArraySlots}, core_access::{CoreAccess, CoreOptions, RuntimeResult}, name_owner::FindName};
use hp2_engine::EngineOwner;
use std::{path::PathBuf, ptr};
fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }
unsafe fn construct(access: CoreAccess<'_>, class: &str, outer: *mut UObject) -> *mut UObject {
    unsafe { access.construct_object(access.native_class(class).unwrap(), outer, 0, 0, ptr::null_mut()) }.unwrap()
}
unsafe fn set_array<T: Copy>(raw: *mut TArrayRaw, values: &[T]) {
    unsafe {
        array_storage::set(Hp2ArraySlots { data: &raw mut (*raw).data, num: &raw mut (*raw).num, max: &raw mut (*raw).max }, values.len() as i32, size_of::<T>() as i32, ptr::null(), array_storage::system_allocator()).unwrap();
        ptr::copy_nonoverlapping(values.as_ptr(), (*raw).data.cast(), values.len());
    }
}
unsafe fn local(access: CoreAccess<'_>, function: *mut UFunction, class: &str, name: &str, offset: i32, size: i32) -> Vec<u8> {
    unsafe {
        let property = access.construct_object(access.native_class(class).unwrap(), function.cast(), access.intern_str(name, FindName::Add).unwrap(), 0, ptr::null_mut()).unwrap().cast::<UProperty>();
        (*property).offset = offset; (*property).element_size = size; (*property).array_dim = 1;
        let mut code = vec![0]; code.extend((*property.cast::<UObject>()).index.to_le_bytes()); code
    }
}
unsafe fn object_arg(object: *mut UObject) -> Vec<u8> {
    if object.is_null() { vec![0x2a] } else { let mut code = vec![0x20]; code.extend(unsafe { (*object).index }.to_le_bytes()); code }
}
unsafe fn call(access: CoreAccess<'_>, object: *mut UObject, native: u32, mut locals: [u128; 2], args: impl FnOnce(*mut UFunction) -> Vec<u8>) -> (usize, [u128; 2]) {
    unsafe {
        let function = construct(access, "Core.Function", (*object).class.cast()).cast::<UFunction>();
        let mut code = args(function); code.push(0x16);
        set_array(ptr::addr_of_mut!((*function.cast::<UStruct>()).script).cast(), &code);
        let script = (*function.cast::<UStruct>()).script.data.cast::<u8>();
        let mut frame = FFrame { node: function.cast(), object: object.cast(), code: script, locals: locals.as_mut_ptr().cast(), parent: ptr::null_mut() };
        let mut result = usize::MAX;
        access.execute_vm_native_index(object, native, &raw mut frame, (&raw mut result).cast()).unwrap();
        assert_eq!(frame.code, script.add(code.len()), "native {native} consumes its arguments and terminator");
        (result, locals)
    }
}
#[test]
fn canonical_navigation_natives_reset_search_route_and_publish_out_parameters() {
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts")).join(format!("pawn-path-vm-{}", std::process::id()));
    std::fs::create_dir_all(root.join("System")).unwrap();
    std::fs::write(root.join("System/Default.ini"), "").unwrap(); std::fs::write(root.join("System/DefUser.ini"), "").unwrap();
    let owner = EngineOwner::new(CoreOptions { data_root: root.clone(), system_ini: root.join("System/Default.ini"), user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(), output, log_file: Some(std::fs::File::create(root.join("runtime.log")).unwrap()) }).unwrap();
    let access = owner.access();
    unsafe {
        let engine = construct(access, "Engine.GameEngine", access.transient_package()).cast::<UEngine>();
        hp2_engine::engine_owner::init(access, ptr::NonNull::new(engine).unwrap()).unwrap();
        let level = construct(access, "Engine.Level", access.transient_package()).cast::<ULevel>(); (*level).base.engine = engine.cast();
        let info = construct(access, "Engine.LevelInfo", level.cast()).cast::<ALevelInfo>();
        let pawn = construct(access, "Engine.Pawn", level.cast()).cast::<APawn>();
        let first = construct(access, "Engine.NavigationPoint", level.cast()).cast::<ANavigationPoint>();
        let second = construct(access, "Engine.NavigationPoint", level.cast()).cast::<ANavigationPoint>();
        let actors = [info.cast::<AActor>(), pawn.cast(), first.cast(), second.cast()];
        set_array(ptr::addr_of_mut!((*level).base.actors).cast(), &actors);
        for actor in actors { (*actor).x_level = level.cast(); (*actor).level = info.cast(); }
        (*info).navigation_point_list = first.cast(); (*first).next_navigation_point = second;
        for node in [first, second] {
            (*node).visited_weight = 13; (*node).next_ordered = second; (*node).prev_ordered = first;
            (*node).set_b_end_point(true); (*node).set_b_special_cost(false); (*node).paths = [-1; 16]; (*node).upstream_paths = [-1; 16];
        }
        (*first).extra_cost = 7; (*second).extra_cost = -3;
        call(access, pawn.cast(), 522, [0; 2], |_| vec![]);
        for (node, cost) in [(first, 7), (second, -3)] {
            assert_eq!((*node).visited_weight, 10_000_000); assert_eq!((*node).cost, cost);
            assert!((*node).next_ordered.is_null()); assert!((*node).prev_ordered.is_null()); assert!(!(*node).b_end_point());
        }
        assert_eq!(call(access, pawn.cast(), 520, [0; 2], |_| vec![0x2a]).0 as u32, 0);
        (*pawn).special_pause = 5.;
        assert_eq!(call(access, pawn.cast(), 517, [0; 2], |_| vec![0x2a, 0x0b, 0x0b]).0, 0);
        assert_eq!((*pawn).special_pause, 5., "null goal exits before path settlement");
        let mut vector = vec![0x23]; for value in [100_f32, 0., 0.] { vector.extend(value.to_le_bytes()); } vector.extend([0x0b, 0x0b]);
        assert_eq!(call(access, pawn.cast(), 518, [0; 2], |_| vector).0, 0, "no reachspecs means no path");
        assert_eq!((*pawn).special_pause, 0., "failed search still settles SpecialPause");
        assert_eq!(call(access, pawn.cast(), 525, [0; 2], |_| vec![0x0b]).0, 0, "empty reachspec graph has no random destination");
        let mut initial = [0_u128; 2]; initial.as_mut_ptr().cast::<f32>().write(7.5);
        let (best, weight) = call(access, pawn.cast(), 540, initial, |function| {
            let mut code = local(access, function, "Core.FloatProperty", "Weight", 0, 4); code.push(0x0b); code
        });
        assert_eq!(best, 0); assert_eq!(weight.as_ptr().cast::<f32>().read(), 7.5, "no improvement preserves caller Weight");
        assert_eq!(call(access, pawn.cast(), 553, [0; 2], |_| vec![0x2a, 0x0b]).0, 0);
        assert_eq!(call(access, pawn.cast(), 532, [0; 2], |_| vec![]).0 as u32, 0, "no viewports cannot see an actor");
        let mut spec: FReachSpec = std::mem::zeroed(); spec.start = first.cast(); spec.end = second.cast(); spec.reach_flags = 9; spec.distance = 123;
        set_array(ptr::addr_of_mut!((*level).reach_specs), &[spec]);
        (*first).paths[0] = 0;
        let destination = i32::from_ne_bytes((*second.cast::<UObject>()).name);
        let found = call(access, pawn.cast(), 553, [0; 2], |_| {
            let mut code = object_arg(first.cast()); code.push(0x21); code.extend(destination.to_le_bytes()); code
        }).0;
        assert_eq!(found, second as usize, "authored route returns next actor even though legacy findPath's bool is false");
        (*pawn).base.collision_radius = 10.; (*pawn).base.collision_height = 20.; (*pawn).move_target = first.cast();
        (*pawn).base.location = FVector { x: 0., y: 0., z: 0. };
        assert_eq!(call(access, pawn.cast(), 520, [0; 2], |_| object_arg(first.cast())).0 as u32, 1, "anchored NavigationPoint reachability uses the real route fast path");
        for index in [-1_i32, 0, 1] {
            let (_, locals) = call(access, first.cast(), 519, [u128::MAX; 2], |function| {
                let mut code = vec![0x1d]; code.extend(index.to_le_bytes());
                for (name, class, offset, size) in [("Start", "Core.ObjectProperty", 0, 8), ("End", "Core.ObjectProperty", 8, 8), ("Flags", "Core.IntProperty", 16, 4), ("Distance", "Core.IntProperty", 20, 4)] {
                    code.extend(local(access, function, class, name, offset, size));
                }
                code
            });
            let bytes = locals.as_ptr().cast::<u8>();
            let (start, end, flags, distance) = if index == 0 { (first as usize, second as usize, 9, 123) } else { (0, 0, 0, 0) };
            assert_eq!(bytes.cast::<usize>().read_unaligned(), start); assert_eq!(bytes.add(8).cast::<usize>().read_unaligned(), end);
            assert_eq!(bytes.add(16).cast::<i32>().read_unaligned(), flags); assert_eq!(bytes.add(20).cast::<i32>().read_unaligned(), distance);
        }
    }
    owner.shutdown().unwrap();
    std::fs::write(root.join("report.txt"), "data_profile=data-none\nall_nine_path_VM_natives=ok\nClearPaths_two_nodes_costs_and_ordering=ok\nnull_goals_and_empty_graph_boundaries=ok\nauthored_next_node_and_anchor_reach=ok\nWeight_reference_preserved=ok\ndescribeSpec_real_out_parameters_and_bounds=ok\nshutdown=ok\n").unwrap();
}
