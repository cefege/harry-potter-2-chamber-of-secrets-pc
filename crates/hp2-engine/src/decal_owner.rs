//! Canonical Core host for the source `UnScript.cpp` decal callbacks.
//!
//! Every geometry read returns copied scalar records. Container operations keep
//! only indices across reallocation, matching the native decal policy contract.

use crate::{
    actor_owner,
    decal::{self, Hp2DecalNode, Hp2DecalServices, Hp2DecalState, Hp2DecalSurface},
    model::{self, ModelArray, ModelArrayKind},
    model_archive::Hp2ModelArchiveField as ModelField,
};
use hp2_abi::render_scene::{FBspNode, FBspSurf, FVert, TArrayRaw as SceneArrayRaw};
use hp2_abi::{
    FVector,
    engine::generated::{
        self, AActor, ADecal, FCheckResult, TArrayRaw, UClient, UEngine, ULevel, UModel, UObject,
        UTexture,
    },
};
use hp2_core::{
    array_storage::{self, Hp2ArrayServices, Hp2ArraySlots},
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    core_class_specs::{NativeClassSpec, NativeRecordKind},
    name_owner::FindName,
    object_owner::ClassOps,
};
use std::{alloc::Layout, ffi::c_void, mem::size_of, ptr};

const OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
const ARRAY_SERVICES: Hp2ArrayServices = array_storage::system_allocator();
const TRACE_VIS_BLOCKING: u32 = 0x06;
const INDEX_NONE: i32 = -1;

/// Native `ADecal` allocation and `SurfList` lifetime.
pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[NativeClassSpec {
    short_name: "Decal",
    package: "Engine",
    super_name: Some("Actor"),
    within: "Object",
    flags: 0,
    object_flags: OBJECT_FLAGS,
    config: "System",
    guid: [0; 4],
    kind: NativeRecordKind::Object,
    layout: Some(Layout::new::<ADecal>()),
    operations: Some(DECAL_OPERATIONS),
    properties: &[],
    defaults: &[],
}];

const DECAL_OPERATIONS: ClassOps = {
    let mut operations = actor_owner::ACTOR_OPERATIONS;
    operations.construct = construct_decal;
    operations.finalize_members = finalize_decal;
    operations
};

unsafe fn construct_decal(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe {
        (actor_owner::ACTOR_OPERATIONS.construct)(access, object)?;
    }
    let decal = object.cast::<ADecal>();
    // TArray's genuine empty state is the zero header installed by its C++ ctor.
    unsafe {
        ptr::addr_of_mut!((*decal).surf_list).write_unaligned(TArrayRaw {
            data: ptr::null_mut(),
            num: 0,
            max: 0,
        });
    }
    Ok(())
}

unsafe fn finalize_decal(_access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let decal = object.cast::<ADecal>();
    // The SurfList is an implicit native member and remains live through AActor::Destroy.
    unsafe { array_storage::release(decal_slots(decal), ARRAY_SERVICES) }
        .map_err(|_| RuntimeError::new("Unable to destroy canonical ADecal SurfList"))
}

struct DecalContext<'a> {
    access: CoreAccess<'a>,
    actor: *mut ADecal,
    error: Option<RuntimeError>,
}

impl DecalContext<'_> {
    fn remember(&mut self, error: RuntimeError) -> i32 {
        if self.error.is_none() {
            self.error = Some(error);
        }
        1
    }

    unsafe fn output<T: Copy>(&mut self, out: *mut T, result: RuntimeResult<T>) -> i32 {
        if out.is_null() {
            return self.remember(RuntimeError::new("Null canonical decal callback output"));
        }
        match result {
            Ok(value) => {
                unsafe {
                    out.write_unaligned(value);
                }
                0
            }
            Err(error) => self.remember(error),
        }
    }

    fn status(&mut self, result: RuntimeResult<()>) -> i32 {
        match result {
            Ok(()) => 0,
            Err(error) => self.remember(error),
        }
    }
}

unsafe fn context<'a>(raw: *mut c_void) -> &'a mut DecalContext<'a> {
    unsafe { &mut *raw.cast::<DecalContext<'_>>() }
}

fn invalid(label: &str) -> RuntimeError {
    RuntimeError::new(&format!("Invalid canonical decal {label}"))
}

fn array_slots(array: *mut TArrayRaw) -> Hp2ArraySlots {
    unsafe {
        Hp2ArraySlots {
            data: ptr::addr_of_mut!((*array).data),
            num: ptr::addr_of_mut!((*array).num),
            max: ptr::addr_of_mut!((*array).max),
        }
    }
}

fn scene_array_slots(array: *mut SceneArrayRaw) -> Hp2ArraySlots {
    unsafe {
        Hp2ArraySlots {
            data: ptr::addr_of_mut!((*array).data),
            num: ptr::addr_of_mut!((*array).array_num),
            max: ptr::addr_of_mut!((*array).array_max),
        }
    }
}

unsafe fn array_count(slots: Hp2ArraySlots, label: &str) -> RuntimeResult<i32> {
    unsafe {
        let count = slots.num.read();
        let capacity = slots.max.read();
        let data = slots.data.read_unaligned();
        if count < 0 || capacity < count || capacity < 0 || (capacity > 0 && data.is_null()) {
            return Err(invalid(label));
        }
        Ok(count)
    }
}

unsafe fn array_item<T: Copy>(slots: Hp2ArraySlots, index: i32, label: &str) -> RuntimeResult<T> {
    unsafe {
        let count = array_count(slots, label)?;
        let data = slots.data.read_unaligned();
        if index < 0 || index >= count || data.is_null() {
            return Err(invalid(label));
        }
        Ok(data.cast::<T>().add(index as usize).read_unaligned())
    }
}

fn model_array(model: *mut UModel, field: ModelField) -> RuntimeResult<ModelArray> {
    if model.is_null() {
        return Err(invalid("model"));
    }
    unsafe { model::model_array(model, field) }.map_err(|_| invalid("model array"))
}

unsafe fn model_item<T: Copy>(
    model: *mut UModel,
    field: ModelField,
    index: i32,
    label: &str,
) -> RuntimeResult<T> {
    let array = model_array(model, field)?;
    unsafe { array_item(array.slots, index, label) }
}

unsafe fn surface_ptr(model: *mut UModel, index: i32) -> RuntimeResult<*mut FBspSurf> {
    let array = model_array(model, ModelField::Surfs)?;
    let count = unsafe { array_count(array.slots, "surface array")? };
    let data = unsafe { array.slots.data.read_unaligned() };
    if index < 0 || index >= count || data.is_null() {
        return Err(invalid("surface index"));
    }
    Ok(unsafe { data.cast::<FBspSurf>().add(index as usize) })
}

unsafe fn surface_nodes(surface: *mut FBspSurf) -> Hp2ArraySlots {
    unsafe { scene_array_slots(ptr::addr_of_mut!((*surface).nodes)) }
}

unsafe fn surface_decals(surface: *mut FBspSurf) -> Hp2ArraySlots {
    unsafe { scene_array_slots(ptr::addr_of_mut!((*surface).decals)) }
}

unsafe fn decal_slots(decal: *mut ADecal) -> Hp2ArraySlots {
    unsafe { array_slots(ptr::addr_of_mut!((*decal).surf_list)) }
}

unsafe fn actor_level(actor: *mut ADecal) -> RuntimeResult<*mut ULevel> {
    if actor.is_null() {
        return Err(invalid("receiver"));
    }
    let info = unsafe {
        ptr::addr_of!((*actor).base.level)
            .read_unaligned()
            .cast::<AActor>()
    };
    if info.is_null() {
        return Err(invalid("receiver Level"));
    }
    let level = unsafe {
        ptr::addr_of!((*info).x_level)
            .read_unaligned()
            .cast::<ULevel>()
    };
    if level.is_null() {
        return Err(invalid("receiver XLevel"));
    }
    Ok(level)
}

unsafe fn actor_model(actor: *mut ADecal) -> RuntimeResult<*mut UModel> {
    let level = unsafe { actor_level(actor)? };
    let model = unsafe {
        ptr::addr_of!((*level).model)
            .read_unaligned()
            .cast::<UModel>()
    };
    if model.is_null() {
        return Err(invalid("level Model"));
    }
    Ok(model)
}

unsafe extern "C" fn enabled(raw: *mut c_void, out: *mut i32) -> i32 {
    if raw.is_null() {
        return 1;
    }
    let context = unsafe { context(raw) };
    let result = (|| {
        let level = unsafe { actor_level(context.actor)? };
        let engine = unsafe {
            ptr::addr_of!((*level).base.engine)
                .read_unaligned()
                .cast::<UEngine>()
        };
        if engine.is_null() {
            return Err(invalid("level Engine"));
        }
        let client = unsafe {
            ptr::addr_of!((*engine).client)
                .read_unaligned()
                .cast::<UClient>()
        };
        Ok(i32::from(
            !client.is_null() && unsafe { (*client).decals() },
        ))
    })();
    unsafe { context.output(out, result) }
}

unsafe extern "C" fn state(raw: *mut c_void, out: *mut Hp2DecalState) -> i32 {
    if raw.is_null() {
        return 1;
    }
    let context = unsafe { context(raw) };
    let result = (|| {
        if context.actor.is_null() {
            return Err(invalid("receiver"));
        }
        let base = context.actor.cast::<AActor>();
        let texture = unsafe { ptr::addr_of!((*base).texture).read_unaligned() };
        let u_size = if texture.is_null() {
            0
        } else {
            unsafe { ptr::addr_of!((*texture.cast::<UTexture>()).base.u_size).read_unaligned() }
        };
        Ok(Hp2DecalState {
            texture,
            location: unsafe { ptr::addr_of!((*base).location).read_unaligned() },
            rotation: unsafe { ptr::addr_of!((*base).rotation).read_unaligned() },
            draw_scale: unsafe { ptr::addr_of!((*base).draw_scale).read_unaligned() },
            texture_u_size: u_size,
            multi: unsafe { ptr::addr_of!((*context.actor).multi_decal_level).read_unaligned() },
        })
    })();
    unsafe { context.output(out, result) }
}

unsafe extern "C" fn set_multi(raw: *mut c_void, value: i32) -> i32 {
    if raw.is_null() {
        return 1;
    }
    let context = unsafe { context(raw) };
    if context.actor.is_null() {
        return context.remember(invalid("receiver"));
    }
    unsafe {
        ptr::addr_of_mut!((*context.actor).multi_decal_level).write_unaligned(value);
    }
    0
}

unsafe extern "C" fn no_texture(raw: *mut c_void) -> i32 {
    if raw.is_null() {
        return 1;
    }
    let context = unsafe { context(raw) };
    let result = (|| {
        let category = context.access.intern_str("Dev", FindName::Add)?;
        context
            .access
            .emit_log(category, hp2_core::lit!("AttachDecal: No Texture"))
    })();
    context.status(result)
}

unsafe extern "C" fn random(raw: *mut c_void, out: *mut FVector) -> i32 {
    if raw.is_null() {
        return 1;
    }
    let context = unsafe { context(raw) };
    let access = context.access;
    let result = (|| {
        let value = loop {
            let unit = || access.rand_range_float(0.0, 1.0) * 2.0 - 1.0;
            let value = FVector {
                x: unit(),
                y: unit(),
                z: unit(),
            };
            if crate::vec::size_squared(value) <= 1.0 {
                break value;
            }
        };
        Ok(crate::vec::unsafe_normal(value))
    })();
    unsafe { context.output(out, result) }
}

unsafe extern "C" fn trig() -> *const f32 {
    hp2_core::math::trig_table().as_ptr()
}

unsafe extern "C" fn line(
    raw: *mut c_void,
    end: *const FVector,
    start: *const FVector,
    out: *mut i32,
) -> i32 {
    if raw.is_null() || end.is_null() || start.is_null() {
        return 1;
    }
    let context = unsafe { context(raw) };
    let result = (|| {
        let model = unsafe { actor_model(context.actor)? };
        let end = unsafe { end.read_unaligned() };
        let start = unsafe { start.read_unaligned() };
        let mut hit: FCheckResult = crate::prim::empty_result();
        hit.time = 1.0;
        let clear = unsafe {
            crate::collision_owner::world_line_check(
                context.access,
                model,
                &mut hit,
                end,
                start,
                FVector {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                TRACE_VIS_BLOCKING,
            )?
        };
        Ok(if clear { INDEX_NONE } else { hit.item })
    })();
    unsafe { context.output(out, result) }
}

unsafe extern "C" fn surface(raw: *mut c_void, index: i32, out: *mut Hp2DecalSurface) -> i32 {
    if raw.is_null() {
        return 1;
    }
    let context = unsafe { context(raw) };
    let result = (|| {
        let model = unsafe { actor_model(context.actor)? };
        let surface = unsafe { surface_ptr(model, index)? };
        let base_index = unsafe { ptr::addr_of!((*surface).p_base).read_unaligned() };
        let normal_index = unsafe { ptr::addr_of!((*surface).v_normal).read_unaligned() };
        let base =
            unsafe { model_item::<FVector>(model, ModelField::Points, base_index, "point array")? };
        let normal = unsafe {
            model_item::<FVector>(model, ModelField::Vectors, normal_index, "vector array")?
        };
        let count = unsafe { array_count(surface_nodes(surface), "surface node array")? };
        Ok(Hp2DecalSurface {
            base,
            normal,
            texture: unsafe { ptr::addr_of!((*surface).texture).read_unaligned() },
            flags: unsafe { ptr::addr_of!((*surface).poly_flags).read_unaligned() },
            node_count: count,
        })
    })();
    unsafe { context.output(out, result) }
}

unsafe extern "C" fn node(raw: *mut c_void, index: i32, out: *mut Hp2DecalNode) -> i32 {
    if raw.is_null() {
        return 1;
    }
    let context = unsafe { context(raw) };
    let result = (|| {
        let model = unsafe { actor_model(context.actor)? };
        let node =
            unsafe { model_item::<FBspNode>(model, ModelField::Nodes, index, "node array")? };
        Ok(Hp2DecalNode {
            surface: node.i_surf,
            next_plane: node.i_plane,
            vertex_count: i32::from(node.num_vertices),
        })
    })();
    unsafe { context.output(out, result) }
}

unsafe extern "C" fn vertex(
    raw: *mut c_void,
    node_index: i32,
    index: i32,
    out: *mut FVector,
) -> i32 {
    if raw.is_null() {
        return 1;
    }
    let context = unsafe { context(raw) };
    let result = (|| {
        let model = unsafe { actor_model(context.actor)? };
        let node =
            unsafe { model_item::<FBspNode>(model, ModelField::Nodes, node_index, "node array")? };
        if index < 0 || index >= i32::from(node.num_vertices) {
            return Err(invalid("node vertex index"));
        }
        let vert_index = node
            .i_vert_pool
            .checked_add(index)
            .ok_or_else(|| invalid("vertex index"))?;
        let vert =
            unsafe { model_item::<FVert>(model, ModelField::Verts, vert_index, "vertex array")? };
        unsafe { model_item::<FVector>(model, ModelField::Points, vert.p_vertex, "point array") }
    })();
    unsafe { context.output(out, result) }
}

unsafe extern "C" fn surface_node(
    raw: *mut c_void,
    surface_index: i32,
    index: i32,
    out: *mut i32,
) -> i32 {
    if raw.is_null() {
        return 1;
    }
    let context = unsafe { context(raw) };
    let result = (|| {
        let model = unsafe { actor_model(context.actor)? };
        let surface = unsafe { surface_ptr(model, surface_index)? };
        unsafe { array_item::<i32>(surface_nodes(surface), index, "surface node array") }
    })();
    unsafe { context.output(out, result) }
}

unsafe extern "C" fn decal_count(raw: *mut c_void, surface_index: i32, out: *mut i32) -> i32 {
    if raw.is_null() {
        return 1;
    }
    let context = unsafe { context(raw) };
    let result = (|| {
        let model = unsafe { actor_model(context.actor)? };
        let surface = unsafe { surface_ptr(model, surface_index)? };
        unsafe { array_count(surface_decals(surface), "surface decal array") }
    })();
    unsafe { context.output(out, result) }
}

unsafe fn surface_decal(
    model: *mut UModel,
    surface_index: i32,
    index: i32,
) -> RuntimeResult<*mut generated::FDecal> {
    let surface = unsafe { surface_ptr(model, surface_index)? };
    let slots = unsafe { surface_decals(surface) };
    let count = unsafe { array_count(slots, "surface decal array")? };
    let data = unsafe { slots.data.read_unaligned() };
    if index < 0 || index >= count || data.is_null() {
        return Err(invalid("surface decal index"));
    }
    Ok(unsafe { data.cast::<generated::FDecal>().add(index as usize) })
}

unsafe extern "C" fn decal_texture(
    raw: *mut c_void,
    surface_index: i32,
    index: i32,
    out: *mut *mut c_void,
) -> i32 {
    if raw.is_null() {
        return 1;
    }
    let context = unsafe { context(raw) };
    let result = (|| {
        let model = unsafe { actor_model(context.actor)? };
        let decal = unsafe { surface_decal(model, surface_index, index)? };
        let actor = unsafe {
            ptr::addr_of!((*decal).actor)
                .read_unaligned()
                .cast::<AActor>()
        };
        if actor.is_null() {
            return Err(invalid("surface decal Actor"));
        }
        Ok(unsafe { ptr::addr_of!((*actor).texture).read_unaligned() })
    })();
    unsafe { context.output(out, result) }
}

unsafe extern "C" fn insert(raw: *mut c_void, surface_index: i32, index: i32) -> i32 {
    if raw.is_null() {
        return 1;
    }
    let context = unsafe { context(raw) };
    let result = (|| {
        let model = unsafe { actor_model(context.actor)? };
        let surface = unsafe { surface_ptr(model, surface_index)? };
        let slots = unsafe { surface_decals(surface) };
        let count = unsafe { array_count(slots, "surface decal array")? };
        if index < 0 || index > count {
            return Err(invalid("surface decal insertion index"));
        }
        unsafe {
            array_storage::insert_zeroed(
                slots,
                index,
                1,
                size_of::<generated::FDecal>() as i32,
                ptr::null(),
                ARRAY_SERVICES,
            )
        }
        .map_err(|_| RuntimeError::new("Unable to insert canonical FDecal"))?;
        // Re-resolve both containers after insertion; only the index crosses it.
        let model = unsafe { actor_model(context.actor)? };
        let decal = unsafe { surface_decal(model, surface_index, index)? };
        unsafe { ptr::addr_of_mut!((*decal).actor).write_unaligned(context.actor.cast()) };
        Ok(())
    })();
    context.status(result)
}

unsafe extern "C" fn set_vertex(
    raw: *mut c_void,
    surface_index: i32,
    decal_index: i32,
    vertex_index: i32,
    value: *const FVector,
) -> i32 {
    if raw.is_null() || value.is_null() {
        return 1;
    }
    let context = unsafe { context(raw) };
    let result = (|| {
        if !(0..4).contains(&vertex_index) {
            return Err(invalid("decal vertex index"));
        }
        let model = unsafe { actor_model(context.actor)? };
        let decal = unsafe { surface_decal(model, surface_index, decal_index)? };
        let value = unsafe { value.read_unaligned() };
        unsafe {
            ptr::addr_of_mut!((*decal).vertices)
                .cast::<FVector>()
                .add(vertex_index as usize)
                .write_unaligned(value);
        }
        Ok(())
    })();
    context.status(result)
}

unsafe extern "C" fn add_node(
    raw: *mut c_void,
    surface_index: i32,
    decal_index: i32,
    node_index: i32,
) -> i32 {
    if raw.is_null() {
        return 1;
    }
    let context = unsafe { context(raw) };
    let result = (|| {
        let model = unsafe { actor_model(context.actor)? };
        let decal = unsafe { surface_decal(model, surface_index, decal_index)? };
        let nodes = unsafe { array_slots(ptr::addr_of_mut!((*decal).nodes)) };
        let index = unsafe {
            array_storage::add(
                nodes,
                1,
                size_of::<i32>() as i32,
                ptr::null(),
                ARRAY_SERVICES,
            )
        }
        .map_err(|_| RuntimeError::new("Unable to append canonical FDecal node"))?;
        // Re-resolve after possible relocation, then publish the new scalar.
        let model = unsafe { actor_model(context.actor)? };
        let decal = unsafe { surface_decal(model, surface_index, decal_index)? };
        let nodes = unsafe { array_slots(ptr::addr_of_mut!((*decal).nodes)) };
        let count = unsafe { array_count(nodes, "decal node array")? };
        if index < 0 || index >= count {
            return Err(invalid("decal node append result"));
        }
        let data = unsafe { nodes.data.read_unaligned() };
        unsafe {
            data.cast::<i32>()
                .add(index as usize)
                .write_unaligned(node_index);
        }
        Ok(())
    })();
    context.status(result)
}

unsafe extern "C" fn list_count(raw: *mut c_void, out: *mut i32) -> i32 {
    if raw.is_null() {
        return 1;
    }
    let context = unsafe { context(raw) };
    let result = if context.actor.is_null() {
        Err(invalid("receiver"))
    } else {
        unsafe { array_count(decal_slots(context.actor), "SurfList") }
    };
    unsafe { context.output(out, result) }
}

unsafe extern "C" fn list_item(raw: *mut c_void, index: i32, out: *mut i32) -> i32 {
    if raw.is_null() {
        return 1;
    }
    let context = unsafe { context(raw) };
    let result = if context.actor.is_null() {
        Err(invalid("receiver"))
    } else {
        unsafe { array_item::<i32>(decal_slots(context.actor), index, "SurfList") }
    };
    unsafe { context.output(out, result) }
}

unsafe extern "C" fn list_add(raw: *mut c_void, surface: i32) -> i32 {
    if raw.is_null() {
        return 1;
    }
    let context = unsafe { context(raw) };
    let result = (|| {
        if context.actor.is_null() {
            return Err(invalid("receiver"));
        }
        let slots = unsafe { decal_slots(context.actor) };
        let index = unsafe { array_count(slots, "SurfList")? };
        unsafe {
            array_storage::add(
                slots,
                1,
                size_of::<i32>() as i32,
                ptr::null(),
                ARRAY_SERVICES,
            )
        }
        .map_err(|_| RuntimeError::new("Unable to append canonical ADecal SurfList"))?;
        // Only the append index crosses realloc; reacquire the current header.
        let slots = unsafe { decal_slots(context.actor) };
        let count = unsafe { array_count(slots, "SurfList")? };
        if index < 0 || index >= count {
            return Err(invalid("SurfList append result"));
        }
        let data = unsafe { slots.data.read_unaligned() };
        unsafe {
            data.cast::<i32>()
                .add(index as usize)
                .write_unaligned(surface);
        }
        Ok(())
    })();
    context.status(result)
}

unsafe extern "C" fn list_remove(raw: *mut c_void, index: i32) -> i32 {
    if raw.is_null() {
        return 1;
    }
    let context = unsafe { context(raw) };
    let result = (|| {
        if context.actor.is_null() {
            return Err(invalid("receiver"));
        }
        let slots = unsafe { decal_slots(context.actor) };
        let count = unsafe { array_count(slots, "SurfList")? };
        if index < 0 || index >= count {
            return Err(invalid("SurfList index"));
        }
        unsafe {
            array_storage::remove(
                slots,
                index,
                1,
                size_of::<i32>() as i32,
                ptr::null(),
                ARRAY_SERVICES,
            )
        }
        .map_err(|_| RuntimeError::new("Unable to remove canonical ADecal SurfList entry"))
    })();
    context.status(result)
}

unsafe extern "C" fn is_owner(
    raw: *mut c_void,
    surface_index: i32,
    index: i32,
    out: *mut i32,
) -> i32 {
    if raw.is_null() {
        return 1;
    }
    let context = unsafe { context(raw) };
    let result = (|| {
        let model = unsafe { actor_model(context.actor)? };
        let decal = unsafe { surface_decal(model, surface_index, index)? };
        let owner = unsafe { ptr::addr_of!((*decal).actor).read_unaligned() };
        Ok(i32::from(owner == context.actor.cast()))
    })();
    unsafe { context.output(out, result) }
}

unsafe extern "C" fn remove(raw: *mut c_void, surface_index: i32, index: i32) -> i32 {
    if raw.is_null() {
        return 1;
    }
    let context = unsafe { context(raw) };
    let result = (|| {
        let model = unsafe { actor_model(context.actor)? };
        let decal = unsafe { surface_decal(model, surface_index, index)? };
        unsafe {
            model::destroy_model_element(ModelArrayKind::FDecal, decal.cast(), ARRAY_SERVICES)
        }
        .map_err(|_| RuntimeError::new("Unable to destroy canonical FDecal node storage"))?;
        // FDecal's node array is destroyed before TArray<FDecal>::Remove; refresh
        // surface storage and use the original index after that mutation.
        let model = unsafe { actor_model(context.actor)? };
        let surface = unsafe { surface_ptr(model, surface_index)? };
        let slots = unsafe { surface_decals(surface) };
        let count = unsafe { array_count(slots, "surface decal array")? };
        if index < 0 || index >= count {
            return Err(invalid("surface decal removal index"));
        }
        unsafe {
            array_storage::remove(
                slots,
                index,
                1,
                size_of::<generated::FDecal>() as i32,
                ptr::null(),
                ARRAY_SERVICES,
            )
        }
        .map_err(|_| RuntimeError::new("Unable to remove canonical FDecal"))
    })();
    context.status(result)
}

const SERVICES: Hp2DecalServices = Hp2DecalServices {
    enabled,
    state,
    set_multi,
    no_texture,
    random,
    trig,
    line,
    surface,
    node,
    vertex,
    surface_node,
    decal_count,
    decal_texture,
    insert,
    set_vertex,
    add_node,
    list_count,
    list_item,
    list_add,
    list_remove,
    is_owner,
    remove,
};

fn finish(
    context: &mut DecalContext<'_>,
    result: Result<(), i32>,
    operation: &str,
) -> RuntimeResult<()> {
    if let Some(error) = context.error.take() {
        return Err(error);
    }
    result.map_err(|_| RuntimeError::new(&format!("Canonical ADecal {operation} failed")))
}

/// Runs shared projection against this canonical `ADecal` and its live BSP model.
///
/// # Safety
/// `actor` is a live canonical `ADecal`, and every model object it references is
/// owned by the same CoreAccess graph for the duration of this synchronous call.
pub(crate) unsafe fn attach(
    access: CoreAccess<'_>,
    actor: *mut ADecal,
    distance: f32,
    direction: FVector,
) -> RuntimeResult<*mut c_void> {
    let mut context = DecalContext {
        access,
        actor,
        error: None,
    };
    let raw = (&mut context as *mut DecalContext<'_>).cast();
    let result = unsafe { decal::attach(&SERVICES, raw, distance, direction) };
    if let Some(error) = context.error.take() {
        return Err(error);
    }
    result.map_err(|_| RuntimeError::new("Canonical ADecal attach failed"))
}

/// Runs the shared detach policy over this canonical `ADecal`'s live SurfList.
///
/// # Safety
/// `actor` is a live canonical `ADecal`; all referenced model storage is owned
/// by this CoreAccess graph throughout the call.
pub(crate) unsafe fn detach(access: CoreAccess<'_>, actor: *mut ADecal) -> RuntimeResult<()> {
    let mut context = DecalContext {
        access,
        actor,
        error: None,
    };
    let raw = (&mut context as *mut DecalContext<'_>).cast();
    let result = unsafe { decal::detach(&SERVICES, raw) };
    finish(&mut context, result, "detach")
}
