use hp2_abi::engine::generated::{
    self, FColor, FFontCharacter, FFontPage, FFrame, FMipmap, TArrayRaw, UFont,
    UObject, UPalette, UTexture, UFunction, UProperty, UStruct,
};
use hp2_core::{
    array_storage::{self, Hp2ArraySlots},
    core_access::{CoreAccess, CoreOptions, RuntimeResult},
    lazy_array::{self, LazyArraySlots},
    name_owner::FindName,
};
use hp2_engine::EngineOwner;
use std::{ffi::c_void, mem::size_of, path::PathBuf, ptr};

fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }
unsafe fn construct(access: CoreAccess<'_>, class: &str, outer: *mut UObject) -> *mut UObject {
    unsafe { access.construct_object(access.native_class(class).unwrap(), outer, 0, 0, ptr::null_mut()) }.unwrap()
}
unsafe fn set_values<T: Copy>(array: *mut TArrayRaw, values: &[T]) {
    let slots = Hp2ArraySlots { data: unsafe { ptr::addr_of_mut!((*array).data) }, num: unsafe { ptr::addr_of_mut!((*array).num) }, max: unsafe { ptr::addr_of_mut!((*array).max) } };
    if values.is_empty() { return; }
    unsafe { slots.max.write_unaligned(values.len() as i32); }
    unsafe { array_storage::realloc(slots, size_of::<T>() as i32, ptr::null(), array_storage::system_allocator()).unwrap(); }
    unsafe { ptr::copy_nonoverlapping(values.as_ptr(), slots.data.read_unaligned().cast(), values.len()); slots.num.write_unaligned(values.len() as i32); }
}
fn mip_slots(mip: *mut FMipmap) -> LazyArraySlots {
    let lazy = unsafe { ptr::addr_of_mut!((*mip).data_array) };
    LazyArraySlots { array: unsafe { lazy.cast::<u8>().add(generated::TLazyBytes::TARRAY_BYTE_OFFSET).cast() }, loader: unsafe { lazy.cast::<u8>().add(generated::TLazyBytes::FLAZYLOADER_OFFSET).cast() }, saved_archive: unsafe { ptr::addr_of_mut!((*mip).data_array.saved_ar) }, saved_position: unsafe { ptr::addr_of_mut!((*mip).data_array.saved_pos) } }
}
unsafe fn set_mip(access: CoreAccess<'_>, texture: *mut UTexture, side: i32, bytes: &[u8]) {
    let raw = ptr::addr_of_mut!((*texture).mips);
    let slots = Hp2ArraySlots { data: ptr::addr_of_mut!((*raw).data), num: ptr::addr_of_mut!((*raw).num), max: ptr::addr_of_mut!((*raw).max) };
    unsafe { slots.max.write_unaligned(1); array_storage::realloc(slots, size_of::<FMipmap>() as i32, ptr::null(), array_storage::system_allocator()).unwrap(); }
    let mip = unsafe { slots.data.read_unaligned().cast::<FMipmap>() };
    unsafe { mip.write(std::mem::zeroed()); (*mip).base.u_size = side; (*mip).base.v_size = side; (*mip).base.u_bits = (32 - (side - 1).leading_zeros()) as u8; (*mip).base.v_bits = (32 - (side - 1).leading_zeros()) as u8; lazy_array::lazy_array_initialize(access, mip_slots(mip)).unwrap(); }
    let byte_array = unsafe { ptr::addr_of_mut!((*mip).data_array).cast::<u8>().add(generated::TLazyBytes::TARRAY_BYTE_OFFSET).cast::<TArrayRaw>() };
    unsafe { set_values(byte_array, bytes); slots.num.write_unaligned(1); }
}
unsafe fn palette(access: CoreAccess<'_>, outer: *mut UObject) -> *mut UPalette {
    let p = unsafe { construct(access, "Engine.Palette", outer) }.cast::<UPalette>();
    let colors: Vec<FColor> = (0..256).map(|i| FColor { r: i as u8, g: i as u8, b: i as u8, a: 255 }).collect();
    unsafe { set_values(ptr::addr_of_mut!((*p).colors), &colors); }
    p
}
unsafe fn texture(access: CoreAccess<'_>, outer: *mut UObject, pal: *mut UPalette, side: i32, pixels: &[u8]) -> *mut UTexture {
    let t = unsafe { construct(access, "Engine.Texture", outer) }.cast::<UTexture>();
    unsafe { (*t).base.u_size = side; (*t).base.v_size = side; (*t).base.u_clamp = side; (*t).base.v_clamp = side; (*t).base.u_bits = (32 - (side - 1).leading_zeros()) as u8; (*t).base.v_bits = (32 - (side - 1).leading_zeros()) as u8; (*t).base.format = 0; (*t).base.palette = pal.cast(); set_mip(access, t, side, pixels); }
    t
}
unsafe fn pixels(texture: *mut UTexture) -> Vec<u8> {
    let mips = unsafe { ptr::read_unaligned(ptr::addr_of!((*texture).mips)) };
    let mip = unsafe { &*mips.data.cast::<FMipmap>() };
    let size = unsafe { ptr::addr_of!((*texture).base.u_size).read_unaligned() as usize * ptr::addr_of!((*texture).base.v_size).read_unaligned() as usize };
    let data = unsafe { ptr::addr_of!((*mip).data_array.data).read_unaligned().cast::<u8>() };
    unsafe { std::slice::from_raw_parts(data, size).to_vec() }
}
fn float_arg(value: f32) -> Vec<u8> { let mut out=vec![0x1e]; out.extend(value.to_le_bytes()); out }
unsafe fn object_arg(object: *mut UObject) -> Vec<u8> { let mut out=vec![0x20]; out.extend(unsafe{(*object).index}.to_le_bytes()); out }
fn string_arg(value: &str) -> Vec<u8> { let mut out=vec![0x34]; for unit in value.encode_utf16().chain([0]) { out.extend(unit.to_le_bytes()); } out }
unsafe fn call(access: CoreAccess<'_>, object: *mut UObject, index: u32, args: Vec<u8>, locals: *mut c_void) -> RuntimeResult<()> {
    unsafe {
        let function = construct(access, "Core.Function", (*object).class.cast()).cast::<UFunction>();
        let mut code = args; code.push(0x16); set_values(ptr::addr_of_mut!((*function.cast::<UStruct>()).script).cast(), &code);
        let start=(*function.cast::<UStruct>()).script.data.cast::<u8>(); let mut frame=FFrame{node:function.cast(),object:object.cast(),code:start,locals:locals.cast(),parent:ptr::null_mut()};
        access.execute_vm_native_index(object,index,&raw mut frame,ptr::null_mut())?;
        assert_eq!(frame.code,start.add(code.len()),"native {index} consumes the complete source argument list");
        Ok(())
    }
}
unsafe fn property(access: CoreAccess<'_>, parent: *mut UStruct, name: &str, offset: i32) -> i32 {
    let class = access.native_class("Core.FloatProperty").unwrap();
    let value = unsafe { access.construct_object(class, parent.cast(), access.intern_str(name, FindName::Add).unwrap(), 0, ptr::null_mut()) }.unwrap().cast::<UProperty>();
    unsafe { (*value).offset=offset;(*value).element_size=4;(*value).array_dim=1;(*value.cast::<hp2_abi::engine::generated::UField>()).next=(*parent).children;(*parent).children=value.cast();(*value.cast::<UObject>()).index }
}
#[test]
fn scripted_texture_vm_natives_rasterize_measure_replace_and_release_real_core_resources() {
    let root=PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("test artifact directory")).join(format!("scripted-texture-vm-{}",std::process::id()));
    std::fs::create_dir_all(root.join("System")).unwrap();std::fs::write(root.join("System/Default.ini"),"").unwrap();std::fs::write(root.join("System/DefUser.ini"),"").unwrap();
    let owner=EngineOwner::new(CoreOptions{data_root:root.clone(),system_ini:root.join("System/Default.ini"),user_ini:root.join("System/DefUser.ini"),command_line:Vec::new(),output,log_file:Some(std::fs::File::create(root.join("runtime.log")).unwrap())}).unwrap();
    let access=owner.access();
    unsafe {
        let scripted=access.construct_object(access.native_class("Engine.ScriptedTexture").unwrap(),access.transient_package(),access.intern_str("Scripted",FindName::Add).unwrap(),0,ptr::null_mut()).unwrap();
        (*scripted.cast::<UTexture>()).base.u_size=3;(*scripted.cast::<UTexture>()).base.v_size=3;(*scripted.cast::<UTexture>()).base.u_bits=2;(*scripted.cast::<UTexture>()).base.v_bits=2;
        let own_palette=palette(access,access.transient_package());let source_palette=palette(access,access.transient_package());(*scripted.cast::<UTexture>()).base.palette=own_palette.cast();
        set_mip(access,scripted.cast(),3,&[0,0,0,0,0,0,0,0,0]);
        let glyph_tex=texture(access,access.transient_package(),source_palette,2,&[0,1,0,0]);
        let tile_tex=texture(access,access.transient_package(),source_palette,2,&[9,8,7,6]);
        let replacement=texture(access,access.transient_package(),source_palette,3,&[4,3,2,1,12,11,10,9,8]);
        let font=construct(access,"Engine.Font",access.transient_package()).cast::<UFont>();(*font).characters_per_page=256;
        let mut glyphs=vec![FFontCharacter{start_u:0,start_v:0,u_size:0,v_size:0};256];glyphs[65]=FFontCharacter{start_u:1,start_v:0,u_size:1,v_size:1};
        let mut page=FFontPage{texture:glyph_tex.cast(),characters:TArrayRaw{data:ptr::null_mut(),num:0,max:0}};set_values(ptr::addr_of_mut!(page.characters),&glyphs);set_values(ptr::addr_of_mut!((*font).pages),&[page]);

        let mut draw=Vec::new();draw.extend(float_arg(0.));draw.extend(float_arg(0.));draw.extend(string_arg("A"));draw.extend(object_arg(font.cast()));call(access,scripted,472,draw,ptr::null_mut()).unwrap();
        assert_eq!(pixels(scripted.cast()),vec![1,0,0,0,0,0,0,0,0],"DrawText rasterizes the glyph bitmap into the canonical destination mip");

        let mut colored=Vec::new();colored.extend(float_arg(0.));colored.extend(float_arg(0.));colored.extend(string_arg("A"));colored.extend(object_arg(font.cast()));colored.extend([0x1d,10,10,10,255]);call(access,scripted,474,colored,ptr::null_mut()).unwrap();
        assert_eq!(pixels(scripted.cast())[0],10,"DrawColoredText maps its FColor through the destination palette");

        let mut out=[0.0f32;2];let mut measure=string_arg("A");
        let text_function=construct(access,"Core.Function",(*scripted).class.cast()).cast::<UFunction>();let x=property(access,text_function.cast(),"XL",0);let y=property(access,text_function.cast(),"YL",4);
        measure.push(0);measure.extend(x.to_le_bytes());measure.push(0);measure.extend(y.to_le_bytes());measure.extend(object_arg(font.cast()));
        call(access,scripted,476,measure,out.as_mut_ptr().cast()).unwrap();
        assert_eq!(out,[1.0,1.0],"TextSize writes glyph width and maximum height through both FLOAT out references");

        let mut tile=Vec::new();for value in [0.,0.,3.,3.,0.,0.,3.,3.] {tile.extend(float_arg(value));}tile.extend(object_arg(tile_tex.cast()));tile.push(0x28);call(access,scripted,473,tile,ptr::null_mut()).unwrap();
        assert_eq!(pixels(scripted.cast()),vec![9,8,0,7,6,0,0,0,0],"DrawTile writes the clipped indexed source pixels using canonical palette mapping");
        let replacement_args=object_arg(replacement.cast());call(access,scripted,475,replacement_args,ptr::null_mut()).unwrap();
        assert_eq!(pixels(scripted.cast()),vec![4,3,2,1,12,11,10,9,8],"ReplaceTexture overwrites the complete matching destination mip");

        let ticking=construct(access,"Engine.ScriptedTexture",access.transient_package()).cast::<generated::UScriptedTexture>();
        (*ticking).base.base.u_size=2;(*ticking).base.base.v_size=2;
        (*ticking).base.base.u_bits=1;(*ticking).base.base.v_bits=1;
        (*ticking).base.base.u_clamp=2;(*ticking).base.base.v_clamp=2;
        (*ticking).base.base.palette=own_palette.cast();
        set_mip(access,ticking.cast(),2,&[1,1,1,1]);
        (*ticking).source_texture=tile_tex.cast();
        (*ticking.cast::<UObject>()).object_flags|=hp2_core::object_lifecycle::RF_NEED_POST_LOAD;
        access.conditional_post_load(ticking.cast()).unwrap();
        (*scripted.cast::<generated::UScriptedTexture>()).local_time=1.0;
        let mut tile=Vec::new();for value in [0.,0.,3.,3.,0.,0.,3.,3.] {tile.extend(float_arg(value));}
        tile.extend(object_arg(ticking.cast()));tile.push(0x28);
        call(access,scripted,473,tile,ptr::null_mut()).unwrap();
        assert_eq!(pixels(scripted.cast()),vec![9,8,2,7,6,11,10,9,8],"Get/Lock dispatches ScriptedTexture's real virtual Tick and refreshes its source bitmap before sampling");
        assert!(!access.object_at((*replacement.cast::<UObject>()).index).is_null(),"native calls borrow their canonical texture arguments without consuming their Core lifetimes");
    }
    owner.shutdown().unwrap();std::fs::write(root.join("report.txt"),"ScriptedTexture natives rasterize, measure, replace, and retain canonical resources\n").unwrap();
}
