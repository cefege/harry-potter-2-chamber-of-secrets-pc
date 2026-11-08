use hp2_abi::engine::generated::{AActor, ALevelInfo, APawn, APlayerPawn, FFrame, TArrayRaw, UConsole, UEngine, UField, UFunction, UGameEngine, ULevel, UObject, UProperty, UStruct, UViewport};
use hp2_core::{array_storage::{self, Hp2ArraySlots}, core_access::{CoreAccess, CoreOptions, CoreProcessMode, RuntimeResult}, name_owner::FindName};
use hp2_engine::EngineOwner;
use std::{ffi::c_void, path::PathBuf, ptr};
fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }
unsafe fn construct(access: CoreAccess<'_>, class: &str, outer: *mut UObject) -> *mut UObject {
    unsafe { access.construct_object(access.native_class(class).unwrap(), outer, 0, 0, ptr::null_mut()) }
        .unwrap_or_else(|error| panic!("construct {class}: {error:?}"))
}
unsafe fn set_array<T: Copy>(raw: *mut TArrayRaw, values: &[T]) {
    unsafe {
        array_storage::set(Hp2ArraySlots { data: &raw mut (*raw).data, num: &raw mut (*raw).num, max: &raw mut (*raw).max }, values.len() as i32, size_of::<T>() as i32, ptr::null(), array_storage::system_allocator()).unwrap();
        ptr::copy_nonoverlapping(values.as_ptr(), (*raw).data.cast(), values.len());
    }
}
fn string_arg(value: &str) -> Vec<u8> { let mut code = vec![0x34]; for unit in value.encode_utf16().chain([0]) { code.extend(unit.to_le_bytes()); } code }
unsafe fn call(access: CoreAccess<'_>, object: *mut UObject, class_name: &str, name: &str, index: Option<u32>, args: &[u8], result: *mut c_void) -> RuntimeResult<()> {
    unsafe { call_with_locals(access,object,class_name,name,index,args,result,ptr::null_mut()) }
}
unsafe fn call_with_locals(access: CoreAccess<'_>, object: *mut UObject, class_name: &str, name: &str, index: Option<u32>, args: &[u8], result: *mut c_void, locals: *mut u8) -> RuntimeResult<()> {
    unsafe {
        let class = access.native_class(class_name)?;
        let function = access.construct_object(access.native_class("Core.Function")?, class.cast(), access.intern_str(name, FindName::Add)?, 0, ptr::null_mut())?.cast::<UFunction>();
        (*function).i_native = index.unwrap_or(0) as u16; (*function).function_flags = 0x400;
        (*function).return_value_offset = u16::MAX; (*function.cast::<UStruct>()).properties_align = 1;
        (*function.cast::<UObject>()).object_flags |= hp2_core::object_lifecycle::RF_NEED_POST_LOAD;
        access.conditional_post_load(function.cast())?;
        let caller = construct(access, "Core.Function", class.cast()).cast::<UStruct>();
        let mut code = Vec::new();
        if index.is_none() { code.push(0x1c); code.extend((*function.cast::<UObject>()).index.to_le_bytes()); }
        code.extend(args); code.push(0x16);
        set_array(ptr::addr_of_mut!((*caller).script).cast(), &code);
        let start = (*caller).script.data.cast::<u8>();
        let mut frame = FFrame { node: caller.cast(), object: object.cast(), code: start, locals, parent: ptr::null_mut() };
        let operation = if let Some(index) = index { access.execute_vm_native_index(object.cast(), index, &raw mut frame, result) }
            else { access.execute_state_frame_step(&raw mut frame, result) };
        if operation.is_ok() { assert_eq!(frame.code, start.add(code.len()), "{class_name}.{name} completes its script argument list"); }
        operation
    }
}
fn wide(value: &str) -> Vec<u32> { value.chars().map(u32::from).chain([0]).collect() }
fn byte_arg(value: u8) -> Vec<u8> { vec![0x24, value] }
fn float_arg(value: f32) -> Vec<u8> { let mut code=vec![0x1e];code.extend(value.to_le_bytes());code }
fn vector_arg(value: [f32;3]) -> Vec<u8> { let mut code=vec![0x23];for component in value {code.extend(component.to_le_bytes());}code }
unsafe fn string_result(
    access: CoreAccess<'_>, object: *mut UObject, class: &str, name: &str, args: &[u8],
) -> String {
    unsafe {
        let mut result=TArrayRaw{data:ptr::null_mut(),num:0,max:0};
        let operation=call(access,object,class,name,None,args,(&raw mut result).cast());
        let text=hp2_core::native_text::code_units(hp2_core::native_text::fstring_units(&raw const result))
            .iter().map(|&unit|char::from_u32(unit).unwrap()).collect();
        access.destroy_owned_fstring(&raw mut result).unwrap();
        operation.unwrap();
        text
    }
}
unsafe fn property(
    access: CoreAccess<'_>, parent: *mut UStruct, class: &str, name: &str,
    offset: i32, size: i32, flags: u32,
) -> *mut UProperty {
    unsafe {
        let value=access.construct_object(access.native_class(class).unwrap(),parent.cast(),access.intern_str(name,FindName::Add).unwrap(),0,ptr::null_mut()).unwrap().cast::<UProperty>();
        (*value).offset=offset;(*value).element_size=size;(*value).array_dim=1;(*value).property_flags=flags;
        (*value.cast::<UField>()).next=(*parent).children;(*parent).children=value.cast();
        value
    }
}
unsafe fn script_function(access: CoreAccess<'_>, class: *mut UStruct, name: &str, size: u16, count: u8, flags: u32) -> *mut UFunction {
    unsafe {
        let function=access.construct_object(access.native_class("Core.Function").unwrap(),class.cast(),access.intern_str(name,FindName::Add).unwrap(),0,ptr::null_mut()).unwrap().cast::<UFunction>();
        (*function.cast::<UField>()).next=(*class).children;(*class).children=function.cast();
        (*function.cast::<UStruct>()).properties_size=i32::from(size);(*function.cast::<UStruct>()).properties_align=4;
        (*function).parms_size=size;(*function).num_parms=count;(*function).function_flags=flags;(*function).return_value_offset=u16::MAX;
        // Publish authored fixture metadata exactly where ScriptConsoleExec's
        // UState function lookup reads it; no package linker runs in data-none.
        let state=class.cast::<hp2_abi::engine::generated::UState>();
        let hash=(i32::from_ne_bytes((*function.cast::<UObject>()).name) as usize)&255;
        (*function.cast::<UField>()).hash_next=(*state).vf_hash[hash];
        (*state).vf_hash[hash]=function.cast();
        function
    }
}
#[test]
fn console_platform_and_misc_natives_preserve_real_world_state_and_files() {
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts")).join(format!("console-vm-{}", std::process::id()));
    std::fs::create_dir_all(root.join("System")).unwrap();
    std::fs::write(root.join("System/Default.ini"), "[Engine.Engine]\nCanvas=Engine.Canvas\nInput=Engine.Input\n").unwrap();
    std::fs::write(root.join("System/DefUser.ini"), "").unwrap();
    let owner = EngineOwner::new(CoreOptions { data_root: root.clone(), system_ini: root.join("System/Default.ini"), user_ini: root.join("System/DefUser.ini"), command_line: wide("-STRICT"), output, log_file: Some(std::fs::File::create(root.join("runtime.log")).unwrap()) }).unwrap();
    let access = owner.access(); access.set_process_mode(CoreProcessMode { is_client: true, is_server: true, is_scriptable: true, ..access.process_mode() });
    unsafe {
        let engine = construct(access, "Engine.GameEngine", access.transient_package()).cast::<UGameEngine>();
        hp2_engine::engine_owner::init(access, ptr::NonNull::new(engine.cast::<UEngine>()).unwrap()).unwrap();
        let level = construct(access, "Engine.Level", access.transient_package()).cast::<ULevel>();
        (*level).base.engine = engine.cast(); (*level).model = construct(access, "Engine.Model", level.cast()).cast();
        let info = construct(access, "Engine.LevelInfo", level.cast()).cast::<ALevelInfo>();
        let pawn = construct(access, "Engine.PlayerPawn", level.cast()).cast::<APlayerPawn>();
        set_array(ptr::addr_of_mut!((*level).base.actors).cast(), &[info.cast::<AActor>(), pawn.cast()]);
        for actor in [info.cast::<AActor>(), pawn.cast()] { (*actor).x_level = level.cast(); (*actor).level = info.cast(); }
        (*engine).g_level = level.cast();
        let client = construct(access, "SDLDrv.SDLClient", engine.cast()); (*engine.cast::<UEngine>()).client = client.cast();
        // This data-none world supplies the enum identity required by UInput's
        // constructor, not the package's key-name data or gameplay scripts.
        access.construct_object(access.native_class("Core.Enum").unwrap(),
            access.native_class("Engine.Actor").unwrap().cast(),
            access.intern_str("EInputKey", FindName::Add).unwrap(), 0, ptr::null_mut()).unwrap();
        let viewport = construct(access, "SDLDrv.SDLViewport", client).cast::<UViewport>();
        (*viewport).actor = pawn.cast(); (*pawn).player = viewport.cast();
        let console = construct(access, "Engine.Console", client).cast::<UConsole>(); (*console).viewport = viewport.cast(); (*viewport).console = console.cast();
        let pending = construct(access, "Engine.NetPendingLevel", engine.cast()); (*engine).g_pending_level = pending.cast();
        let pending_index = (*pending).index;
        let mut handled = -17_i32;
        call(access, console.cast(), "Engine.Console", "ConsoleCommand", None, &string_arg("CANCEL"), (&raw mut handled).cast()).unwrap();
        assert_eq!(handled, 1, "CANCEL is handled by UGameEngine, not qualified UEngine::Exec");
        assert!((*engine).g_pending_level.is_null(), "CANCEL clears the live pending travel");
        assert!(access.object_at(pending_index).is_null(), "cancelled pending level is destroyed");

        let input=(*viewport).input.cast::<hp2_abi::engine::generated::UInput>();
        (*input).key_down_table[17]=7;
        for (key,expected) in [(17,1),(18,0),(255,0)] {
            let mut down=-1_i32;
            call(access,pawn.cast(),"Engine.Actor","GetCurrentKeyState",Some(330),&byte_arg(key),(&raw mut down).cast()).unwrap();
            assert_eq!(down,expected,"key state is normalized and IK_MAX is excluded");
        }
        let saved_player=(*pawn).player;
        (*pawn).player=ptr::null_mut();
        let mut down=-1_i32;
        call(access,pawn.cast(),"Engine.Actor","GetCurrentKeyState",Some(330),&byte_arg(17),(&raw mut down).cast()).unwrap();
        assert_eq!(down,0,"an unpossessed pawn does not read another viewport's keys");
        (*pawn).player=saved_player;

        access.assign_fstring(ptr::addr_of_mut!((*level).base.url.protocol),&wide("https")).unwrap();
        access.assign_fstring(ptr::addr_of_mut!((*level).base.url.host),&wide("宿.local")).unwrap();
        access.assign_fstring(ptr::addr_of_mut!((*level).base.url.map),&wide("Entrance.unr")).unwrap();
        (*level).base.url.port=i32::MIN;
        assert_eq!(string_result(access,info.cast(),"Engine.LevelInfo","GetAddressURL",&[]),"宿.local:-2147483648");
        assert_eq!(string_result(access,info.cast(),"Engine.LevelInfo","GetLocalURL",&[]),"https://宿.local:-2147483648/Entrance.unr");

        let fps="fps=60 Ω\n";
        call(access,console.cast(),"Engine.Console","SaveTimeDemo",None,&string_arg(fps),ptr::null_mut()).unwrap();
        let mut expected=vec![0xff,0xfe];
        for unit in fps.encode_utf16() { expected.extend(unit.to_le_bytes()); }
        assert_eq!(std::fs::read(root.join("System/fps.txt")).unwrap(),expected,"non-ANSI time demos use the native UTF-16LE BOM encoding");
        call(access,console.cast(),"Engine.Console","SaveTimeDemo",None,&string_arg(""),ptr::null_mut()).unwrap();
        assert_eq!(std::fs::read(root.join("System/fps.txt")).unwrap(),expected,"an empty time demo must not truncate the existing file");

        let cache=root.join("Cache");
        let destination=root.join("Destination");
        std::fs::create_dir_all(&cache).unwrap();std::fs::create_dir_all(&destination).unwrap();
        let system=access.system().unwrap();
        access.assign_fstring(ptr::addr_of_mut!((*system).cache_path),&wide(cache.to_str().unwrap())).unwrap();
        access.assign_fstring(ptr::addr_of_mut!((*system).cache_ext),&wide(".uxx")).unwrap();
        access.destroy_owned_fstring_array(ptr::addr_of_mut!((*system).paths)).unwrap();
        let mut path=TArrayRaw{data:ptr::null_mut(),num:0,max:0};
        access.assign_fstring(&raw mut path,&wide(destination.join("*.u").to_str().unwrap())).unwrap();
        set_array(ptr::addr_of_mut!((*system).paths),&[path]);
        std::fs::write(cache.join("cache.ini"),"[Cache]\nGUID=downloaded.u\n").unwrap();
        std::fs::write(cache.join("GUID.uxx"),b"actual cached package bytes").unwrap();
        std::fs::write(destination.join("Renamed.u"),b"old destination bytes").unwrap();
        let mut args=string_arg("GUID");args.extend(string_arg("C:\\Downloads\\Renamed.u"));
        let mut moved=-1_i32;
        call(access,pawn.cast(),"Engine.Actor","MoveCacheEntry",None,&args,(&raw mut moved).cast()).unwrap();
        assert_eq!(moved,1);assert_eq!(std::fs::read(destination.join("Renamed.u")).unwrap(),b"actual cached package bytes");
        assert!(!cache.join("GUID.uxx").exists());
        let mut persisted=access.new_config_cache();
        assert_eq!(persisted.get(Some(&wide(cache.join("cache.ini").to_str().unwrap())),&wide("Cache"),&wide("GUID")).unwrap(),Some(Vec::new()),"successful moves flush the cleared key from their independent config session");
        std::fs::write(cache.join("cache.ini"),"[Cache]\nMISSING=Keep.u\n").unwrap();
        let mut args=string_arg("MISSING");args.extend(string_arg(""));
        call(access,pawn.cast(),"Engine.Actor","MoveCacheEntry",None,&args,(&raw mut moved).cast()).unwrap();
        assert_eq!(moved,0,"a missing source is not reported as a successful move");
        let mut persisted=access.new_config_cache();
        assert_eq!(persisted.get(Some(&wide(cache.join("cache.ini").to_str().unwrap())),&wide("Cache"),&wide("MISSING")).unwrap(),Some("Keep.u".chars().map(u32::from).collect()),"failed moves retain the cache entry");

        let pawn_class=(*pawn.cast::<UObject>()).class.cast::<UStruct>();
        let speed=property(access,pawn_class,"Core.FloatProperty","DesiredSpeed",std::mem::offset_of!(APawn,desired_speed) as i32,4,0);
        let exec=script_function(access,pawn_class,"SetB4Speed",4,1,0x200);
        let amount=property(access,exec.cast(),"Core.FloatProperty","Amount",0,4,0x80);
        let mut script=vec![0x0f,0x01];script.extend((*speed.cast::<UObject>()).index.to_le_bytes());
        script.push(0x00);script.extend((*amount.cast::<UObject>()).index.to_le_bytes());script.extend([0x04,0x0b]);
        set_array(ptr::addr_of_mut!((*exec.cast::<UStruct>()).script).cast(),&script);
        string_result(access,pawn.cast(),"Engine.PlayerPawn","ConsoleCommand",&string_arg("SetB4Speed 0.625"));
        assert_eq!((*pawn.cast::<APawn>()).desired_speed,0.0,"Actor ProcessEvent does not execute gameplay script before BeginPlay");
        (*info).set_b_begun_play(true);
        string_result(access,pawn.cast(),"Engine.PlayerPawn","ConsoleCommand",&string_arg("SetB4Speed 0.625"));
        assert_eq!((*pawn.cast::<APawn>()).desired_speed,0.625,"PlayerPawn.ConsoleCommand imports each parameter once and executes the real script exec after BeginPlay");
        string_result(access,pawn.cast(),"Engine.Actor","ConsoleCommand",&string_arg("SetB4Speed 0.875"));
        assert_eq!((*pawn.cast::<APawn>()).desired_speed,0.625,"Actor's qualified ConsoleCommand uses the engine, not PlayerPawn's exec override");

        let player=property(access,pawn_class,"Core.ObjectProperty","Player",std::mem::offset_of!(APlayerPawn,player) as i32,8,0);
        let pre_travel=script_function(access,pawn_class,"PreClientTravel",0,0,0x800);
        let mut script=vec![0x0f,0x01];script.extend((*speed.cast::<UObject>()).index.to_le_bytes());
        script.extend(float_arg(1.25));script.extend([0x04,0x0b]);
        set_array(ptr::addr_of_mut!((*pre_travel.cast::<UStruct>()).script).cast(),&script);
        let mut args=string_arg("Entrance.unr?Restart");args.extend(byte_arg(1));args.push(0x27);
        call(access,pawn.cast(),"Engine.PlayerPawn","ClientTravel",None,&args,ptr::null_mut()).unwrap();
        assert_eq!((*pawn.cast::<APawn>()).desired_speed,1.25,"PreClientTravel runs before storing viewport travel");
        let previous_travel:Vec<u32>="Entrance.unr?Restart".chars().map(u32::from).collect();
        assert_eq!(hp2_core::native_text::code_units(hp2_core::native_text::fstring_units(ptr::addr_of!((*viewport).travel_url))),previous_travel);
        assert_eq!((*viewport).travel_type,1);assert!((*viewport).b_travel_items());
        let mut script=vec![0x0f,0x01];script.extend((*player.cast::<UObject>()).index.to_le_bytes());script.extend([0x2a,0x04,0x0b]);
        set_array(ptr::addr_of_mut!((*pre_travel.cast::<UStruct>()).script).cast(),&script);
        let mut args=string_arg("must-not-be-stored.unr");args.extend([0x0b,0x0b]);
        call(access,pawn.cast(),"Engine.PlayerPawn","ClientTravel",None,&args,ptr::null_mut()).unwrap_err();
        assert!((*pawn).player.is_null(),"the real PreClientTravel script can clear Player");
        assert_eq!(hp2_core::native_text::code_units(hp2_core::native_text::fstring_units(ptr::addr_of!((*viewport).travel_url))),previous_travel,"SetClientTravel must reread Player after script reentry");
        (*pawn).player=saved_player;

        let wind=construct(access,"Engine.Wind",level.cast()).cast::<hp2_abi::engine::generated::AWind>();
        (*wind).base.x_level=level.cast();(*wind).base.level=info.cast();
        (*wind).wind_speed=8.;(*wind).wind_radius=10;(*wind).wind_source=1;(*wind).set_b_permeating(true);
        let mut force:hp2_abi::FVector=std::mem::zeroed();
        call(access,wind.cast(),"Engine.Wind","GetWind",Some(425),&vector_arg([0.,0.,0.]),(&raw mut force).cast()).unwrap();
        assert_eq!(force,hp2_abi::FVector{x:8.,y:0.,z:0.});
        call(access,wind.cast(),"Engine.Wind","GetWind",Some(425),&vector_arg([1000.,0.,0.]),(&raw mut force).cast()).unwrap();
        assert_eq!(force,hp2_abi::FVector{x:0.,y:0.,z:0.},"out-of-range winds contribute no force");

        let vector_type=construct(access,"Core.Struct",pawn_class.cast()).cast::<UStruct>();
        (*vector_type).properties_size=12;(*vector_type).properties_align=4;
        for (name,offset) in [("X",0),("Y",4),("Z",8)] {property(access,vector_type,"Core.FloatProperty",name,offset,4,0);}
        let source=construct(access,"Core.Function",pawn_class.cast()).cast::<UStruct>();
        let element=property(access,source,"Core.StructProperty","Vector",0,12,0).cast::<hp2_abi::engine::generated::UStructProperty>();
        (*element).r#struct=vector_type.cast();
        let points=property(access,source,"Core.ArrayProperty","Points",0,16,0).cast::<hp2_abi::engine::generated::UArrayProperty>();
        (*points).inner=element.cast();
        let gesture=construct(access,"Engine.Gesture",access.transient_package()).cast::<hp2_abi::engine::generated::UGesture>();
        set_array(ptr::addr_of_mut!((*gesture).points),&[hp2_abi::FVector{x:0.,y:0.,z:0.},hp2_abi::FVector{x:1.,y:0.,z:0.}]);
        let drawn:Vec<hp2_abi::FVector>=(0..10).map(|i|hp2_abi::FVector{x:i as f32/9.,y:0.,z:0.}).collect();
        let mut local_points=TArrayRaw{data:ptr::null_mut(),num:0,max:0};set_array(&raw mut local_points,&drawn);
        let mut args=vec![0x00];args.extend((*points.cast::<UObject>()).index.to_le_bytes());args.extend(float_arg(0.1));
        let mut score=-1_f32;
        call_with_locals(access,gesture.cast(),"Engine.Gesture","CompareGesture",Some(426),&args,(&raw mut score).cast(),(&raw mut local_points).cast()).unwrap();
        assert_eq!(score,1.0,"a dense matching drawn gesture receives the full score");
        array_storage::release(Hp2ArraySlots{data:&raw mut local_points.data,num:&raw mut local_points.num,max:&raw mut local_points.max},array_storage::system_allocator()).unwrap();
        let mut args=vector_arg([0.,0.,0.]);args.push(0x0b);
        call(access,gesture.cast(),"Engine.Gesture","CompareGesturePoint",Some(427),&args,(&raw mut score).cast()).unwrap();
        assert_eq!(score,0.0,"a point on the gesture has zero distance");

        (*pawn.cast::<AActor>()).physics=0;
        (*pawn.cast::<AActor>()).acceleration=hp2_abi::FVector{x:1.66,y:-2.44,z:0.14};
        call(access,pawn.cast(),"Engine.Actor","AutonomousPhysics",Some(3971),&float_arg(0.25),ptr::null_mut()).unwrap();
        assert_eq!((*pawn.cast::<AActor>()).acceleration,hp2_abi::FVector{x:1.6,y:-2.4,z:0.1},"replicated tenth-unit truncation occurs even when physics is disabled");
        (*pawn.cast::<AActor>()).physics=5;(*pawn.cast::<AActor>()).rotation_rate.yaw=8192;
        (*pawn.cast::<AActor>()).set_b_fixed_rotation_dir(true);
        (*pawn.cast::<AActor>()).set_b_rotate_to_desired(false);
        // Engine.u's Pawn script defaults are absent in this data-none world.
        (*pawn.cast::<AActor>()).set_b_static(false);(*pawn.cast::<AActor>()).set_b_movable(true);
        call(access,pawn.cast(),"Engine.Actor","AutonomousPhysics",Some(3971),&float_arg(0.25),ptr::null_mut()).unwrap();
        assert_eq!((*pawn.cast::<AActor>()).rotation.yaw,2048,"enabled autonomous physics runs the canonical rotation policy");
        (*pawn.cast::<AActor>()).physics=0;(*pawn.cast::<AActor>()).rotation.yaw=0;
        let mut args=string_arg("UNREALSHARE");args.extend(string_arg("Male1"));
        let mut valid=-1_i32;
        call(access,pawn.cast(),"Engine.Pawn","CheckValidSkinPackage",None,&args,(&raw mut valid).cast()).unwrap();
        assert_eq!(valid,0,"proscribed stock skin packages remain rejected");

        let client=client.cast::<hp2_abi::engine::generated::UClient>();
        set_array(ptr::addr_of_mut!((*client).viewports),&[viewport]);
        (*viewport).size_x=640;(*viewport).size_y=480;(*pawn.cast::<APawn>()).fov_angle=90.;
        let calc=script_function(access,pawn_class,"PlayerCalcView",32,3,0x800);
        property(access,calc.cast(),"Core.ObjectProperty","ViewActor",0,8,0x180);
        let camera=property(access,calc.cast(),"Core.StructProperty","CameraLocation",8,12,0x180).cast::<hp2_abi::engine::generated::UStructProperty>();
        (*camera).r#struct=vector_type.cast();
        let rotation=property(access,calc.cast(),"Core.StructProperty","CameraRotation",20,12,0x180).cast::<hp2_abi::engine::generated::UStructProperty>();
        (*rotation).r#struct=vector_type.cast();
        let mut script=vec![0x0f,0x00];script.extend((*camera.cast::<UObject>()).index.to_le_bytes());script.extend(vector_arg([10.,20.,30.]));script.extend([0x04,0x0b]);
        set_array(ptr::addr_of_mut!((*calc.cast::<UStruct>()).script).cast(),&script);
        let mut projected:hp2_abi::FVector=std::mem::zeroed();
        call(access,pawn.cast(),"Engine.PlayerPawn","ScreenToWorld",Some(542),&vector_arg([320.,240.,64.]),(&raw mut projected).cast()).unwrap();
        assert_eq!(projected,hp2_abi::FVector{x:10.,y:20.,z:94.},"projection uses the real PlayerCalcView out parameters");

        let model=(*level).model.cast::<hp2_abi::engine::generated::UModel>();
        hp2_engine::level_owner::set_actor_collision(access,level,true).unwrap();
        let flag_source=construct(access,"Core.Function",pawn_class.cast()).cast::<UStruct>();
        let flag_property=property(access,flag_source,"Core.IntProperty","Flags",0,4,0);
        let mut trace_flags=-1_i32;let mut texture=ptr::null_mut::<c_void>();
        let trace_args=|start:[f32;3],end:[f32;3],decals:bool|{
            let mut args=vector_arg(end);args.extend(vector_arg(start));args.push(0x00);
            args.extend((*flag_property.cast::<UObject>()).index.to_le_bytes());args.push(if decals{0x27}else{0x28});args
        };
        call_with_locals(access,pawn.cast(),"Engine.Actor","TraceTexture",Some(285),&trace_args([0.,0.,10.],[0.,0.,-10.],false),(&raw mut texture).cast(),(&raw mut trace_flags).cast()).unwrap();
        assert!(texture.is_null());assert_eq!(trace_flags,0,"a miss clears the caller's real INT out reference");
        let mut node:hp2_abi::render_scene::FBspNode=std::mem::zeroed();
        node.plane=hp2_abi::FPlane{x:0.,y:0.,z:1.,w:0.};node.num_vertices=4;
        node.i_back=-1;node.i_front=-1;node.i_plane=-1;node.i_collision_bound=-1;node.i_render_bound=-1;node.i_leaf=[-1,-1];
        (*model).root_outside=1;
        set_array(ptr::addr_of_mut!((*model).nodes).cast(),&[node]);
        let origin=hp2_abi::FVector{x:0.,y:0.,z:0.};
        let quad=[hp2_abi::FVector{x:-100.,y:-100.,z:0.},hp2_abi::FVector{x:100.,y:-100.,z:0.},hp2_abi::FVector{x:100.,y:100.,z:0.},hp2_abi::FVector{x:-100.,y:100.,z:0.}];
        set_array(ptr::addr_of_mut!((*model).points).cast(),&[origin,quad[0],quad[1],quad[2],quad[3]]);
        set_array(ptr::addr_of_mut!((*model).vectors).cast(),&[hp2_abi::FVector{x:0.,y:0.,z:1.}]);
        set_array(ptr::addr_of_mut!((*model).verts).cast(),&[hp2_abi::render_scene::FVert{p_vertex:1,i_side:0},hp2_abi::render_scene::FVert{p_vertex:2,i_side:0},hp2_abi::render_scene::FVert{p_vertex:3,i_side:0},hp2_abi::render_scene::FVert{p_vertex:4,i_side:0}]);
        let base_texture=construct(access,"Engine.Texture",access.transient_package()).cast::<hp2_abi::engine::generated::UTexture>();
        (*base_texture).poly_flags=1;
        let mut surf:hp2_abi::engine::generated::FBspSurf=std::mem::zeroed();surf.texture=base_texture.cast();surf.poly_flags=0x100;
        set_array(ptr::addr_of_mut!((*model).surfs).cast(),&[surf]);
        call_with_locals(access,pawn.cast(),"Engine.Actor","TraceTexture",Some(285),&trace_args([0.,0.,10.],[0.,0.,-10.],false),(&raw mut texture).cast(),(&raw mut trace_flags).cast()).unwrap();
        assert_eq!(texture,base_texture.cast());assert_eq!(trace_flags,0x101,"BSP surface and original texture flags combine");
        let mut decals=Vec::new();
        let mut last_texture=ptr::null_mut::<UObject>();
        for index in 0..3 {
            let actor=construct(access,"Engine.Actor",level.cast()).cast::<AActor>();
            if index<2 {
                last_texture=construct(access,"Engine.Texture",access.transient_package());
                (*last_texture.cast::<hp2_abi::engine::generated::UTexture>()).poly_flags=0x8000;
                (*actor).texture=last_texture.cast();
            }
            let mut decal:hp2_abi::engine::generated::FDecal=std::mem::zeroed();
            decal.vertices=quad;decal.actor=actor.cast();decals.push(decal);
        }
        let live_surf=(*model).surfs.data.cast::<hp2_abi::engine::generated::FBspSurf>();
        set_array(ptr::addr_of_mut!((*live_surf).decals),&decals);
        call_with_locals(access,pawn.cast(),"Engine.Actor","TraceTexture",Some(285),&trace_args([0.,0.,10.],[0.,0.,-10.],true),(&raw mut texture).cast(),(&raw mut trace_flags).cast()).unwrap();
        assert_eq!(texture,last_texture.cast(),"the last containing non-null decal texture wins");
        assert_eq!(trace_flags,0x101,"decal replacement retains the original surface/texture flags");

        // The port preserves Core Unix's intentionally disabled clipboard API,
        // rather than silently changing these natives to SDL clipboard behavior.
        call(access,pawn.cast(),"Engine.PlayerPawn","CopyToClipboard",None,&string_arg("source Unix clipboard smoke"),ptr::null_mut()).unwrap();
        assert_eq!(string_result(access,pawn.cast(),"Engine.PlayerPawn","PasteFromClipboard",&[]),"");

        assert_eq!(string_result(access,pawn.cast(),"Engine.PlayerPawn","GetPlayerNetworkAddress",&[]),"","local viewport players have no remote network address");
        let mut args=string_arg("Helvetica");args.push(0x1d);args.extend(18_i32.to_le_bytes());
        let mut font=ptr::null_mut::<UObject>();
        call(access,console.cast(),"Engine.Console","CreateNativeFont",None,&args,(&raw mut font).cast()).unwrap();
        assert!(hp2_core::object_model::is_a(font.cast(),access.native_class("Engine.Font").unwrap().cast()),"the font result is a Core-owned Font object, not a backend handle");
        let font_index=(*font).index;
        access.delete_object(font).unwrap();
        assert!(access.object_at(font_index).is_null(),"native-font ownership ends through the ordinary Core object lifecycle");

        let actor_index=(*pawn.cast::<UObject>()).index;
        let source=construct(access,"Core.Function",pawn_class.cast()).cast::<UStruct>();
        let message=property(access,source,"Core.StrProperty","Message",0,size_of::<TArrayRaw>() as i32,0);
        let mut long_text=TArrayRaw{data:ptr::null_mut(),num:0,max:0};
        access.assign_fstring(&raw mut long_text,&wide(&"x".repeat(4096))).unwrap();
        // String literals intentionally pass through appStaticString1024; a real
        // FString local is required to exercise FFrame::Log's larger boundary.
        let mut args=vec![0x00];args.extend((*message.cast::<UObject>()).index.to_le_bytes());
        let operation=call_with_locals(access,pawn.cast(),"Engine.Actor","Error",Some(233),&args,ptr::null_mut(),(&raw mut long_text).cast());
        access.destroy_owned_fstring(&raw mut long_text).unwrap();
        let error=operation.unwrap_err();
        assert_eq!(error.diagnostic(),&[],"unformatted FFrame::Log clears the overflowing final diagnostic, not just its input");
        assert_eq!(access.object_at(actor_index),pawn.cast(),"strict script logging fails before DestroyActor");

        // MoveCacheEntry above selected this live System.Paths search root.
        std::fs::write(destination.join("Engine.u"),b"deliberately invalid package fixture").unwrap();
        let loaded=access.static_load_class(access.native_class("Engine.Canvas").unwrap(),ptr::null_mut(),&wide("ini:Engine.Engine.Canvas"),2);
        assert!(loaded.map_or(true,|class|class.is_null()),"a loaded Rust module must not bypass an existing corrupt package");
        std::fs::remove_file(destination.join("Engine.u")).unwrap();
    }
    owner.shutdown().unwrap();
    std::fs::write(root.join("report.txt"), "data_profile=data-none\npending_travel_virtual_cancel=ok\nnative_override_and_single_parameter_import=ok\nPreClientTravel_reentry=ok\nkey_boundaries_and_local_URLs=ok\ncache_move_replace_and_persistence=ok\nUTF16_time_demo_and_empty_preservation=ok\nwind_and_owned_gesture_arrays=ok\nautonomous_physics=ok\nscripted_projection=ok\nBSP_flags_and_last_decal=ok\nfont_lifetime=ok\nstrict_unformatted_error=ok\ncorrupt_package_not_bypassed=ok\nshutdown=ok\n").unwrap();
}
