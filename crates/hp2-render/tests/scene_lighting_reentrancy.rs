//! A stack allocator may reenter native code and relocate a model's light array.
//! Keep the old allocation alive so the regression fails deterministically rather
//! than depending on a dangling-pointer crash.
use hp2_abi::FVector;
use hp2_abi::engine::generated::FMemStack;
use hp2_abi::render_scene::{FSceneNode, FVolActorLink, TArrayRaw};
use hp2_core::mem::{self, Hp2MemServices};
use hp2_render::scene_pipeline::Hp2SceneStackServices;
use hp2_render::scene_pipeline::helpers_core::{
    Hp2ScenePipelineHelperLighting, Hp2ScenePipelineHelperLightingActor, leaf_lighting,
};
use std::alloc::{Layout, alloc, dealloc};
use std::ffi::{c_char, c_void};
use std::ptr;

struct Light {
    id: i32,
    tag: i32,
}

#[repr(C)]
struct Probe {
    // Also a valid NUL-terminated allocator tag, regardless of TCHAR width.
    tag: [u8; 8],
    lights: TArrayRaw,
    replacement: *mut c_void,
    allocations: Vec<(*mut u8, Layout)>,
}

unsafe extern "C" fn allocate(size: u32, tag: *const c_void, out: *mut *mut c_void) -> i32 {
    // SAFETY: this stack's tag is the first field of the live, boxed Probe.
    unsafe {
        let probe = &mut *tag.cast_mut().cast::<Probe>();
        probe.lights.data = probe.replacement;
        let layout = Layout::from_size_align(size as usize, 16).unwrap();
        let allocation = alloc(layout);
        if allocation.is_null() {
            return 1;
        }
        probe.allocations.push((allocation, layout));
        out.write(allocation.cast());
    }
    0
}
unsafe extern "C" fn release(_: *mut c_void) {
    // The test deallocates recorded blocks after the core unused list is drained.
}
unsafe extern "C" fn error(_: *const c_void) -> i32 {
    1
}
unsafe extern "C" fn assertion(_: *const c_char, _: *const c_char, _: i32) -> i32 {
    1
}
unsafe extern "C" fn allocation_failure(_: *mut c_void, _: i32, _: u32) {}

unsafe extern "C" fn actor(
    _: *mut c_void,
    actor: *mut c_void,
    out: *mut Hp2ScenePipelineHelperLightingActor,
) -> i32 {
    // SAFETY: actor points to a live Light whose tag outlives this traversal.
    unsafe {
        (*out).location = FVector {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        (*out).volume_radius = 0;
        (*out).tag = ptr::addr_of_mut!((*actor.cast::<Light>()).tag);
    }
    1
}
unsafe extern "C" fn construct_static(
    _: *mut c_void,
    _: *mut FSceneNode,
    actor: *mut c_void,
    next: *mut FVolActorLink,
    storage: *mut FVolActorLink,
) -> i32 {
    // SAFETY: the core stack allocated suitably aligned, disjoint link storage.
    unsafe {
        storage.write(FVolActorLink {
            location: FVector {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            actor,
            next,
            volumetric: 1,
        });
    }
    1
}
unsafe extern "C" fn construct_dynamic(
    _: *mut c_void,
    _: *mut FVolActorLink,
    _: *mut FVolActorLink,
    _: *mut FVolActorLink,
) -> i32 {
    0
}

#[test]
fn volumetric_lighting_reloads_the_light_array_after_stack_allocation() {
    let mut light_actors = [
        Light { id: 1, tag: 0 },
        Light { id: 2, tag: 0 },
        Light { id: 3, tag: 0 },
    ];
    let [first_light, old_light, new_light] = light_actors
        .each_mut()
        .map(|light| ptr::from_mut(light).cast::<c_void>());
    let mut original = [first_light, old_light, ptr::null_mut()];
    let mut replacement = [first_light, new_light, ptr::null_mut()];
    let mut probe = Box::new(Probe {
        tag: [0; 8],
        lights: TArrayRaw {
            data: original.as_mut_ptr().cast(),
            array_num: 3,
            array_max: 3,
        },
        replacement: replacement.as_mut_ptr().cast(),
        allocations: Vec::new(),
    });
    let services = Hp2MemServices {
        malloc: allocate,
        free: release,
        errorf: error,
        fail_assert: assertion,
    };
    // SAFETY: these pinned ABI views contain only numeric values and raw pointers.
    let mut stack: FMemStack = unsafe { std::mem::zeroed() };
    let mut frame: FSceneNode = unsafe { std::mem::zeroed() };
    let context = ptr::from_mut(&mut *probe).cast();
    mem::init(&mut stack, 4096, context);
    let mut first = ptr::null_mut();
    let arguments = Hp2ScenePipelineHelperLighting {
        context,
        memory: Hp2SceneStackServices {
            memory: &services,
            slow_checks: 1,
            allocation_failure,
        },
        stack: ptr::from_mut(&mut stack).cast(),
        frame: &mut frame,
        lights: ptr::addr_of!(probe.lights).cast(),
        first_static: 0,
        dynamic: ptr::null_mut(),
        first: &mut first,
        stamp: 9,
        actor,
        construct_static,
        construct_dynamic,
    };
    // SAFETY: all headers, actor tags, backing arrays and callbacks outlive the call.
    let result = unsafe { leaf_lighting(&arguments) };
    let mut actors = Vec::new();
    let mut link = first;
    while !link.is_null() {
        // SAFETY: every link belongs to the still-live core stack.
        unsafe {
            actors.push((*(*link).actor.cast::<Light>()).id);
            link = (*link).next;
        }
    }
    // SAFETY: this integration-test process owns all chunks and uses one allocator.
    unsafe {
        mem::free_chunks(&mut stack, ptr::null_mut());
        mem::exit(&stack, &services, c"scene_lighting_reentrancy.rs".as_ptr()).unwrap();
        for (allocation, layout) in probe.allocations.drain(..) {
            dealloc(allocation, layout);
        }
    }
    assert_eq!(result, 1);
    assert_eq!(
        actors,
        [3, 1],
        "the relocated array must replace old light 2 with light 3"
    );
}
