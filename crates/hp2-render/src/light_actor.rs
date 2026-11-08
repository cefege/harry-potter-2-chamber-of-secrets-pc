//! Actor lighting kernels for `UnLight.cpp` (`FLightManager` actor group).
//!
//! `light_manager` owns the shared storage and setup/lifetime orchestration.
//! Native leaves provide SDK cache access, linked-list field reads, statistics,
//! and virtual engine calls. Candidate selection, lighting decisions and finish
//! order stay here; callback failures stop before their outputs are consumed.
//!
//! Fused operations follow the oracle `UnLight.o` disassembly; the table is in
//! `/tmp/RenLight/fma_actor.md`. Everything not listed there is unfused.

use crate::services_light::Hp2LightServices;
use hp2_abi::engine::generated::{AActor, ULevel, UModel};
use std::ffi::c_void;

/// `enum {MaxActorLights=16}` in UnLight.cpp.
pub const MAX_ACTOR_LIGHTS: usize = 16;
/// `enum {DesiredActorLights=3}` in UnLight.cpp.
pub const DESIRED_ACTOR_LIGHTS: i32 = 3;
/// Capacity of `Consider[120]` in UnLight.cpp.
pub const CONSIDER_CAP: usize = 120;

const LT_NONE: u32 = 0;
const LT_STEADY: u32 = 1;
const LE_OMNI_BUMP_MAP: u32 = 15;
const ALO_STATIC_LIGHT: u32 = 0;
const ALO_DYNAMIC_LIGHT: u32 = 1;
const ALO_MOVING_LIGHT: u32 = 2;
pub(crate) const ALO_NOT_LIGHT: u32 = 3;

/// C++ `LightRec` mirror (56 bytes): one candidate light. The shell fills it
/// from the `ALight` fields before `consider_add` and writes the tags back.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LightRec {
    pub actor: *mut c_void,
    pub location: [f32; 3],
    pub radius: f32,
    pub extra_tag: i32,
    pub lighting_tag: i32,
    pub special_tag: i32,
    pub index: i32,
    pub brightness: u32,
    pub special_lit: u32,
    pub is_static: u32,
    pub is_movable: u32,
}

/// C++ `GConsider` mirror: the candidate set for one actor (replaces
/// `Consider[120]`, `NumConsider`, `ConsiderTag`).
#[repr(C)]
pub struct LightConsiderSet {
    pub tag: i32,
    pub num: i32,
    pub actor_location: [f32; 3],
    pub actor_special_lit: u32,
    pub recs: [LightRec; CONSIDER_CAP],
}

/// C++ `FInfo` mirror (16 bytes): one entry of the sender cache block.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SenderEntry {
    pub actor: *mut c_void,
    pub factor: u8,
    pub index: i32,
}

/// `AddLight` decision input and output (C++ `Hp2LightAddIo`). `light_type`,
/// `light_effect`, `light_brightness` and the dynamic/static/no-delete/changed
/// flags are read straight off the `light` `AActor` mirror; the C++ shell no
/// longer copies them in.
#[repr(C)]
pub struct LightAddIo {
    pub actor: *mut c_void,
    pub light: *mut c_void,
    pub accept: u32,
    pub opt: u32,
    pub moving_delta: u32,
    pub static_delta: u32,
    pub dynamic_delta: u32,
    pub static_lighting_changed: u32,
}

/// Visibility pass input (C++ `LightVisIn`). `delta` is the shell's
/// `Clamp(appRound((CurrentTime-LastUpdateTime)*768),0,255)`.
#[repr(C)]
pub struct LightVisIn {
    pub delta: i32,
    pub frame_count: i32,
    pub first_see: u32,
}

/// `SetupForActor` begin-of-actor math input and output (C++ `ActorBeginIo`).
#[repr(C)]
pub struct ActorBeginIo {
    pub scale_glow: f32,
    pub specular_glow: f32,
    pub specular_width: u32,
    pub ambient_glow: u32,
    pub time: f32,
    pub hsv: [f32; 3],
    pub diffuse: f32,
    pub specular: f32,
    pub spec_min_cos: f32,
    /// `AmbientVector` as FPlane: lane 3 (W) is 0 (`FPlane(const FVector&)`, UnMath.h:998).
    pub ambient: [f32; 4],
}

const _: () = assert!(std::mem::size_of::<LightRec>() == 56);
const _: () = assert!(std::mem::size_of::<LightConsiderSet>() == 24 + 120 * 56);
const _: () = assert!(std::mem::size_of::<SenderEntry>() == 16);
const _: () = assert!(std::mem::size_of::<LightAddIo>() == 40);
const _: () = assert!(std::mem::size_of::<LightVisIn>() == 12);
const _: () = assert!(std::mem::size_of::<ActorBeginIo>() == 60);

/// `appRound` on the clang path is `static_cast<INT>` (UnGnuG.h:623-639).
fn app_round(f: f32) -> i32 {
    f as i32
}

/// Unqualified `SqrtApprox(F)` in UnLight.cpp resolves to Render.h:292,
/// `appSqrt(F)`, not `GMath.SqrtApprox` (UnMath.h:1587). The `appSqrt` double
/// result is stored as FLOAT.
fn sqrt_plain(f: f32) -> f32 {
    (f as f64).sqrt() as f32
}

/// `FVector |` (UnMath.h dot). Order and fusion match the oracle: `Y*Y`
/// plain, then two fused multiply-adds for X and Z.
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[2].mul_add(b[2], a[0].mul_add(b[0], a[1] * b[1]))
}

/// `SetupForActor` begin math: diffuse, specular, specular cosine floor and
/// ambient vector (UnLight.cpp, before the zone/HSV reads are combined).
/// The sin branch is `0.25f + 0.2f*appSin(...)` in double, one fused op.
pub fn actor_begin(io: &mut ActorBeginIo) {
    let diffuse = io.scale_glow * 2.0;
    let mut specular = io.specular_glow * 2.0;
    let t = 1.0 - io.specular_width as f32 / 170.0;
    let spec_min_cos = t * t;
    let u = 1.0 - spec_min_cos;
    specular /= u * u;
    if diffuse == 1.4 {
        specular = 6.0;
    }
    let c = if io.ambient_glow != 255 {
        io.ambient_glow as f32 / 255.0
    } else {
        f64::from(io.time * 8.0)
            .sin()
            .mul_add(f64::from(0.2f32), 0.25) as f32
    };
    io.diffuse = diffuse;
    io.specular = specular;
    io.spec_min_cos = spec_min_cos;
    for k in 0..3 {
        io.ambient[k] = c + io.hsv[k] * diffuse;
    }
    io.ambient[3] = 0.0;
}

/// `AddLight` decision. The shell has already checked `LastLight>=FinalLight`
/// and set `io.actor`/`io.light`. `no_dynamic_lights` and `effect_flags` are
/// the remaining engine reads; `effect_flags` returns bit 0 = `IsSpatialDynamic`,
/// bit 1 = `IsMergeDynamic`. `Light->LightType/LightEffect/LightBrightness` and
/// the dynamic/static/no-delete/changed flags are read straight off the
/// `AActor` mirror (raw deref of `io.light`, as the C++ shell did).
pub fn add_light_decide(
    io: &mut LightAddIo,
    no_dynamic_lights: impl Fn() -> bool,
    effect_flags: impl Fn(u8) -> u32,
) {
    io.accept = 0;
    io.opt = 0;
    io.moving_delta = 0;
    io.static_delta = 0;
    io.dynamic_delta = 0;
    io.static_lighting_changed = 0;
    let light = unsafe { &*(io.light as *const AActor) };
    let light_type = light.light_type as u32;
    let light_effect = light.light_effect as u32;
    let brightness = light.light_brightness as u32;
    if light_type == LT_NONE || brightness == 0 || io.actor == io.light {
        return;
    }
    // (opt, which counter): 0 none, 1 moving, 2 static, 3 dynamic.
    let (opt, slot) = if !io.actor.is_null() {
        (ALO_MOVING_LIGHT, 1)
    } else if light_effect == LE_OMNI_BUMP_MAP {
        (ALO_NOT_LIGHT, 0)
    } else if light.b_dynamic_light() || (!light.b_static() && !light.b_no_delete()) {
        if no_dynamic_lights() {
            return;
        }
        (ALO_MOVING_LIGHT, 1)
    } else if (light.b_static() && light_type == LT_STEADY && effect_flags(light_effect as u8) == 0)
        || no_dynamic_lights()
    {
        (ALO_STATIC_LIGHT, 2)
    } else {
        (ALO_DYNAMIC_LIGHT, 3)
    };
    match slot {
        1 => io.moving_delta = 1,
        2 => io.static_delta = 1,
        3 => io.dynamic_delta = 1,
        _ => {}
    }
    io.opt = opt;
    io.static_lighting_changed = light.b_light_changed() as u32;
    io.accept = 1;
}

/// `GConsider` reset at the start of an actor's candidate collection.
pub fn consider_reset(set: &mut LightConsiderSet, location: [f32; 3], special_lit: u32) {
    set.tag = set.tag.wrapping_add(1);
    set.num = 0;
    set.actor_location = location;
    set.actor_special_lit = special_lit;
}

/// `AddConsider` body. Returns true when the light was accepted and its tags
/// changed; the shell writes `lighting_tag`, `extra_tag`, `special_tag` back.
/// Callers check `set.num < CONSIDER_CAP` before each call.
pub fn consider_add(set: &mut LightConsiderSet, rec: &mut LightRec, factor: u8) -> bool {
    if rec.extra_tag == set.tag {
        return false;
    }
    let dx = rec.location[0] - set.actor_location[0];
    let dy = rec.location[1] - set.actor_location[1];
    let dz = rec.location[2] - set.actor_location[2];
    let dist_sq = dx * dx + dy * dy + dz * dz;
    let radius = rec.radius;
    if rec.special_lit == set.actor_special_lit && dist_sq < radius * radius {
        rec.lighting_tag =
            app_round((1.0 - sqrt_plain(dist_sq) / radius) * rec.brightness as f32 * 1024.0);
        rec.extra_tag = set.tag;
        rec.special_tag = factor as i32;
        set.recs[set.num as usize] = *rec;
        set.num += 1;
        return true;
    }
    false
}

/// `Compare` (UnLight.cpp): 1 - 2*(A1->LightingTag > A2->LightingTag).
fn compare(a: &LightRec, b: &LightRec) -> i32 {
    1 - 2 * ((a.lighting_tag > b.lighting_tag) as i32)
}

/// `Sort<AActor*>` (UnTemplate.h:1727-1800): literal port, same pivots, same
/// partitioning and same recursion-stack order, so equal keys land identically.
pub fn consider_sort(recs: &mut [LightRec]) {
    let num = recs.len() as isize;
    if num < 2 {
        return;
    }
    let mut stack = [(0isize, 0isize); 32];
    stack[0] = (0, num - 1);
    let mut top: isize = 0;
    while top >= 0 {
        let mut cur = stack[top as usize];
        'current: loop {
            let count = cur.1 - cur.0 + 1;
            if count <= 8 {
                while cur.1 > cur.0 {
                    let mut max = cur.0;
                    let mut item = cur.0 + 1;
                    while item <= cur.1 {
                        if compare(&recs[item as usize], &recs[max as usize]) > 0 {
                            max = item;
                        }
                        item += 1;
                    }
                    recs.swap(max as usize, cur.1 as usize);
                    cur.1 -= 1;
                }
                break 'current;
            }
            recs.swap((cur.0 + count / 2) as usize, cur.0 as usize);
            let mut inner = (cur.0, cur.1 + 1);
            loop {
                loop {
                    inner.0 += 1;
                    if !(inner.0 <= cur.1
                        && compare(&recs[inner.0 as usize], &recs[cur.0 as usize]) <= 0)
                    {
                        break;
                    }
                }
                loop {
                    inner.1 -= 1;
                    if !(inner.1 > cur.0
                        && compare(&recs[inner.1 as usize], &recs[cur.0 as usize]) >= 0)
                    {
                        break;
                    }
                }
                if inner.0 > inner.1 {
                    break;
                }
                recs.swap(inner.0 as usize, inner.1 as usize);
            }
            recs.swap(cur.0 as usize, inner.1 as usize);
            if inner.1 - 1 - cur.0 >= cur.1 - inner.0 {
                if cur.0 + 1 < inner.1 {
                    stack[top as usize] = (cur.0, inner.1 - 1);
                    top += 1;
                }
                if cur.1 > inner.0 {
                    cur.0 = inner.0;
                    continue 'current;
                }
            } else {
                if cur.1 > inner.0 {
                    stack[top as usize] = (inner.0, cur.1);
                    top += 1;
                }
                if cur.0 + 1 < inner.1 {
                    cur.1 = inner.1 - 1;
                    continue 'current;
                }
            }
            break 'current;
        }
        top -= 1;
    }
}

/// Visibility pass over the sorted candidates (UnLight.cpp visibility block).
/// `ops.line_check(actor, light)` returns true when the path is clear.
/// `ops.add_light(actor, light)` is `GLightManagerInstance.AddLight`.
/// Only `special_tag` of `set.recs` changes; the shell writes it back.
pub fn consider_visibility<O: CandidateOps>(
    ops: &mut O,
    set: &mut LightConsiderSet,
    input: &LightVisIn,
    actor: *mut c_void,
) -> Result<(), O::Error> {
    let delta = input.delta;
    let first_see = input.first_see != 0;
    let mut threshold: i32 = -1;
    let mut num_static: i32 = 0;
    let mut num_real: i32 = 0;
    let num = set.num as usize;
    for rec in set.recs[..num].iter_mut() {
        let is_static = rec.is_static != 0;
        let count = if is_static { num_static } else { num_real };
        let visible = if rec.lighting_tag < threshold || count >= DESIRED_ACTOR_LIGHTS {
            false
        } else if !is_static && rec.is_movable != 0 {
            true
        } else if first_see || ((input.frame_count ^ rec.index) & 15) == 0 {
            let v = ops.line_check(actor, rec.actor)?;
            if v && first_see {
                rec.special_tag = 255;
            }
            v
        } else {
            (rec.special_tag & 1) != 0
        };
        if visible {
            num_real += 1;
            if is_static {
                num_static += 1;
            }
            if threshold == -1 {
                threshold = rec.lighting_tag / 8;
            }
            ops.add_light(actor, rec.actor)?;
            rec.special_tag = (rec.special_tag + delta).min(255) | 1;
        } else {
            rec.special_tag = (rec.special_tag - delta).max(0) & !1;
            if rec.special_tag > 0 {
                ops.add_light(actor, rec.actor)?;
            }
        }
    }
    Ok(())
}

/// Typed leaves for candidate collection, shared by Native and Core hosts.
pub trait CandidateOps {
    type Error;
    fn sender_alive(&mut self, entry: &SenderEntry) -> bool;
    fn world_light_radius(&mut self, light: *mut c_void) -> f32;
    fn line_check(&mut self, actor: *mut c_void, light: *mut c_void) -> Result<bool, Self::Error>;
    fn add_light(&mut self, actor: *mut c_void, light: *mut c_void) -> Result<(), Self::Error>;
    fn validate_sender_count(&mut self, count: i32, capacity: usize) -> Result<(), Self::Error> {
        assert!(
            count >= 0
                && count as usize <= capacity
                && count as usize <= MAX_ACTOR_LIGHTS
                && capacity == MAX_ACTOR_LIGHTS,
            "actor light sender count {count} exceeds fixed source capacity {MAX_ACTOR_LIGHTS} (slice capacity {capacity})"
        );
        Ok(())
    }
}

unsafe fn fill_rec<O: CandidateOps>(
    ops: &mut O,
    light: *mut c_void,
    rec: &mut LightRec,
) {
    unsafe {
        let l = &*(light as *const AActor);
        rec.actor = light;
        rec.location = [l.location.x, l.location.y, l.location.z];
        rec.radius = ops.world_light_radius(light);
        rec.extra_tag = l.extra_tag;
        rec.lighting_tag = l.lighting_tag;
        rec.special_tag = l.special_tag;
        rec.index = l.index;
        rec.brightness = l.light_brightness as u32;
        rec.special_lit = l.b_special_lit() as u32;
        rec.is_static = l.b_static() as u32;
        rec.is_movable = l.b_movable() as u32;
    }
}

unsafe fn write_back(light: *mut c_void, rec: &LightRec) {
    unsafe {
        let l = &mut *(light as *mut AActor);
        l.extra_tag = rec.extra_tag;
        l.lighting_tag = rec.lighting_tag;
        l.special_tag = rec.special_tag;
    }
}

unsafe fn model_light(index: i32) -> *mut c_void {
    unsafe {
        let level = (*crate::light_manager::state()).Level;
        if level.is_null() {
            return std::ptr::null_mut();
        }
        let model = (*(level as *const ULevel)).model;
        if model.is_null() {
            return std::ptr::null_mut();
        }
        let lights = std::ptr::addr_of!((*(model as *const UModel)).lights);
        let data = std::ptr::addr_of!((*lights).data).read_unaligned();
        *(data as *const *mut c_void).add(index as usize)
    }
}

fn add_candidate<O: CandidateOps>(
    ops: &mut O,
    set: &mut LightConsiderSet,
    light: *mut c_void,
    factor: u8,
) {
    let mut rec: LightRec = unsafe { std::mem::zeroed() };
    unsafe { fill_rec(ops, light, &mut rec) };
    if consider_add(set, &mut rec, factor) {
        unsafe { write_back(light, &rec) };
    }
}

/// Shared candidate collector; all engine- and platform-specific reads enter as
/// typed leaves, while ordering, visibility hysteresis, and sender recaching stay here.
pub fn collect_candidates_with<O: CandidateOps>(
    ops: &mut O,
    set: &mut LightConsiderSet,
    io: &CandidateIo,
    senders: &mut [SenderEntry],
    num: &mut i32,
) -> Result<(), O::Error> {
    ops.validate_sender_count(*num, senders.len())?;
    consider_reset(set, io.actor_location, io.actor_special_lit);
    *num = senders_compact(&mut senders[..*num as usize], |entry| ops.sender_alive(entry));
    for &entry in &senders[..*num as usize] {
        if !entry.actor.is_null() {
            add_candidate(ops, set, entry.actor, entry.factor);
        }
    }
    if io.permeating != -1 {
        let mut i = io.permeating;
        loop {
            let light = unsafe { model_light(i) };
            if light.is_null() || set.num as usize >= CONSIDER_CAP {
                break;
            }
            add_candidate(ops, set, light, 0);
            i += 1;
        }
    }
    let mut link = io.link_head;
    while !link.is_null() && (set.num as usize) < CONSIDER_CAP {
        let (actor, next) = unsafe { crate::light_manager::read_vol_actor_link(link) };
        add_candidate(ops, set, actor, 255);
        link = next;
    }
    consider_sort(&mut set.recs[..set.num as usize]);
    let input = LightVisIn {
        delta: io.delta,
        frame_count: io.frame_count,
        first_see: io.first_see,
    };
    consider_visibility(ops, set, &input, io.actor)?;
    for rec in &set.recs[..set.num as usize] {
        unsafe { write_back(rec.actor, rec) };
    }
    *num = senders_recache(set, senders);
    Ok(())
}

struct NativeCandidateOps<'a>(&'a Hp2LightServices);

impl CandidateOps for NativeCandidateOps<'_> {
    type Error = std::convert::Infallible;
    fn sender_alive(&mut self, entry: &SenderEntry) -> bool {
        unsafe { crate::light_manager::callback(|| (self.0.sender_alive)(entry.index, entry.actor)) != 0 }
    }
    fn world_light_radius(&mut self, light: *mut c_void) -> f32 {
        unsafe { crate::light_manager::callback(|| (self.0.world_light_radius)(light)) }
    }
    fn line_check(&mut self, actor: *mut c_void, light: *mut c_void) -> Result<bool, Self::Error> {
        Ok(unsafe { crate::light_manager::callback(|| (self.0.line_check)(actor, light)) != 0 })
    }
    fn add_light(&mut self, actor: *mut c_void, light: *mut c_void) -> Result<(), Self::Error> {
        unsafe { crate::light_manager::callback(|| (self.0.add_light)(actor, light)); }
        Ok(())
    }
}

pub fn collect_candidates(
    svc: &Hp2LightServices,
    set: &mut LightConsiderSet,
    io: &CandidateIo,
    senders: &mut [SenderEntry],
    num: &mut i32,
) {
    let mut ops = NativeCandidateOps(svc);
    match collect_candidates_with(&mut ops, set, io, senders, num) {
        Ok(()) => (),
        Err(never) => match never {},
    }
}

/// Candidate collector input (C++ `LightCandidateIo`, 48 bytes). `permeating` is
/// `Leaf.iPermeating` (-1 when none); `link_head` is `LeafLights`.
#[repr(C)]
pub struct CandidateIo {
    pub actor: *mut c_void,
    pub link_head: *mut c_void,
    pub actor_location: [f32; 3],
    pub actor_special_lit: u32,
    pub permeating: i32,
    pub delta: i32,
    pub frame_count: i32,
    pub first_see: u32,
}

const _: () = assert!(std::mem::size_of::<CandidateIo>() == 48);


/// Sender cache compaction (found path): drops entries whose actor is no
/// longer live. `alive` is the shell's `GetIndexedObject`/`bDeleteMe` test.
/// Returns the new count.
pub fn senders_compact(
    entries: &mut [SenderEntry],
    mut alive: impl FnMut(&SenderEntry) -> bool,
) -> i32 {
    let mut new_num = 0usize;
    for i in 0..entries.len() {
        if new_num != i {
            entries[new_num] = entries[i];
        }
        if alive(&entries[i]) {
            new_num += 1;
        }
    }
    new_num as i32
}

/// Sender cache recache: keeps up to `MAX_ACTOR_LIGHTS` candidates with
/// `special_tag > 1`. `entries` must hold `MAX_ACTOR_LIGHTS` slots.
pub fn senders_recache(set: &LightConsiderSet, entries: &mut [SenderEntry]) -> i32 {
    let mut num = 0usize;
    for rec in &set.recs[..set.num as usize] {
        if num >= MAX_ACTOR_LIGHTS {
            break;
        }
        if rec.special_tag > 1 {
            entries[num] = SenderEntry {
                actor: rec.actor,
                factor: rec.special_tag as u8,
                index: rec.index,
            };
            num += 1;
        }
    }
    num as i32
}

/// `FloatColor *= SpecialTag/255.0f`, then `*= -1` when the light is dark.
/// `color` is the four floats of `FPlane` (X, Y, Z, W); `FPlane::operator*=`
/// scales all four.
pub fn color_scale(color: &mut [f32; 4], special_tag: i32, dark: bool) {
    let s = special_tag as f32 / 255.0;
    for c in color.iter_mut() {
        *c *= s;
    }
    if dark {
        for c in color.iter_mut() {
            *c *= -1.0;
        }
    }
}

/// One light's contribution to `LightParticleSystem`:
/// `LightColor += ScaledColor * Intensity * Intensity`,
/// `Intensity = MaxSpecDir | LightDir`.
pub fn particle_accumulate(
    acc: &mut [f32; 4],
    scaled: &[f32; 4],
    max_spec: [f32; 3],
    light_dir: [f32; 3],
) {
    let intensity = dot(max_spec, light_dir);
    for k in 0..4 {
        acc[k] += scaled[k] * intensity * intensity;
    }
}

/// Setup-tail io (C++ `Hp2LightSetupTailIo`, 24 bytes). `frame` is the scene frame for
/// `ComputeFromActor`; `volumetrics` is the `FActorLink*` list head; `result` is
/// `Result` in and out (`PF_RenderFog` bit).
#[repr(C)]
pub struct SetupTailIo {
    pub frame: *mut c_void,
    pub volumetrics: *mut c_void,
    pub result: u32,
    pub pad: u32,
}

const _: () = assert!(std::mem::size_of::<SetupTailIo>() == 24);

/// `SetupForActor` tail (oracle 2575–2610): volumetric occluders join `FirstLight`
/// (`IsVolumetric` on the existing slot or a new `ALO_NotLight` slot), then every light
/// is computed and tinted, volumetric lights are collected and sorted, and the
/// `MeshLightCount` stat is added. Runs only inside the shell's gate. Outside it
/// `LastLight` stays at `FirstLight`, so the oracle's stat adds 0 there.
pub trait SetupTailOps {
    type Error;
    fn compute_light(&mut self, index: u32, frame: *mut c_void) -> Result<(), Self::Error>;
    fn stat_vtric_inc(&mut self) -> Result<(), Self::Error>;
    fn stat_mesh_light_add(&mut self, count: u32) -> Result<(), Self::Error>;
}

pub fn setup_tail_with_ops<O: SetupTailOps>(
    ops: &mut O,
    io: &mut SetupTailIo,
) -> Result<(), O::Error> {
    let mut link = io.volumetrics;
    while !link.is_null() && unsafe { crate::light_manager::light_has_room() } {
        io.result |= crate::light::PF_RENDER_FOG;
        let (actor, next) = unsafe { crate::light_manager::read_actor_link(link) };
        let count = unsafe { crate::light_manager::light_count() } as u32;
        let mut found = count;
        for i in 0..count {
            if unsafe { crate::light_manager::light_actor_at(i) } == actor {
                found = i;
                break;
            }
        }
        unsafe { crate::light_manager::light_set_volumetric(found) };
        if found == count {
            unsafe { crate::light_manager::light_append(actor, ALO_NOT_LIGHT) };
            ops.stat_vtric_inc()?;
        }
        link = next;
    }
    let count = unsafe { crate::light_manager::light_count() } as u32;
    unsafe { crate::light_manager::vtric_reset() };
    for i in 0..count {
        ops.compute_light(i, io.frame)?;
        let actor = unsafe { &*(crate::light_manager::light_actor_at(i) as *const AActor) };
        let special = actor.special_tag;
        let dark = actor.b_dark_light();
        let color = unsafe { &mut *crate::light_manager::light_float_color(i).cast::<[f32; 4]>() };
        color_scale(color, special, dark);
        if unsafe { crate::light_manager::light_is_volumetric(i) } {
            unsafe { crate::light_manager::vtric_push(i) };
        }
    }
    let vtrics = unsafe { crate::light_manager::vtric_count() };
    if vtrics != 0 { unsafe { crate::light_manager::vtric_sort(vtrics) }; }
    ops.stat_mesh_light_add(count)?;
    Ok(())
}

struct NativeSetupTailOps<'a>(&'a Hp2LightServices);

impl SetupTailOps for NativeSetupTailOps<'_> {
    type Error = std::convert::Infallible;
    fn compute_light(&mut self, index: u32, frame: *mut c_void) -> Result<(), Self::Error> {
        unsafe { crate::light_manager::callback(|| (self.0.light_compute)(index, frame)); }
        Ok(())
    }
    fn stat_vtric_inc(&mut self) -> Result<(), Self::Error> {
        unsafe { crate::light_manager::callback(|| (self.0.stat_vtric_inc)()); }
        Ok(())
    }
    fn stat_mesh_light_add(&mut self, count: u32) -> Result<(), Self::Error> {
        unsafe { crate::light_manager::callback(|| (self.0.stat_mesh_light_add)(count)); }
        Ok(())
    }
}

pub fn setup_tail(svc: &Hp2LightServices, io: &mut SetupTailIo) {
    let mut ops = NativeSetupTailOps(svc);
    match setup_tail_with_ops(&mut ops, io) {
        Ok(()) => (),
        Err(never) => match never {},
    }
}

pub trait FinishActorOps {
    type Error;
    fn mark_pop(&mut self) -> Result<(), Self::Error>;
    fn unlock_one(&mut self) -> Result<(), Self::Error>;
}

pub fn finish_actor_with_ops<O: FinishActorOps>(ops: &mut O) -> Result<(), O::Error> {
    ops.mark_pop()?;
    while unsafe { crate::light_manager::unlock_pending() } {
        ops.unlock_one()?;
    }
    Ok(())
}

struct NativeFinishActorOps<'a>(&'a Hp2LightServices);

impl FinishActorOps for NativeFinishActorOps<'_> {
    type Error = std::convert::Infallible;
    fn mark_pop(&mut self) -> Result<(), Self::Error> {
        unsafe { crate::light_manager::callback(|| (self.0.mark_pop)()); }
        Ok(())
    }
    fn unlock_one(&mut self) -> Result<(), Self::Error> {
        unsafe { crate::light_manager::callback(|| (self.0.unlock_one)()); }
        Ok(())
    }
}

pub fn finish_actor(svc: &Hp2LightServices) {
    let mut ops = NativeFinishActorOps(svc);
    match finish_actor_with_ops(&mut ops) {
        Ok(()) => (),
        Err(never) => match never {},
    }
}
