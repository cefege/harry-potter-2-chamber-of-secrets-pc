//! Checked replication bodies from `UnActor.cpp`.
//!
//! Native objects and Recent remain raw across callbacks. Only named fields are
//! read, never object images or padding. The native adapter owns each DOREP site's
//! lazy UProperty cache, publishing a successful lookup before reading RepIndex.

use std::ffi::{c_char, c_void};
use std::ptr;

use hp2_abi::engine::generated::{
    AActor, AAmmo, AGameReplicationInfo, AInventory, ALevelInfo, AMover, APawn, APickup,
    APlayerPawn, APlayerReplicationInfo, AWeapon, AZoneInfo, FRotator,
};
use hp2_abi::{FPlane, FVector};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2ActorRepServices {
    pub static_class_flags: unsafe extern "C" fn(kind: u32, out: *mut u32) -> i32,
    pub find_rep_index:
        unsafe extern "C" fn(kind: u32, name: *const c_char, site: u32, out: *mut u32) -> i32,
    pub can_serialize_object:
        unsafe extern "C" fn(map: *mut c_void, obj: *mut c_void, out: *mut i32) -> i32,
    pub fstring_neq: unsafe extern "C" fn(a: *const c_void, b: *const c_void, out: *mut i32) -> i32,
    pub object_is_a: unsafe extern "C" fn(obj: *const c_void, kind: u32, out: *mut i32) -> i32,
    pub default_actor_of:
        unsafe extern "C" fn(actor: *const c_void, out: *mut *const c_void) -> i32,
    pub base_rep_list: unsafe extern "C" fn(
        kind: u32,
        actor: *const c_void,
        recent: *const c_void,
        map: *mut c_void,
        cursor: *mut i32,
        num_reps: i32,
        out: *mut *mut i32,
    ) -> i32,
}

pub(crate) type RepResult<T> = Result<T, i32>;

fn checked(status: i32) -> RepResult<()> {
    if status == 0 { Ok(()) } else { Err(status) }
}

const KIND_ACTOR: u32 = 0;
const KIND_PAWN: u32 = 1;
const KIND_PLAYER_PAWN: u32 = 2;
const KIND_MOVER: u32 = 3;
const KIND_ZONE_INFO: u32 = 4;
const KIND_LEVEL_INFO: u32 = 5;
const KIND_PLAYER_REPLICATION_INFO: u32 = 6;
const KIND_GAME_REPLICATION_INFO: u32 = 7;
const KIND_INVENTORY: u32 = 8;
const KIND_WEAPON: u32 = 9;
const KIND_AMMO: u32 = 10;
const KIND_PICKUP: u32 = 11;

const CLASS_NATIVE_REPLICATION: u32 = 0x0000_0800;
const LT_NONE: u8 = 0;
const DT_SPRITE: u8 = 1;
const DT_MESH: u8 = 2;
const DT_BRUSH: u8 = 3;
const DT_PARTICLES: u8 = 8;
const ROLE_SIMULATED_PROXY: u8 = 2;
const ROLE_AUTHORITY: u8 = 4;
const PHYS_ROTATING: u8 = 5;

/// Leaf operations consumed by the shared actor replication family.
///
/// Native service tables and the canonical Core backend both implement this,
/// so the original optimized rep-list bodies remain the only policy.
pub(crate) trait RepBackend {
    fn static_class_flags(&self, kind: u32) -> RepResult<u32>;
    fn find_rep_index(&self, kind: u32, name: &str, site: u32) -> RepResult<i32>;
    fn can_serialize_object(&self, map: *mut c_void, obj: *mut c_void) -> RepResult<bool>;
    fn fstring_neq(&self, a: *const c_void, b: *const c_void) -> RepResult<bool>;
    fn base_rep_list(
        &self,
        kind: u32,
        actor: *const c_void,
        recent: *const c_void,
        map: *mut c_void,
        cursor: *mut i32,
        num_reps: i32,
    ) -> RepResult<*mut i32>;
}

impl RepBackend for Hp2ActorRepServices {
    fn static_class_flags(&self, kind: u32) -> RepResult<u32> {
        let mut value = 0;
        checked(unsafe { (self.static_class_flags)(kind, &mut value) })?;
        Ok(value)
    }
    fn find_rep_index(&self, kind: u32, name: &str, site: u32) -> RepResult<i32> {
        let mut value = 0;
        checked(unsafe { (self.find_rep_index)(kind, name.as_ptr().cast(), site, &mut value) })?;
        Ok(value as i32)
    }
    fn can_serialize_object(&self, map: *mut c_void, obj: *mut c_void) -> RepResult<bool> {
        let mut value = 0;
        checked(unsafe { (self.can_serialize_object)(map, obj, &mut value) })?;
        Ok(value != 0)
    }
    fn fstring_neq(&self, a: *const c_void, b: *const c_void) -> RepResult<bool> {
        let mut value = 0;
        checked(unsafe { (self.fstring_neq)(a, b, &mut value) })?;
        Ok(value != 0)
    }
    fn base_rep_list(
        &self,
        kind: u32,
        actor: *const c_void,
        recent: *const c_void,
        map: *mut c_void,
        cursor: *mut i32,
        num_reps: i32,
    ) -> RepResult<*mut i32> {
        let mut next = cursor;
        checked(unsafe {
            (self.base_rep_list)(kind, actor, recent, map, cursor, num_reps, &mut next)
        })?;
        Ok(next)
    }
}

/// Invocation-local cursor. No native object, cache lock, or slice is borrowed.
pub struct Rep<'a> {
    svc: &'a dyn RepBackend,
    map: *mut c_void,
    out: *mut i32,
}

impl<'a> Rep<'a> {
    /// # Safety
    /// Map is live; out has room for all indices selected by the native contract.
    pub unsafe fn new(svc: &'a Hp2ActorRepServices, map: *mut c_void, out: *mut i32) -> Self {
        Self { svc, map, out }
    }

    /// Builds a cursor over an already-generalized backend.
    ///
    /// # Safety
    /// Map is live; out has room for all indices selected by the backend contract.
    pub(crate) unsafe fn new_with_backend(svc: &'a dyn RepBackend, map: *mut c_void, out: *mut i32) -> Self {
        Self { svc, map, out }
    }

    fn push(&mut self, index: u32) {
        unsafe {
            self.out.write(index as i32);
            self.out = self.out.add(1);
        }
    }

    pub fn finish(self) -> *mut i32 {
        self.out
    }

    fn index(&self, kind: u32, name: &'static str, site: u32) -> RepResult<u32> {
        Ok(self.svc.find_rep_index(kind, name, site)? as u32)
    }

    fn base(
        &mut self,
        kind: u32,
        actor: *const c_void,
        recent: *const c_void,
        num_reps: i32,
    ) -> RepResult<()> {
        self.out = self.svc.base_rep_list(kind, actor, recent, self.map, self.out, num_reps)?;
        Ok(())
    }

    fn string_neq(&self, a: *const c_void, b: *const c_void) -> RepResult<bool> {
        self.svc.fstring_neq(a, b)
    }
}

fn is_native(rep: &Rep<'_>, kind: u32) -> RepResult<bool> {
    Ok(rep.svc.static_class_flags(kind)? & CLASS_NATIVE_REPLICATION != 0)
}

fn min_f32(a: f32, b: f32) -> f32 {
    if a <= b { a } else { b }
}

trait Neq: Copy {
    fn neq(self, other: Self, rep: &Rep<'_>) -> RepResult<bool>;
}

macro_rules! scalar_neq {
    ($($ty:ty),*) => { $(
        impl Neq for $ty {
            fn neq(self, other: Self, _rep: &Rep<'_>) -> RepResult<bool> { Ok(self != other) }
        }
    )* };
}
scalar_neq!(u8, i32, bool, [u8; 4]);

impl Neq for f32 {
    fn neq(self, other: Self, _rep: &Rep<'_>) -> RepResult<bool> {
        Ok(self.to_bits() != other.to_bits())
    }
}
impl Neq for FVector {
    fn neq(self, other: Self, _rep: &Rep<'_>) -> RepResult<bool> {
        Ok(self.x.to_bits() != other.x.to_bits()
            || self.y.to_bits() != other.y.to_bits()
            || self.z.to_bits() != other.z.to_bits())
    }
}
impl Neq for FPlane {
    fn neq(self, other: Self, _rep: &Rep<'_>) -> RepResult<bool> {
        Ok(self.x.to_bits() != other.x.to_bits()
            || self.y.to_bits() != other.y.to_bits()
            || self.z.to_bits() != other.z.to_bits()
            || self.w.to_bits() != other.w.to_bits())
    }
}
impl Neq for FRotator {
    fn neq(self, other: Self, _rep: &Rep<'_>) -> RepResult<bool> {
        Ok(self.pitch != other.pitch || self.yaw != other.yaw || self.roll != other.roll)
    }
}
impl Neq for *mut c_void {
    fn neq(self, other: Self, rep: &Rep<'_>) -> RepResult<bool> {
        let value = rep.svc.can_serialize_object(rep.map, self)?;
        Ok((if value { self } else { ptr::null_mut() }) != other)
    }
}

// Scalar comparisons precede lazy lookup. A failed lookup leaves the native
// cache slot uninitialized and all previously emitted indices intact.
macro_rules! dorep {
    ($rep:ident, $site:literal, $kind:expr, $name:literal, $a:expr, $b:expr $(,)?) => {{
        if Neq::neq($a, $b, $rep)? {
            let index = $rep.index($kind, concat!($name, "\0"), $site)?;
            $rep.push(index);
        }
    }};
}
macro_rules! dorep_string {
    ($rep:ident, $site:literal, $kind:expr, $name:literal, $a:expr, $b:expr $(,)?) => {{
        if $rep.string_neq(ptr::addr_of!($a).cast(), ptr::addr_of!($b).cast())? {
            let index = $rep.index($kind, concat!($name, "\0"), $site)?;
            $rep.push(index);
        }
    }};
}
macro_rules! dorep_array {
    ($rep:ident, $site:literal, $kind:expr, $name:literal, $a:expr, $b:expr $(,)?) => {{
        // Unlike DOREP, DOREPARRAY resolves the property before any comparison.
        // Keep raw array addresses; callbacks may mutate subsequent elements.
        let _ = $rep.index($kind, concat!($name, "\0"), $site)?;
        let va = ptr::addr_of!($a).cast::<*mut c_void>();
        let vb = ptr::addr_of!($b).cast::<*mut c_void>();
        for i in 0..8 {
            if Neq::neq(va.add(i).read_unaligned(), vb.add(i).read_unaligned(), $rep)? {
                let base = $rep.index($kind, concat!($name, "\0"), $site)?;
                $rep.push(base + i as u32);
            }
        }
    }};
}

/// AActor::GetOptimizedRepList.
pub unsafe fn actor_get_optimized_rep_list(
    rep: &mut Rep<'_>,
    this: *const AActor,
    recent: *const AActor,
) -> RepResult<()> {
    unsafe {
        if !is_native(rep, KIND_ACTOR)? || (*this).role != ROLE_AUTHORITY {
            return Ok(());
        }
        dorep!(rep, 0, KIND_ACTOR, "Owner", (*this).owner, (*recent).owner);
        dorep!(rep, 1, KIND_ACTOR, "Role", (*this).role, (*recent).role);
        dorep!(
            rep,
            2,
            KIND_ACTOR,
            "RemoteRole",
            (*this).remote_role,
            (*recent).remote_role
        );
        dorep!(
            rep,
            3,
            KIND_ACTOR,
            "bCollideActors",
            (*this).b_collide_actors(),
            (*recent).b_collide_actors()
        );
        dorep!(
            rep,
            4,
            KIND_ACTOR,
            "bCollideWorld",
            (*this).b_collide_world(),
            (*recent).b_collide_world()
        );
        dorep!(
            rep,
            5,
            KIND_ACTOR,
            "LightType",
            (*this).light_type,
            (*recent).light_type
        );
        dorep!(
            rep,
            6,
            KIND_ACTOR,
            "bHidden",
            (*this).b_hidden(),
            (*recent).b_hidden()
        );
        dorep!(
            rep,
            7,
            KIND_ACTOR,
            "bOnlyOwnerSee",
            (*this).b_only_owner_see(),
            (*recent).b_only_owner_see()
        );
        dorep!(
            rep,
            8,
            KIND_ACTOR,
            "Texture",
            (*this).texture,
            (*recent).texture
        );
        dorep!(
            rep,
            9,
            KIND_ACTOR,
            "DrawScale",
            (*this).draw_scale,
            (*recent).draw_scale
        );
        dorep!(
            rep,
            10,
            KIND_ACTOR,
            "DrawType",
            (*this).draw_type,
            (*recent).draw_type
        );
        dorep!(rep, 11, KIND_ACTOR, "Style", (*this).style, (*recent).style);
        if (*this).b_net_owner() {
            dorep!(
                rep,
                12,
                KIND_ACTOR,
                "bNetOwner",
                (*this).b_net_owner(),
                (*recent).b_net_owner()
            );
            dorep!(
                rep,
                13,
                KIND_ACTOR,
                "Inventory",
                (*this).inventory,
                (*recent).inventory
            );
        }
        if (*this).b_replicate_instigator() && (*this).remote_role >= ROLE_SIMULATED_PROXY {
            dorep!(
                rep,
                14,
                KIND_ACTOR,
                "Instigator",
                (*this).instigator,
                (*recent).instigator
            );
        }
        if !(*this).b_net_owner() || !(*this).b_client_anim() {
            dorep!(
                rep,
                15,
                KIND_ACTOR,
                "AmbientSound",
                (*this).ambient_sound,
                (*recent).ambient_sound
            );
            if !(*this).ambient_sound.is_null() {
                dorep!(
                    rep,
                    16,
                    KIND_ACTOR,
                    "SoundRadius",
                    (*this).sound_radius,
                    (*recent).sound_radius
                );
                dorep!(
                    rep,
                    17,
                    KIND_ACTOR,
                    "SoundVolume",
                    (*this).sound_volume,
                    (*recent).sound_volume
                );
                dorep!(
                    rep,
                    18,
                    KIND_ACTOR,
                    "SoundPitch",
                    (*this).sound_pitch,
                    (*recent).sound_pitch
                );
                dorep!(
                    rep,
                    19,
                    KIND_ACTOR,
                    "SoundOcclusion",
                    (*this).sound_occlusion,
                    (*recent).sound_occlusion
                );
            }
        }
        if (*this).b_collide_actors() || (*this).b_collide_world() {
            dorep!(
                rep,
                20,
                KIND_ACTOR,
                "bProjTarget",
                (*this).b_proj_target(),
                (*recent).b_proj_target()
            );
            dorep!(
                rep,
                21,
                KIND_ACTOR,
                "bBlockActors",
                (*this).b_block_actors(),
                (*recent).b_block_actors()
            );
            dorep!(
                rep,
                22,
                KIND_ACTOR,
                "bBlockPlayers",
                (*this).b_block_players(),
                (*recent).b_block_players()
            );
            dorep!(
                rep,
                23,
                KIND_ACTOR,
                "CollisionRadius",
                (*this).collision_radius,
                (*recent).collision_radius
            );
            dorep!(
                rep,
                24,
                KIND_ACTOR,
                "CollisionHeight",
                (*this).collision_height,
                (*recent).collision_height
            );
        }
        if !(*this).b_carried_item()
            && ((*this).b_net_initial()
                || (*this).b_simulated_pawn()
                || (*this).remote_role < ROLE_SIMULATED_PROXY)
        {
            dorep!(
                rep,
                25,
                KIND_ACTOR,
                "Location",
                (*this).location,
                (*recent).location
            );
            if (*this).draw_type == DT_MESH
                || (*this).draw_type == DT_BRUSH
                || (*this).draw_type == DT_PARTICLES
            {
                dorep!(
                    rep,
                    26,
                    KIND_ACTOR,
                    "Rotation",
                    (*this).rotation,
                    (*recent).rotation
                );
            }
        }
        if (*this).draw_type == DT_MESH {
            dorep!(
                rep,
                27,
                KIND_ACTOR,
                "AmbientGlow",
                (*this).ambient_glow,
                (*recent).ambient_glow
            );
            dorep!(
                rep,
                28,
                KIND_ACTOR,
                "ScaleGlow",
                (*this).scale_glow,
                (*recent).scale_glow
            );
            dorep!(
                rep,
                29,
                KIND_ACTOR,
                "bUnlit",
                (*this).b_unlit(),
                (*recent).b_unlit()
            );
            dorep!(
                rep,
                30,
                KIND_ACTOR,
                "Fatness",
                (*this).fatness,
                (*recent).fatness
            );
            dorep!(
                rep,
                31,
                KIND_ACTOR,
                "Wideness",
                (*this).wideness,
                (*recent).wideness
            );
            dorep!(
                rep,
                32,
                KIND_ACTOR,
                "PrePivot",
                (*this).pre_pivot,
                (*recent).pre_pivot
            );
            dorep!(rep, 33, KIND_ACTOR, "Mesh", (*this).mesh, (*recent).mesh);
            dorep!(
                rep,
                34,
                KIND_ACTOR,
                "bMeshEnviroMap",
                (*this).b_mesh_enviro_map(),
                (*recent).b_mesh_enviro_map()
            );
            dorep!(rep, 35, KIND_ACTOR, "Skin", (*this).skin, (*recent).skin);
            dorep_array!(
                rep,
                36,
                KIND_ACTOR,
                "MultiSkins",
                (*this).multi_skins,
                (*recent).multi_skins
            );
            if (((*this).remote_role <= ROLE_SIMULATED_PROXY)
                && (!(*this).b_net_owner() || !(*this).b_client_anim()))
                || (*this).b_demo_recording()
            {
                dorep!(
                    rep,
                    37,
                    KIND_ACTOR,
                    "AnimSequence",
                    (*this).anim_sequence,
                    (*recent).anim_sequence
                );
                dorep!(
                    rep,
                    38,
                    KIND_ACTOR,
                    "SimAnim",
                    (*this).sim_anim,
                    (*recent).sim_anim
                );
                dorep!(
                    rep,
                    39,
                    KIND_ACTOR,
                    "AnimMinRate",
                    (*this).anim_min_rate,
                    (*recent).anim_min_rate
                );
                dorep!(
                    rep,
                    40,
                    KIND_ACTOR,
                    "bAnimNotify",
                    (*this).b_anim_notify(),
                    (*recent).b_anim_notify()
                );
            }
        } else if (*this).draw_type == DT_SPRITE {
            if !(*this).b_hidden() && (!(*this).b_only_owner_see() || (*this).b_net_owner()) {
                dorep!(
                    rep,
                    41,
                    KIND_ACTOR,
                    "Sprite",
                    (*this).sprite,
                    (*recent).sprite
                );
            }
        } else if (*this).draw_type == DT_BRUSH {
            dorep!(
                rep,
                42,
                KIND_ACTOR,
                "PrePivot",
                (*this).pre_pivot,
                (*recent).pre_pivot
            );
            dorep!(rep, 43, KIND_ACTOR, "Brush", (*this).brush, (*recent).brush);
        }
        if (*this).light_type != LT_NONE {
            dorep!(
                rep,
                44,
                KIND_ACTOR,
                "LightEffect",
                (*this).light_effect,
                (*recent).light_effect
            );
            dorep!(
                rep,
                45,
                KIND_ACTOR,
                "LightBrightness",
                (*this).light_brightness,
                (*recent).light_brightness
            );
            dorep!(
                rep,
                46,
                KIND_ACTOR,
                "LightHue",
                (*this).light_hue,
                (*recent).light_hue
            );
            dorep!(
                rep,
                47,
                KIND_ACTOR,
                "LightSaturation",
                (*this).light_saturation,
                (*recent).light_saturation
            );
            dorep!(
                rep,
                48,
                KIND_ACTOR,
                "bDarkLight",
                (*this).b_dark_light(),
                (*recent).b_dark_light()
            );
            dorep!(
                rep,
                49,
                KIND_ACTOR,
                "LightRadius",
                (*this).light_radius,
                (*recent).light_radius
            );
            dorep!(
                rep,
                50,
                KIND_ACTOR,
                "LightRadiusInner",
                (*this).light_radius_inner,
                (*recent).light_radius_inner
            );
            dorep!(
                rep,
                51,
                KIND_ACTOR,
                "LightPeriod",
                (*this).light_period,
                (*recent).light_period
            );
            dorep!(
                rep,
                52,
                KIND_ACTOR,
                "LightPhase",
                (*this).light_phase,
                (*recent).light_phase
            );
            dorep!(
                rep,
                53,
                KIND_ACTOR,
                "VolumeBrightness",
                (*this).volume_brightness,
                (*recent).volume_brightness
            );
            dorep!(
                rep,
                54,
                KIND_ACTOR,
                "VolumeRadius",
                (*this).volume_radius,
                (*recent).volume_radius
            );
            dorep!(
                rep,
                55,
                KIND_ACTOR,
                "bSpecialLit",
                (*this).b_special_lit(),
                (*recent).b_special_lit()
            );
        }
        if (*this).remote_role == ROLE_SIMULATED_PROXY {
            dorep!(rep, 56, KIND_ACTOR, "Base", (*this).base, (*recent).base);
            if (*this).b_net_initial() {
                if !(*this).b_simulated_pawn() {
                    dorep!(
                        rep,
                        57,
                        KIND_ACTOR,
                        "Physics",
                        (*this).physics,
                        (*recent).physics
                    );
                    dorep!(
                        rep,
                        58,
                        KIND_ACTOR,
                        "Acceleration",
                        (*this).acceleration,
                        (*recent).acceleration
                    );
                    dorep!(
                        rep,
                        59,
                        KIND_ACTOR,
                        "bBounce",
                        (*this).b_bounce(),
                        (*recent).b_bounce()
                    );
                }
                if (*this).physics == PHYS_ROTATING {
                    dorep!(
                        rep,
                        60,
                        KIND_ACTOR,
                        "bFixedRotationDir",
                        (*this).b_fixed_rotation_dir(),
                        (*recent).b_fixed_rotation_dir()
                    );
                    dorep!(
                        rep,
                        61,
                        KIND_ACTOR,
                        "bRotateToDesired",
                        (*this).b_rotate_to_desired(),
                        (*recent).b_rotate_to_desired()
                    );
                    dorep!(
                        rep,
                        62,
                        KIND_ACTOR,
                        "RotationRate",
                        (*this).rotation_rate,
                        (*recent).rotation_rate
                    );
                    dorep!(
                        rep,
                        63,
                        KIND_ACTOR,
                        "DesiredRotation",
                        (*this).desired_rotation,
                        (*recent).desired_rotation
                    );
                }
            }
        } else if (*this).b_sim_fall() {
            dorep!(
                rep,
                64,
                KIND_ACTOR,
                "Physics",
                (*this).physics,
                (*recent).physics
            );
            dorep!(
                rep,
                65,
                KIND_ACTOR,
                "Acceleration",
                (*this).acceleration,
                (*recent).acceleration
            );
            dorep!(
                rep,
                66,
                KIND_ACTOR,
                "bBounce",
                (*this).b_bounce(),
                (*recent).b_bounce()
            );
        }
        if (*this).b_sim_fall()
            || (*this).b_is_mover()
            || ((*this).remote_role == ROLE_SIMULATED_PROXY
                && ((*this).b_net_initial() || (*this).b_simulated_pawn()))
        {
            dorep!(
                rep,
                67,
                KIND_ACTOR,
                "Velocity",
                (*this).velocity,
                (*recent).velocity
            );
        }
        Ok(())
    }
}

/// AActor::UpdateFrequency.
pub unsafe fn actor_update_frequency_native(
    svc: &Hp2ActorRepServices,
    this: *const AActor,
    view_dir: *const FVector,
    view_pos: *const FVector,
) -> RepResult<f32> {
    unsafe {
        if (*this).b_net_temporary() && !(*this).b_net_optional() {
            // SAFETY: the service returns the class default object, which the C++ checks
            // before returning (UnClass.h GetDefaultActor).
            let mut default_actor = ptr::null();
            checked((svc.default_actor_of)(this.cast(), &mut default_actor))?;
            let default_life_span = (*default_actor.cast::<AActor>()).life_span;
            if (*this).life_span < default_life_span - 0.2 {
                let loc = (*this).location;
                let d = FVector {
                    x: loc.x - (*view_pos).x,
                    y: loc.y - (*view_pos).y,
                    z: loc.z - (*view_pos).z,
                };
                // Fused as the oracle emits (fmadd chains, see FMA table in notes).
                let size_squared = d.z.mul_add(d.z, d.x.mul_add(d.x, d.y * d.y));
                let dot = (*view_dir)
                    .z
                    .mul_add(d.z, (*view_dir).x.mul_add(d.x, (*view_dir).y * d.y));
                if size_squared > 1_000_000.0 || dot < 0.0 {
                    return Ok(min_f32((*this).net_update_frequency, 8.0));
                }
            }
        }
        Ok((*this).net_update_frequency)
    }
}

/// APawn::GetOptimizedRepList.
pub unsafe fn pawn_get_optimized_rep_list(
    rep: &mut Rep<'_>,
    this: *const APawn,
    recent: *const APawn,
    num_reps: i32,
) -> RepResult<()> {
    unsafe {
        if is_native(rep, KIND_PAWN)? {
            if (*this).base.b_net_owner() || (*this).base.b_net_initial() || num_reps % 5 == 0 {
                rep.base(KIND_ACTOR, this.cast(), recent.cast(), num_reps)?;
                if (*this).base.role == ROLE_AUTHORITY {
                    dorep!(
                        rep,
                        68,
                        KIND_PAWN,
                        "PlayerReplicationInfo",
                        (*this).player_replication_info,
                        (*recent).player_replication_info
                    );
                    dorep!(
                        rep,
                        69,
                        KIND_PAWN,
                        "Health",
                        (*this).health,
                        (*recent).health
                    );
                    if (*this).base.b_net_owner() {
                        dorep!(
                            rep,
                            70,
                            KIND_PAWN,
                            "bIsPlayer",
                            (*this).b_is_player(),
                            (*recent).b_is_player()
                        );
                        dorep!(
                            rep,
                            71,
                            KIND_PAWN,
                            "carriedDecoration",
                            (*this).carried_decoration,
                            (*recent).carried_decoration
                        );
                        dorep!(
                            rep,
                            72,
                            KIND_PAWN,
                            "MoveTarget",
                            (*this).move_target,
                            (*recent).move_target
                        );
                        dorep!(
                            rep,
                            73,
                            KIND_PAWN,
                            "SelectedItem",
                            (*this).selected_item,
                            (*recent).selected_item
                        );
                        dorep!(
                            rep,
                            74,
                            KIND_PAWN,
                            "GroundSpeed",
                            (*this).ground_speed,
                            (*recent).ground_speed
                        );
                        dorep!(
                            rep,
                            75,
                            KIND_PAWN,
                            "WaterSpeed",
                            (*this).water_speed,
                            (*recent).water_speed
                        );
                        dorep!(
                            rep,
                            76,
                            KIND_PAWN,
                            "AirSpeed",
                            (*this).air_speed,
                            (*recent).air_speed
                        );
                        dorep!(
                            rep,
                            77,
                            KIND_PAWN,
                            "AccelRate",
                            (*this).accel_rate,
                            (*recent).accel_rate
                        );
                        dorep!(
                            rep,
                            78,
                            KIND_PAWN,
                            "JumpZ",
                            (*this).jump_z,
                            (*recent).jump_z
                        );
                        dorep!(
                            rep,
                            79,
                            KIND_PAWN,
                            "AirControl",
                            (*this).air_control,
                            (*recent).air_control
                        );
                        dorep!(
                            rep,
                            80,
                            KIND_PAWN,
                            "bBehindView",
                            (*this).b_behind_view(),
                            (*recent).b_behind_view()
                        );
                    }
                }
            } else if (*this).base.role == ROLE_AUTHORITY {
                dorep!(
                    rep,
                    81,
                    KIND_ACTOR,
                    "bHidden",
                    (*this).base.b_hidden(),
                    (*recent).base.b_hidden()
                );
                if (*this).base.remote_role <= ROLE_SIMULATED_PROXY {
                    dorep!(
                        rep,
                        82,
                        KIND_ACTOR,
                        "Location",
                        (*this).base.location,
                        (*recent).base.location
                    );
                    if (*this).base.draw_type == DT_MESH || (*this).base.draw_type == DT_BRUSH {
                        dorep!(
                            rep,
                            83,
                            KIND_ACTOR,
                            "Rotation",
                            (*this).base.rotation,
                            (*recent).base.rotation
                        );
                    }
                }
                if (*this).base.draw_type == DT_MESH {
                    if (*this).base.remote_role <= ROLE_SIMULATED_PROXY
                        || (*this).base.b_demo_recording()
                    {
                        dorep!(
                            rep,
                            84,
                            KIND_ACTOR,
                            "AnimSequence",
                            (*this).base.anim_sequence,
                            (*recent).base.anim_sequence
                        );
                        dorep!(
                            rep,
                            85,
                            KIND_ACTOR,
                            "SimAnim",
                            (*this).base.sim_anim,
                            (*recent).base.sim_anim
                        );
                        dorep!(
                            rep,
                            86,
                            KIND_ACTOR,
                            "AnimMinRate",
                            (*this).base.anim_min_rate,
                            (*recent).base.anim_min_rate
                        );
                        dorep!(
                            rep,
                            87,
                            KIND_ACTOR,
                            "bAnimNotify",
                            (*this).base.b_anim_notify(),
                            (*recent).base.b_anim_notify()
                        );
                    }
                } else if (*this).base.draw_type == DT_SPRITE
                    && !(*this).base.b_hidden()
                    && !(*this).base.b_only_owner_see()
                {
                    dorep!(
                        rep,
                        88,
                        KIND_ACTOR,
                        "Sprite",
                        (*this).base.sprite,
                        (*recent).base.sprite
                    );
                }
                if (*this).base.remote_role == ROLE_SIMULATED_PROXY {
                    dorep!(
                        rep,
                        89,
                        KIND_ACTOR,
                        "Base",
                        (*this).base.base,
                        (*recent).base.base
                    );
                    dorep!(
                        rep,
                        90,
                        KIND_ACTOR,
                        "Velocity",
                        (*this).base.velocity,
                        (*recent).base.velocity
                    );
                } else if (*this).base.b_sim_fall() {
                    dorep!(
                        rep,
                        91,
                        KIND_ACTOR,
                        "Physics",
                        (*this).base.physics,
                        (*recent).base.physics
                    );
                    dorep!(
                        rep,
                        92,
                        KIND_ACTOR,
                        "Acceleration",
                        (*this).base.acceleration,
                        (*recent).base.acceleration
                    );
                    dorep!(
                        rep,
                        93,
                        KIND_ACTOR,
                        "bBounce",
                        (*this).base.b_bounce(),
                        (*recent).base.b_bounce()
                    );
                    dorep!(
                        rep,
                        94,
                        KIND_ACTOR,
                        "Velocity",
                        (*this).base.velocity,
                        (*recent).base.velocity
                    );
                }
            }
            if (*this).base.role == ROLE_AUTHORITY {
                dorep!(
                    rep,
                    95,
                    KIND_PAWN,
                    "Weapon",
                    (*this).weapon,
                    (*recent).weapon
                );
                dorep!(
                    rep,
                    96,
                    KIND_PAWN,
                    "bCanFly",
                    (*this).b_can_fly(),
                    (*recent).b_can_fly()
                );
            }
            if ((*this).base.b_net_initial() && (*this).base.b_net_owner() && (*this).b_is_player())
                || (*this).base.b_demo_recording()
            {
                dorep!(
                    rep,
                    97,
                    KIND_PAWN,
                    "ViewRotation",
                    (*this).view_rotation,
                    (*recent).view_rotation
                );
            }
            if (*this).base.b_demo_recording() {
                dorep!(
                    rep,
                    98,
                    KIND_PAWN,
                    "EyeHeight",
                    (*this).eye_height,
                    (*recent).eye_height
                );
            }
        } else {
            rep.base(KIND_ACTOR, this.cast(), recent.cast(), num_reps)?;
        }
        Ok(())
    }
}

/// APlayerPawn::GetOptimizedRepList.
pub unsafe fn player_pawn_get_optimized_rep_list(
    rep: &mut Rep<'_>,
    this: *const APlayerPawn,
    recent: *const APlayerPawn,
    num_reps: i32,
) -> RepResult<()> {
    unsafe {
        rep.base(KIND_PAWN, this.cast(), recent.cast(), num_reps)?;
        if !is_native(rep, KIND_PLAYER_PAWN)? {
            return Ok(());
        }
        if (*this).base.base.role == ROLE_AUTHORITY {
            if (*this).base.base.b_net_owner() {
                dorep!(
                    rep,
                    99,
                    KIND_PLAYER_PAWN,
                    "ViewTarget",
                    (*this).view_target,
                    (*recent).view_target
                );
                dorep!(
                    rep,
                    100,
                    KIND_PLAYER_PAWN,
                    "ScoringType",
                    (*this).scoring_type,
                    (*recent).scoring_type
                );
                dorep!(
                    rep,
                    101,
                    KIND_PLAYER_PAWN,
                    "HUDType",
                    (*this).hud_type,
                    (*recent).hud_type
                );
                dorep!(
                    rep,
                    102,
                    KIND_PLAYER_PAWN,
                    "GameReplicationInfo",
                    (*this).game_replication_info,
                    (*recent).game_replication_info
                );
                dorep!(
                    rep,
                    103,
                    KIND_PLAYER_PAWN,
                    "bFixedCamera",
                    (*this).b_fixed_camera(),
                    (*recent).b_fixed_camera()
                );
                dorep!(
                    rep,
                    104,
                    KIND_PLAYER_PAWN,
                    "bNeverAutoSwitch",
                    (*this).b_never_auto_switch(),
                    (*recent).b_never_auto_switch()
                );
                dorep!(
                    rep,
                    105,
                    KIND_PLAYER_PAWN,
                    "bCheatsEnabled",
                    (*this).b_cheats_enabled(),
                    (*recent).b_cheats_enabled()
                );
                dorep!(
                    rep,
                    106,
                    KIND_PLAYER_PAWN,
                    "TargetViewRotation",
                    (*this).target_view_rotation,
                    (*recent).target_view_rotation
                );
                dorep!(
                    rep,
                    107,
                    KIND_PLAYER_PAWN,
                    "TargetEyeHeight",
                    (*this).target_eye_height,
                    (*recent).target_eye_height
                );
                dorep!(
                    rep,
                    108,
                    KIND_PLAYER_PAWN,
                    "TargetWeaponViewOffset",
                    (*this).target_weapon_view_offset,
                    (*recent).target_weapon_view_offset
                );
            }
            if (*this).base.base.b_demo_recording() {
                dorep!(
                    rep,
                    109,
                    KIND_PLAYER_PAWN,
                    "DemoViewPitch",
                    (*this).demo_view_pitch,
                    (*recent).demo_view_pitch
                );
                dorep!(
                    rep,
                    110,
                    KIND_PLAYER_PAWN,
                    "DemoViewYaw",
                    (*this).demo_view_yaw,
                    (*recent).demo_view_yaw
                );
            }
        } else {
            dorep_string!(
                rep,
                111,
                KIND_PLAYER_PAWN,
                "Password",
                (*this).password,
                (*recent).password
            );
            dorep!(
                rep,
                112,
                KIND_PLAYER_PAWN,
                "bReadyToPlay",
                (*this).b_ready_to_play(),
                (*recent).b_ready_to_play()
            );
        }
        Ok(())
    }
}

/// AMover::GetOptimizedRepList.
pub unsafe fn mover_get_optimized_rep_list(
    rep: &mut Rep<'_>,
    this: *const AMover,
    recent: *const AMover,
) -> RepResult<()> {
    unsafe {
        rep.base(KIND_ACTOR, this.cast(), recent.cast(), 0)?;
        if !is_native(rep, KIND_MOVER)? || (*this).base.role != ROLE_AUTHORITY {
            return Ok(());
        }
        dorep!(
            rep,
            113,
            KIND_MOVER,
            "SimOldPos",
            (*this).sim_old_pos,
            (*recent).sim_old_pos
        );
        dorep!(
            rep,
            114,
            KIND_MOVER,
            "SimOldRotPitch",
            (*this).sim_old_rot_pitch,
            (*recent).sim_old_rot_pitch
        );
        dorep!(
            rep,
            115,
            KIND_MOVER,
            "SimOldRotYaw",
            (*this).sim_old_rot_yaw,
            (*recent).sim_old_rot_yaw
        );
        dorep!(
            rep,
            116,
            KIND_MOVER,
            "SimOldRotRoll",
            (*this).sim_old_rot_roll,
            (*recent).sim_old_rot_roll
        );
        dorep!(
            rep,
            117,
            KIND_MOVER,
            "SimInterpolate",
            (*this).sim_interpolate,
            (*recent).sim_interpolate
        );
        dorep!(
            rep,
            118,
            KIND_MOVER,
            "RealPosition",
            (*this).real_position,
            (*recent).real_position
        );
        dorep!(
            rep,
            119,
            KIND_MOVER,
            "RealRotation",
            (*this).real_rotation,
            (*recent).real_rotation
        );
        Ok(())
    }
}

/// AZoneInfo::GetOptimizedRepList.
pub unsafe fn zone_info_get_optimized_rep_list(
    rep: &mut Rep<'_>,
    this: *const AZoneInfo,
    recent: *const AZoneInfo,
) -> RepResult<()> {
    unsafe {
        if (*this).base.base.b_net_initial() {
            rep.base(KIND_ACTOR, this.cast(), recent.cast(), 0)?;
        }
        if !is_native(rep, KIND_ZONE_INFO)? || (*this).base.base.role != ROLE_AUTHORITY {
            return Ok(());
        }
        dorep!(
            rep,
            120,
            KIND_ZONE_INFO,
            "ZoneGravity",
            (*this).zone_gravity,
            (*recent).zone_gravity
        );
        dorep!(
            rep,
            121,
            KIND_ZONE_INFO,
            "ZoneVelocity",
            (*this).zone_velocity,
            (*recent).zone_velocity
        );
        dorep!(
            rep,
            122,
            KIND_ZONE_INFO,
            "AmbientBrightness",
            (*this).ambient_brightness,
            (*recent).ambient_brightness
        );
        dorep!(
            rep,
            123,
            KIND_ZONE_INFO,
            "AmbientHue",
            (*this).ambient_hue,
            (*recent).ambient_hue
        );
        dorep!(
            rep,
            124,
            KIND_ZONE_INFO,
            "AmbientSaturation",
            (*this).ambient_saturation,
            (*recent).ambient_saturation
        );
        dorep!(
            rep,
            125,
            KIND_ZONE_INFO,
            "TexUPanSpeed",
            (*this).tex_u_pan_speed,
            (*recent).tex_u_pan_speed
        );
        dorep!(
            rep,
            126,
            KIND_ZONE_INFO,
            "TexVPanSpeed",
            (*this).tex_v_pan_speed,
            (*recent).tex_v_pan_speed
        );
        dorep!(
            rep,
            127,
            KIND_ZONE_INFO,
            "bReverbZone",
            (*this).b_reverb_zone(),
            (*recent).b_reverb_zone()
        );
        dorep!(
            rep,
            128,
            KIND_ZONE_INFO,
            "FogColor",
            (*this).fog_color,
            (*recent).fog_color
        );
        dorep!(
            rep,
            129,
            KIND_ACTOR,
            "Role",
            (*this).base.base.role,
            (*recent).base.base.role
        );
        dorep!(
            rep,
            130,
            KIND_ACTOR,
            "RemoteRole",
            (*this).base.base.remote_role,
            (*recent).base.base.remote_role
        );
        if !(*this).base.base.b_net_owner() || !(*this).base.base.b_client_anim() {
            dorep!(
                rep,
                131,
                KIND_ACTOR,
                "AmbientSound",
                (*this).base.base.ambient_sound,
                (*recent).base.base.ambient_sound
            );
            if !(*this).base.base.ambient_sound.is_null() {
                dorep!(
                    rep,
                    132,
                    KIND_ACTOR,
                    "SoundRadius",
                    (*this).base.base.sound_radius,
                    (*recent).base.base.sound_radius
                );
                dorep!(
                    rep,
                    133,
                    KIND_ACTOR,
                    "SoundVolume",
                    (*this).base.base.sound_volume,
                    (*recent).base.base.sound_volume
                );
                dorep!(
                    rep,
                    134,
                    KIND_ACTOR,
                    "SoundPitch",
                    (*this).base.base.sound_pitch,
                    (*recent).base.base.sound_pitch
                );
            }
        }
        Ok(())
    }
}

/// ALevelInfo::GetOptimizedRepList.
pub unsafe fn level_info_get_optimized_rep_list(
    rep: &mut Rep<'_>,
    this: *const ALevelInfo,
    recent: *const ALevelInfo,
) -> RepResult<()> {
    unsafe {
        rep.base(KIND_ZONE_INFO, this.cast(), recent.cast(), 0)?;
        if !is_native(rep, KIND_LEVEL_INFO)? || (*this).base.base.base.role != ROLE_AUTHORITY {
            return Ok(());
        }
        dorep_string!(
            rep,
            135,
            KIND_LEVEL_INFO,
            "Pauser",
            (*this).pauser,
            (*recent).pauser
        );
        dorep!(
            rep,
            136,
            KIND_LEVEL_INFO,
            "TimeDilation",
            (*this).time_dilation,
            (*recent).time_dilation
        );
        dorep!(
            rep,
            137,
            KIND_LEVEL_INFO,
            "bNoCheating",
            (*this).b_no_cheating(),
            (*recent).b_no_cheating()
        );
        dorep!(
            rep,
            138,
            KIND_LEVEL_INFO,
            "bAllowFOV",
            (*this).b_allow_fov(),
            (*recent).b_allow_fov()
        );
        Ok(())
    }
}

/// APlayerReplicationInfo::GetOptimizedRepList.
pub unsafe fn player_replication_info_get_optimized_rep_list(
    rep: &mut Rep<'_>,
    this: *const APlayerReplicationInfo,
    recent: *const APlayerReplicationInfo,
) -> RepResult<()> {
    unsafe {
        if (*this).base.base.base.b_net_initial() {
            rep.base(KIND_ACTOR, this.cast(), recent.cast(), 0)?;
        }
        if !is_native(rep, KIND_PLAYER_REPLICATION_INFO)?
            || (*this).base.base.base.role != ROLE_AUTHORITY
        {
            return Ok(());
        }
        let k = KIND_PLAYER_REPLICATION_INFO;
        dorep_string!(
            rep,
            139,
            k,
            "PlayerName",
            (*this).player_name,
            (*recent).player_name
        );
        dorep_string!(rep, 140, k, "OldName", (*this).old_name, (*recent).old_name);
        dorep!(
            rep,
            141,
            k,
            "PlayerID",
            (*this).player_id,
            (*recent).player_id
        );
        dorep_string!(
            rep,
            142,
            k,
            "TeamName",
            (*this).team_name,
            (*recent).team_name
        );
        dorep!(rep, 143, k, "Team", (*this).team, (*recent).team);
        dorep!(rep, 144, k, "TeamID", (*this).team_id, (*recent).team_id);
        dorep!(rep, 145, k, "Score", (*this).score, (*recent).score);
        dorep!(rep, 146, k, "Deaths", (*this).deaths, (*recent).deaths);
        dorep!(
            rep,
            147,
            k,
            "VoiceType",
            (*this).voice_type,
            (*recent).voice_type
        );
        dorep!(rep, 148, k, "HasFlag", (*this).has_flag, (*recent).has_flag);
        dorep!(rep, 149, k, "Ping", (*this).ping, (*recent).ping);
        dorep!(
            rep,
            150,
            k,
            "PacketLoss",
            (*this).packet_loss,
            (*recent).packet_loss
        );
        dorep!(
            rep,
            151,
            k,
            "bIsFemale",
            (*this).b_is_female(),
            (*recent).b_is_female()
        );
        dorep!(
            rep,
            152,
            k,
            "bIsABot",
            (*this).b_is_a_bot(),
            (*recent).b_is_a_bot()
        );
        dorep!(
            rep,
            153,
            k,
            "bFeigningDeath",
            (*this).b_feigning_death(),
            (*recent).b_feigning_death()
        );
        dorep!(
            rep,
            154,
            k,
            "bIsSpectator",
            (*this).b_is_spectator(),
            (*recent).b_is_spectator()
        );
        dorep!(
            rep,
            155,
            k,
            "bWaitingPlayer",
            (*this).b_waiting_player(),
            (*recent).b_waiting_player()
        );
        dorep!(
            rep,
            156,
            k,
            "bAdmin",
            (*this).b_admin(),
            (*recent).b_admin()
        );
        dorep!(
            rep,
            157,
            k,
            "TalkTexture",
            (*this).talk_texture,
            (*recent).talk_texture
        );
        dorep!(
            rep,
            158,
            k,
            "PlayerZone",
            (*this).player_zone,
            (*recent).player_zone
        );
        dorep!(
            rep,
            159,
            k,
            "PlayerLocation",
            (*this).player_location,
            (*recent).player_location
        );
        dorep!(
            rep,
            160,
            k,
            "StartTime",
            (*this).start_time,
            (*recent).start_time
        );
        Ok(())
    }
}

/// AGameReplicationInfo::GetOptimizedRepList.
pub unsafe fn game_replication_info_get_optimized_rep_list(
    rep: &mut Rep<'_>,
    this: *const AGameReplicationInfo,
    recent: *const AGameReplicationInfo,
) -> RepResult<()> {
    unsafe {
        if (*this).base.base.base.b_net_initial() {
            rep.base(KIND_ACTOR, this.cast(), recent.cast(), 0)?;
        }
        if !is_native(rep, KIND_GAME_REPLICATION_INFO)?
            || (*this).base.base.base.role != ROLE_AUTHORITY
        {
            return Ok(());
        }
        let k = KIND_GAME_REPLICATION_INFO;
        dorep!(
            rep,
            161,
            k,
            "RemainingMinute",
            (*this).remaining_minute,
            (*recent).remaining_minute
        );
        dorep!(
            rep,
            162,
            k,
            "bStopCountDown",
            (*this).b_stop_count_down(),
            (*recent).b_stop_count_down()
        );
        dorep_string!(
            rep,
            163,
            k,
            "GameEndedComments",
            (*this).game_ended_comments,
            (*recent).game_ended_comments
        );
        dorep!(
            rep,
            164,
            k,
            "NumPlayers",
            (*this).num_players,
            (*recent).num_players
        );
        if (*this).base.base.base.b_net_initial() {
            dorep_string!(
                rep,
                165,
                k,
                "GameName",
                (*this).game_name,
                (*recent).game_name
            );
            dorep_string!(
                rep,
                166,
                k,
                "GameClass",
                (*this).game_class,
                (*recent).game_class
            );
            dorep!(
                rep,
                167,
                k,
                "bTeamGame",
                (*this).b_team_game(),
                (*recent).b_team_game()
            );
            dorep!(
                rep,
                168,
                k,
                "RemainingTime",
                (*this).remaining_time,
                (*recent).remaining_time
            );
            dorep!(
                rep,
                169,
                k,
                "ElapsedTime",
                (*this).elapsed_time,
                (*recent).elapsed_time
            );
            dorep_string!(
                rep,
                170,
                k,
                "ServerName",
                (*this).server_name,
                (*recent).server_name
            );
            dorep_string!(
                rep,
                171,
                k,
                "ShortName",
                (*this).short_name,
                (*recent).short_name
            );
            dorep_string!(
                rep,
                172,
                k,
                "AdminName",
                (*this).admin_name,
                (*recent).admin_name
            );
            dorep_string!(
                rep,
                173,
                k,
                "AdminEmail",
                (*this).admin_email,
                (*recent).admin_email
            );
            dorep_string!(
                rep,
                174,
                k,
                "MOTDLine1",
                (*this).motd_line1,
                (*recent).motd_line1
            );
            dorep_string!(
                rep,
                175,
                k,
                "MOTDLine2",
                (*this).motd_line2,
                (*recent).motd_line2
            );
            dorep_string!(
                rep,
                176,
                k,
                "MOTDLine3",
                (*this).motd_line3,
                (*recent).motd_line3
            );
            dorep_string!(
                rep,
                177,
                k,
                "MOTDLine4",
                (*this).motd_line4,
                (*recent).motd_line4
            );
        }
        Ok(())
    }
}

/// AInventory::GetOptimizedRepList.
pub unsafe fn inventory_get_optimized_rep_list(
    rep: &mut Rep<'_>,
    this: *const AInventory,
    recent: *const AInventory,
    num_reps: i32,
) -> RepResult<()> {
    unsafe {
        if (*this).base.b_always_relevant() && !(*this).base.b_net_initial() {
            dorep!(
                rep,
                178,
                KIND_ACTOR,
                "bHidden",
                (*this).base.b_hidden(),
                (*recent).base.b_hidden()
            );
            return Ok(());
        }
        if is_native(rep, KIND_INVENTORY)? {
            if (*this).base.b_net_initial() || num_reps % 5 == 0 {
                rep.base(KIND_ACTOR, this.cast(), recent.cast(), num_reps)?;
                if (*this).base.b_net_owner() {
                    dorep!(
                        rep,
                        179,
                        KIND_INVENTORY,
                        "bIsAnArmor",
                        (*this).b_is_an_armor(),
                        (*recent).b_is_an_armor()
                    );
                    dorep!(
                        rep,
                        180,
                        KIND_INVENTORY,
                        "Charge",
                        (*this).charge,
                        (*recent).charge
                    );
                    dorep!(
                        rep,
                        181,
                        KIND_INVENTORY,
                        "bActivatable",
                        (*this).b_activatable(),
                        (*recent).b_activatable()
                    );
                    dorep!(
                        rep,
                        182,
                        KIND_INVENTORY,
                        "bActive",
                        (*this).b_active(),
                        (*recent).b_active()
                    );
                    dorep!(
                        rep,
                        183,
                        KIND_INVENTORY,
                        "PlayerViewOffset",
                        (*this).player_view_offset,
                        (*recent).player_view_offset
                    );
                    dorep!(
                        rep,
                        184,
                        KIND_INVENTORY,
                        "PlayerViewMesh",
                        (*this).player_view_mesh,
                        (*recent).player_view_mesh
                    );
                    dorep!(
                        rep,
                        185,
                        KIND_INVENTORY,
                        "PlayerViewScale",
                        (*this).player_view_scale,
                        (*recent).player_view_scale
                    );
                }
            } else if (*this).base.role == ROLE_AUTHORITY {
                dorep!(
                    rep,
                    186,
                    KIND_ACTOR,
                    "LightType",
                    (*this).base.light_type,
                    (*recent).base.light_type
                );
                dorep!(
                    rep,
                    187,
                    KIND_ACTOR,
                    "bCollideWorld",
                    (*this).base.b_collide_world(),
                    (*recent).base.b_collide_world()
                );
                dorep!(
                    rep,
                    188,
                    KIND_ACTOR,
                    "bHidden",
                    (*this).base.b_hidden(),
                    (*recent).base.b_hidden()
                );
                if (*this).base.b_net_owner() {
                    dorep!(
                        rep,
                        189,
                        KIND_ACTOR,
                        "Inventory",
                        (*this).base.inventory,
                        (*recent).base.inventory
                    );
                }
                if !(*this).base.b_carried_item() && (*this).base.remote_role < ROLE_SIMULATED_PROXY
                {
                    dorep!(
                        rep,
                        190,
                        KIND_ACTOR,
                        "Location",
                        (*this).base.location,
                        (*recent).base.location
                    );
                    if (*this).base.draw_type == DT_MESH || (*this).base.draw_type == DT_BRUSH {
                        dorep!(
                            rep,
                            191,
                            KIND_ACTOR,
                            "Rotation",
                            (*this).base.rotation,
                            (*recent).base.rotation
                        );
                    }
                }
                if (*this).base.draw_type == DT_MESH {
                    dorep!(
                        rep,
                        192,
                        KIND_ACTOR,
                        "bUnlit",
                        (*this).base.b_unlit(),
                        (*recent).base.b_unlit()
                    );
                    if (((*this).base.remote_role <= ROLE_SIMULATED_PROXY)
                        && (!(*this).base.b_net_owner() || !(*this).base.b_client_anim()))
                        || (*this).base.b_demo_recording()
                    {
                        dorep!(
                            rep,
                            193,
                            KIND_ACTOR,
                            "AnimSequence",
                            (*this).base.anim_sequence,
                            (*recent).base.anim_sequence
                        );
                        dorep!(
                            rep,
                            194,
                            KIND_ACTOR,
                            "SimAnim",
                            (*this).base.sim_anim,
                            (*recent).base.sim_anim
                        );
                        dorep!(
                            rep,
                            195,
                            KIND_ACTOR,
                            "AnimMinRate",
                            (*this).base.anim_min_rate,
                            (*recent).base.anim_min_rate
                        );
                        dorep!(
                            rep,
                            196,
                            KIND_ACTOR,
                            "bAnimNotify",
                            (*this).base.b_anim_notify(),
                            (*recent).base.b_anim_notify()
                        );
                    }
                } else if (*this).base.draw_type == DT_SPRITE
                    && !(*this).base.b_hidden()
                    && (!(*this).base.b_only_owner_see() || (*this).base.b_net_owner())
                {
                    dorep!(
                        rep,
                        197,
                        KIND_ACTOR,
                        "Sprite",
                        (*this).base.sprite,
                        (*recent).base.sprite
                    );
                }
                if (*this).base.light_type != LT_NONE {
                    dorep!(
                        rep,
                        198,
                        KIND_ACTOR,
                        "LightEffect",
                        (*this).base.light_effect,
                        (*recent).base.light_effect
                    );
                    dorep!(
                        rep,
                        199,
                        KIND_ACTOR,
                        "LightBrightness",
                        (*this).base.light_brightness,
                        (*recent).base.light_brightness
                    );
                    dorep!(
                        rep,
                        200,
                        KIND_ACTOR,
                        "LightHue",
                        (*this).base.light_hue,
                        (*recent).base.light_hue
                    );
                    dorep!(
                        rep,
                        201,
                        KIND_ACTOR,
                        "LightSaturation",
                        (*this).base.light_saturation,
                        (*recent).base.light_saturation
                    );
                    dorep!(
                        rep,
                        202,
                        KIND_ACTOR,
                        "LightRadius",
                        (*this).base.light_radius,
                        (*recent).base.light_radius
                    );
                }
                if (*this).base.remote_role == ROLE_SIMULATED_PROXY {
                    dorep!(
                        rep,
                        203,
                        KIND_ACTOR,
                        "Base",
                        (*this).base.base,
                        (*recent).base.base
                    );
                } else if (*this).base.b_sim_fall() {
                    dorep!(
                        rep,
                        204,
                        KIND_ACTOR,
                        "Physics",
                        (*this).base.physics,
                        (*recent).base.physics
                    );
                    dorep!(
                        rep,
                        205,
                        KIND_ACTOR,
                        "Acceleration",
                        (*this).base.acceleration,
                        (*recent).base.acceleration
                    );
                    dorep!(
                        rep,
                        206,
                        KIND_ACTOR,
                        "bBounce",
                        (*this).base.b_bounce(),
                        (*recent).base.b_bounce()
                    );
                    dorep!(
                        rep,
                        207,
                        KIND_ACTOR,
                        "Velocity",
                        (*this).base.velocity,
                        (*recent).base.velocity
                    );
                }
                if (*this).base.b_net_owner() && instigator_weapon_is(this) {
                    dorep!(
                        rep,
                        208,
                        KIND_INVENTORY,
                        "PlayerViewOffset",
                        (*this).player_view_offset,
                        (*recent).player_view_offset
                    );
                    dorep!(
                        rep,
                        209,
                        KIND_INVENTORY,
                        "PlayerViewMesh",
                        (*this).player_view_mesh,
                        (*recent).player_view_mesh
                    );
                    dorep!(
                        rep,
                        210,
                        KIND_INVENTORY,
                        "PlayerViewScale",
                        (*this).player_view_scale,
                        (*recent).player_view_scale
                    );
                }
            }
            if (*this).base.role == ROLE_AUTHORITY {
                if !(*this).base.b_net_owner()
                    && (*this).base.remote_role == ROLE_SIMULATED_PROXY
                    && !(*this).base.ambient_sound.is_null()
                {
                    dorep!(
                        rep,
                        211,
                        KIND_ACTOR,
                        "Location",
                        (*this).base.location,
                        (*recent).base.location
                    );
                }
                dorep!(
                    rep,
                    212,
                    KIND_INVENTORY,
                    "FlashCount",
                    (*this).flash_count,
                    (*recent).flash_count
                );
                dorep!(
                    rep,
                    213,
                    KIND_INVENTORY,
                    "bSteadyFlash3rd",
                    (*this).b_steady_flash3rd(),
                    (*recent).b_steady_flash3rd()
                );
                dorep!(
                    rep,
                    214,
                    KIND_INVENTORY,
                    "ThirdPersonMesh",
                    (*this).third_person_mesh,
                    (*recent).third_person_mesh
                );
                dorep!(
                    rep,
                    215,
                    KIND_INVENTORY,
                    "ThirdPersonScale",
                    (*this).third_person_scale,
                    (*recent).third_person_scale
                );
            }
        } else {
            rep.base(KIND_ACTOR, this.cast(), recent.cast(), num_reps)?;
        }
        Ok(())
    }
}

/// `bNetOwner && Instigator && (Instigator->Weapon == this)`. `Instigator` is an
/// APawn, so it is read as one; the C++ does the same.
unsafe fn instigator_weapon_is(this: *const AInventory) -> bool {
    unsafe {
        let instigator = (*this).base.instigator;
        if instigator.is_null() {
            return false;
        }
        // SAFETY: a non-null Instigator is an APawn in the engine's object graph, the same
        // object the C++ dereferences.
        let weapon = unsafe { (*instigator.cast::<APawn>()).weapon };
        weapon.cast_const() == this.cast()
    }
}

/// AInventory::ShouldDoScriptReplication.
pub unsafe fn inventory_should_do_script_replication(this: *const AInventory) -> bool {
    unsafe { (*this).base.b_net_owner() || (*this).base.b_net_initial() }
}

/// Canonical `AActor::UpdateFrequency` dispatch, including the Inventory override.
///
/// # Safety
/// `actor` and `viewer` are live canonical records; vector pointers span three f32.
pub unsafe fn actor_update_frequency(
    access: hp2_core::core_access::CoreAccess<'_>,
    actor: *mut c_void,
    viewer: *mut c_void,
    view_dir: *mut f32,
    view_pos: *mut f32,
) -> hp2_core::core_access::RuntimeResult<f32> {
    if actor.is_null() || view_dir.is_null() || view_pos.is_null() {
        return Err(hp2_core::core_access::RuntimeError::new("Invalid UpdateFrequency arguments"));
    }
    let this = actor.cast::<AActor>();
    let inventory = access.native_class("Engine.Inventory")?;
    if unsafe { hp2_core::object_model::is_a(actor, inventory.cast()) } {
        return unsafe { inventory_update_frequency_core(access, this.cast(), viewer) };
    }
    unsafe {
        if (*this).b_net_temporary() && !(*this).b_net_optional() {
            let class = (*this.cast::<hp2_abi::engine::generated::UObject>()).class
                .cast::<hp2_abi::engine::generated::UClass>();
            let defaults = (*class).defaults;
            if defaults.num == 0 || defaults.data.is_null() {
                return Err(hp2_core::core_access::RuntimeError::new("Actor class has no default actor"));
            }
            let default_life_span = (*defaults.data.cast::<AActor>()).life_span;
            if (*this).life_span < default_life_span - 0.2 {
                let loc = (*this).location;
                let view_pos = &*view_pos.cast::<FVector>();
                let view_dir = &*view_dir.cast::<FVector>();
                let d = FVector { x: loc.x - view_pos.x, y: loc.y - view_pos.y, z: loc.z - view_pos.z };
                let size_squared = d.z.mul_add(d.z, d.x.mul_add(d.x, d.y * d.y));
                let dot = view_dir.z.mul_add(d.z, view_dir.x.mul_add(d.x, view_dir.y * d.y));
                if size_squared > 1_000_000.0 || dot < 0.0 {
                    return Ok(min_f32((*this).net_update_frequency, 8.0));
                }
            }
        }
        Ok((*this).net_update_frequency)
    }
}

unsafe fn inventory_update_frequency_core(
    access: hp2_core::core_access::CoreAccess<'_>,
    this: *const AInventory,
    viewer: *mut c_void,
) -> hp2_core::core_access::RuntimeResult<f32> {
    unsafe {
        let owner = (*this).base.owner;
        if (*this).base.b_hidden() && !owner.is_null() {
            let pawn = access.native_class("Engine.Pawn")?;
            if unsafe { hp2_core::object_model::is_a(owner, pawn.cast()) } {
                let weapon = (*owner.cast::<APawn>()).weapon;
                if weapon.cast_const() != this.cast() {
                    return Ok(min_f32((*this).base.net_update_frequency, 4.0));
                }
                if owner.cast_const() != viewer.cast_const() {
                    return Ok(min_f32((*this).base.net_update_frequency, 10.0));
                }
            }
        }
        Ok((*this).base.net_update_frequency)
    }
}

/// AInventory::NoVariablesToReplicate.
pub unsafe fn inventory_no_variables_to_replicate(
    this: *const AInventory,
    old_ver: *const AActor,
) -> bool {
    unsafe { (*this).base.b_hidden() == (*old_ver).b_hidden() }
}

/// AInventory::UpdateFrequency.
pub unsafe fn inventory_update_frequency(
    svc: &Hp2ActorRepServices,
    this: *const AInventory,
    viewer: *const c_void,
) -> RepResult<f32> {
    unsafe {
        let owner = (*this).base.owner;
        if (*this).base.b_hidden() && !owner.is_null() {
            // SAFETY: the service is provided by the linked C++ side.
            let mut is_pawn = 0;
            checked((svc.object_is_a)(
                owner.cast_const(),
                KIND_PAWN,
                &mut is_pawn,
            ))?;
            if is_pawn != 0 {
                // SAFETY: Owner passed the IsA(APawn) check above.
                let weapon = unsafe { (*(*this).base.owner.cast::<APawn>()).weapon };
                if weapon.cast_const() != this.cast() {
                    return Ok(min_f32((*this).base.net_update_frequency, 4.0));
                }
                if (*this).base.owner.cast_const() != viewer {
                    return Ok(min_f32((*this).base.net_update_frequency, 10.0));
                }
            }
        }
        Ok((*this).base.net_update_frequency)
    }
}

/// AWeapon::GetOptimizedRepList.
pub unsafe fn weapon_get_optimized_rep_list(
    rep: &mut Rep<'_>,
    this: *const AWeapon,
    recent: *const AWeapon,
    num_reps: i32,
) -> RepResult<()> {
    unsafe {
        if (*this).base.base.b_always_relevant() && !(*this).base.base.b_net_initial() {
            dorep!(
                rep,
                216,
                KIND_ACTOR,
                "bHidden",
                (*this).base.base.b_hidden(),
                (*recent).base.base.b_hidden()
            );
            return Ok(());
        }
        rep.base(KIND_INVENTORY, this.cast(), recent.cast(), num_reps)?;
        if is_native(rep, KIND_WEAPON)?
            && (*this).base.base.b_net_owner()
            && (*this).base.base.role == ROLE_AUTHORITY
        {
            dorep!(
                rep,
                217,
                KIND_WEAPON,
                "AmmoType",
                (*this).ammo_type,
                (*recent).ammo_type
            );
            dorep!(
                rep,
                218,
                KIND_WEAPON,
                "bLockedOn",
                (*this).b_locked_on(),
                (*recent).b_locked_on()
            );
            dorep!(
                rep,
                219,
                KIND_WEAPON,
                "bHideWeapon",
                (*this).b_hide_weapon(),
                (*recent).b_hide_weapon()
            );
        }
        Ok(())
    }
}

/// AAmmo::GetOptimizedRepList.
pub unsafe fn ammo_get_optimized_rep_list(
    rep: &mut Rep<'_>,
    this: *const AAmmo,
    recent: *const AAmmo,
    num_reps: i32,
) -> RepResult<()> {
    unsafe {
        if (*this).base.base.base.b_always_relevant() && !(*this).base.base.base.b_net_initial() {
            dorep!(
                rep,
                220,
                KIND_ACTOR,
                "bHidden",
                (*this).base.base.base.b_hidden(),
                (*recent).base.base.base.b_hidden()
            );
            return Ok(());
        }
        rep.base(KIND_PICKUP, this.cast(), recent.cast(), num_reps)?;
        if is_native(rep, KIND_AMMO)?
            && (*this).base.base.base.b_net_owner()
            && (*this).base.base.base.role == ROLE_AUTHORITY
        {
            dorep!(
                rep,
                221,
                KIND_AMMO,
                "AmmoAmount",
                (*this).ammo_amount,
                (*recent).ammo_amount
            );
        }
        Ok(())
    }
}

/// APickup::GetOptimizedRepList.
pub unsafe fn pickup_get_optimized_rep_list(
    rep: &mut Rep<'_>,
    this: *const APickup,
    recent: *const APickup,
    num_reps: i32,
) -> RepResult<()> {
    unsafe {
        if (*this).base.base.b_always_relevant() && !(*this).base.base.b_net_initial() {
            dorep!(
                rep,
                222,
                KIND_ACTOR,
                "bHidden",
                (*this).base.base.b_hidden(),
                (*recent).base.base.b_hidden()
            );
            return Ok(());
        }
        rep.base(KIND_INVENTORY, this.cast(), recent.cast(), num_reps)?;
        if is_native(rep, KIND_PICKUP)?
            && (*this).base.base.b_net_owner()
            && (*this).base.base.role == ROLE_AUTHORITY
        {
            dorep!(
                rep,
                223,
                KIND_PICKUP,
                "NumCopies",
                (*this).num_copies,
                (*recent).num_copies
            );
        }
        Ok(())
    }
}
