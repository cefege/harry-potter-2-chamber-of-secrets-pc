//! Replication block of `UnActor.cpp`: the NEQ/DOREP comparisons and the
//! `GetOptimizedRepList` / `UpdateFrequency` bodies of AActor and its subclasses
//! down to AAmmo and APickup.
//!
//! The C++ keeps the replicated property's `UProperty*` in a block-scope static.
//! This module caches the `RepIndex` value instead, in a `RepCache` keyed by
//! (class, property). The index is fixed once the classes are linked, so the result
//! is the same; the lookup count is per property rather than per DOREP site.

use std::collections::HashMap;
use std::ffi::{c_char, c_void};
use std::ptr;

use hp2_abi::engine::generated::{
    AActor, AAmmo, AGameReplicationInfo, AInventory, ALevelInfo, AMover, APawn, APickup,
    APlayerPawn, APlayerReplicationInfo, AWeapon, AZoneInfo, FRotator, TArrayRaw,
};
use hp2_abi::{FPlane, FVector};

/// Services provided by the C++ side (`Hp2ActorRepServices.cpp`). The kind codes
/// are the DOREP class arguments; the C++ adapter maps each code to the UClass
/// of the same name.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2ActorRepServices {
    /// `A##kind::StaticClass()->ClassFlags`.
    pub static_class_flags: unsafe extern "C" fn(kind: u32) -> u32,
    /// `FindObjectChecked<UProperty>(kind's class, name)->RepIndex`.
    pub find_rep_index: unsafe extern "C" fn(kind: u32, name: *const c_char) -> u32,
    /// `Map->CanSerializeObject(obj)`.
    pub can_serialize_object: unsafe extern "C" fn(map: *mut c_void, obj: *mut c_void) -> i32,
    /// `*(const FString*)a != *(const FString*)b`.
    pub fstring_neq: unsafe extern "C" fn(a: *const c_void, b: *const c_void) -> i32,
    /// `((UObject*)obj)->IsA(kind's class)`.
    pub object_is_a: unsafe extern "C" fn(obj: *const c_void, kind: u32) -> i32,
    /// `((AActor*)actor)->GetClass()->GetDefaultActor()`.
    pub default_actor_of: unsafe extern "C" fn(actor: *const c_void) -> *const c_void,
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

/// Output cursor for replicated property indices (the C++ `INT* Ptr`).
pub struct Rep<'a> {
    svc: &'a Hp2ActorRepServices,
    cache: &'a mut RepCache,
    map: *mut c_void,
    out: *mut i32,
}

impl<'a> Rep<'a> {
    /// # Safety
    /// `map` must be the live `UPackageMap` the C++ caller passed in. `out` must
    /// point to writable storage with room for every index the replicated function
    /// pushes (the C++ `NumReps` contract).
    pub unsafe fn new(
        svc: &'a Hp2ActorRepServices,
        cache: &'a mut RepCache,
        map: *mut c_void,
        out: *mut i32,
    ) -> Self {
        Self {
            svc,
            cache,
            map,
            out,
        }
    }

    fn push(&mut self, index: u32) {
        // SAFETY: `new` guarantees room for every push.
        unsafe {
            self.out.write(index as i32);
            self.out = self.out.add(1);
        }
    }

    /// The advanced output cursor (the C++ return value of GetOptimizedRepList).
    pub fn finish(self) -> *mut i32 {
        self.out
    }

    fn class_flags(&self, kind: u32) -> u32 {
        // SAFETY: the service is provided by the linked C++ side.
        unsafe { (self.svc.static_class_flags)(kind) }
    }
}

fn is_native(rep: &Rep<'_>, kind: u32) -> bool {
    (rep.class_flags(kind) & CLASS_NATIVE_REPLICATION) != 0
}

fn min_f32(a: f32, b: f32) -> f32 {
    // UnTemplate.h `Min`: `(A<=B) ? A : B`.
    if a <= b { a } else { b }
}

/// NEQ overloads from `UnActor.cpp`: "has this value changed since Recent".
trait Neq: Copy {
    fn neq(self, other: Self, rep: &Rep<'_>) -> bool;
}

impl Neq for u8 {
    fn neq(self, other: Self, _rep: &Rep<'_>) -> bool {
        self != other
    }
}

impl Neq for i32 {
    fn neq(self, other: Self, _rep: &Rep<'_>) -> bool {
        self != other
    }
}

impl Neq for bool {
    fn neq(self, other: Self, _rep: &Rep<'_>) -> bool {
        self != other
    }
}

impl Neq for f32 {
    // NEQ(FLOAT) compares the raw bits.
    fn neq(self, other: Self, _rep: &Rep<'_>) -> bool {
        self.to_bits() != other.to_bits()
    }
}

impl Neq for FVector {
    fn neq(self, other: Self, _rep: &Rep<'_>) -> bool {
        self.x.to_bits() != other.x.to_bits()
            || self.y.to_bits() != other.y.to_bits()
            || self.z.to_bits() != other.z.to_bits()
    }
}

impl Neq for FPlane {
    fn neq(self, other: Self, _rep: &Rep<'_>) -> bool {
        self.x.to_bits() != other.x.to_bits()
            || self.y.to_bits() != other.y.to_bits()
            || self.z.to_bits() != other.z.to_bits()
            || self.w.to_bits() != other.w.to_bits()
    }
}

impl Neq for FRotator {
    fn neq(self, other: Self, _rep: &Rep<'_>) -> bool {
        self.pitch != other.pitch || self.yaw != other.yaw || self.roll != other.roll
    }
}

/// FName (4 bytes) and FColor (R,G,B,A) compare byte for byte.
impl Neq for [u8; 4] {
    fn neq(self, other: Self, _rep: &Rep<'_>) -> bool {
        self != other
    }
}

/// NEQ(UObject*): `(Map->CanSerializeObject(A) ? A : NULL) != B`. The serialize
/// check runs even when `A` is NULL, as in the C++.
impl Neq for *mut c_void {
    fn neq(self, other: Self, rep: &Rep<'_>) -> bool {
        // SAFETY: the service is provided by the linked C++ side.
        let kept = if unsafe { (rep.svc.can_serialize_object)(rep.map, self) } != 0 {
            self
        } else {
            ptr::null_mut()
        };
        kept != other
    }
}

/// NEQ(FString): both operands are copied locally so the C++ compares the same
/// bytes the Rust read.
impl Neq for TArrayRaw {
    fn neq(self, other: Self, rep: &Rep<'_>) -> bool {
        let a = self;
        let b = other;
        // SAFETY: the service is provided by the linked C++ side; `a` and `b` are
        // live locals for the duration of the call.
        unsafe { (rep.svc.fstring_neq)(ptr::from_ref(&a).cast(), ptr::from_ref(&b).cast()) != 0 }
    }
}

/// Resolved `RepIndex` values, keyed by (class kind, NUL-terminated property name).
/// One instance lives in the FFI layer; each replay test builds its own.
#[derive(Default)]
pub struct RepCache {
    indices: HashMap<(u32, &'static str), u32>,
}

impl RepCache {
    /// Looks up the property's `RepIndex` once per cache.
    fn index_of(&mut self, svc: &Hp2ActorRepServices, kind: u32, name: &'static str) -> u32 {
        *self.indices.entry((kind, name)).or_insert_with(|| {
            // SAFETY: the service is provided by the linked C++ side; `name` is a
            // NUL-terminated static string.
            unsafe { (svc.find_rep_index)(kind, name.as_ptr().cast()) }
        })
    }
}

/// `DOREP(c,v)`: compare against Recent; on change push the property's RepIndex.
/// The index lookup happens only after NEQ reports a change, as in the C++.
macro_rules! dorep {
    ($rep:ident, $kind:expr, $name:literal, $a:expr, $b:expr $(,)?) => {{
        if Neq::neq($a, $b, $rep) {
            let index = $rep.cache.index_of($rep.svc, $kind, concat!($name, "\0"));
            $rep.push(index);
        }
    }};
}

/// `DOREPARRAY(c,v)`: the index lookup runs before the loop, as the C++ block-scope
/// static is initialised whenever the statement executes.
macro_rules! dorep_array {
    ($rep:ident, $kind:expr, $name:literal, $a:expr, $b:expr $(,)?) => {{
        let base = $rep.cache.index_of($rep.svc, $kind, concat!($name, "\0"));
        let (va, vb) = ($a, $b);
        for (i, (&x, &y)) in va.iter().zip(vb.iter()).enumerate() {
            if Neq::neq(x, y, $rep) {
                $rep.push(base + i as u32);
            }
        }
    }};
}

/// AActor::GetOptimizedRepList.
pub fn actor_get_optimized_rep_list(rep: &mut Rep<'_>, this: &AActor, recent: &AActor) {
    if !is_native(rep, KIND_ACTOR) || this.role != ROLE_AUTHORITY {
        return;
    }
    dorep!(rep, KIND_ACTOR, "Owner", this.owner, recent.owner);
    dorep!(rep, KIND_ACTOR, "Role", this.role, recent.role);
    dorep!(
        rep,
        KIND_ACTOR,
        "RemoteRole",
        this.remote_role,
        recent.remote_role
    );
    dorep!(
        rep,
        KIND_ACTOR,
        "bCollideActors",
        this.b_collide_actors(),
        recent.b_collide_actors()
    );
    dorep!(
        rep,
        KIND_ACTOR,
        "bCollideWorld",
        this.b_collide_world(),
        recent.b_collide_world()
    );
    dorep!(
        rep,
        KIND_ACTOR,
        "LightType",
        this.light_type,
        recent.light_type
    );
    dorep!(
        rep,
        KIND_ACTOR,
        "bHidden",
        this.b_hidden(),
        recent.b_hidden()
    );
    dorep!(
        rep,
        KIND_ACTOR,
        "bOnlyOwnerSee",
        this.b_only_owner_see(),
        recent.b_only_owner_see()
    );
    dorep!(rep, KIND_ACTOR, "Texture", this.texture, recent.texture);
    dorep!(
        rep,
        KIND_ACTOR,
        "DrawScale",
        this.draw_scale,
        recent.draw_scale
    );
    dorep!(
        rep,
        KIND_ACTOR,
        "DrawType",
        this.draw_type,
        recent.draw_type
    );
    dorep!(rep, KIND_ACTOR, "Style", this.style, recent.style);
    if this.b_net_owner() {
        dorep!(
            rep,
            KIND_ACTOR,
            "bNetOwner",
            this.b_net_owner(),
            recent.b_net_owner()
        );
        dorep!(
            rep,
            KIND_ACTOR,
            "Inventory",
            this.inventory,
            recent.inventory
        );
    }
    if this.b_replicate_instigator() && this.remote_role >= ROLE_SIMULATED_PROXY {
        dorep!(
            rep,
            KIND_ACTOR,
            "Instigator",
            this.instigator,
            recent.instigator
        );
    }
    if !this.b_net_owner() || !this.b_client_anim() {
        dorep!(
            rep,
            KIND_ACTOR,
            "AmbientSound",
            this.ambient_sound,
            recent.ambient_sound
        );
        if !this.ambient_sound.is_null() {
            dorep!(
                rep,
                KIND_ACTOR,
                "SoundRadius",
                this.sound_radius,
                recent.sound_radius
            );
            dorep!(
                rep,
                KIND_ACTOR,
                "SoundVolume",
                this.sound_volume,
                recent.sound_volume
            );
            dorep!(
                rep,
                KIND_ACTOR,
                "SoundPitch",
                this.sound_pitch,
                recent.sound_pitch
            );
            dorep!(
                rep,
                KIND_ACTOR,
                "SoundOcclusion",
                this.sound_occlusion,
                recent.sound_occlusion
            );
        }
    }
    if this.b_collide_actors() || this.b_collide_world() {
        dorep!(
            rep,
            KIND_ACTOR,
            "bProjTarget",
            this.b_proj_target(),
            recent.b_proj_target()
        );
        dorep!(
            rep,
            KIND_ACTOR,
            "bBlockActors",
            this.b_block_actors(),
            recent.b_block_actors()
        );
        dorep!(
            rep,
            KIND_ACTOR,
            "bBlockPlayers",
            this.b_block_players(),
            recent.b_block_players()
        );
        dorep!(
            rep,
            KIND_ACTOR,
            "CollisionRadius",
            this.collision_radius,
            recent.collision_radius
        );
        dorep!(
            rep,
            KIND_ACTOR,
            "CollisionHeight",
            this.collision_height,
            recent.collision_height
        );
    }
    if !this.b_carried_item()
        && (this.b_net_initial()
            || this.b_simulated_pawn()
            || this.remote_role < ROLE_SIMULATED_PROXY)
    {
        dorep!(rep, KIND_ACTOR, "Location", this.location, recent.location);
        if this.draw_type == DT_MESH || this.draw_type == DT_BRUSH || this.draw_type == DT_PARTICLES
        {
            dorep!(rep, KIND_ACTOR, "Rotation", this.rotation, recent.rotation);
        }
    }
    if this.draw_type == DT_MESH {
        dorep!(
            rep,
            KIND_ACTOR,
            "AmbientGlow",
            this.ambient_glow,
            recent.ambient_glow
        );
        dorep!(
            rep,
            KIND_ACTOR,
            "ScaleGlow",
            this.scale_glow,
            recent.scale_glow
        );
        dorep!(rep, KIND_ACTOR, "bUnlit", this.b_unlit(), recent.b_unlit());
        dorep!(rep, KIND_ACTOR, "Fatness", this.fatness, recent.fatness);
        dorep!(rep, KIND_ACTOR, "Wideness", this.wideness, recent.wideness);
        dorep!(
            rep,
            KIND_ACTOR,
            "PrePivot",
            this.pre_pivot,
            recent.pre_pivot
        );
        dorep!(rep, KIND_ACTOR, "Mesh", this.mesh, recent.mesh);
        dorep!(
            rep,
            KIND_ACTOR,
            "bMeshEnviroMap",
            this.b_mesh_enviro_map(),
            recent.b_mesh_enviro_map()
        );
        dorep!(rep, KIND_ACTOR, "Skin", this.skin, recent.skin);
        dorep_array!(
            rep,
            KIND_ACTOR,
            "MultiSkins",
            this.multi_skins,
            recent.multi_skins
        );
        if ((this.remote_role <= ROLE_SIMULATED_PROXY)
            && (!this.b_net_owner() || !this.b_client_anim()))
            || this.b_demo_recording()
        {
            dorep!(
                rep,
                KIND_ACTOR,
                "AnimSequence",
                this.anim_sequence,
                recent.anim_sequence
            );
            dorep!(rep, KIND_ACTOR, "SimAnim", this.sim_anim, recent.sim_anim);
            dorep!(
                rep,
                KIND_ACTOR,
                "AnimMinRate",
                this.anim_min_rate,
                recent.anim_min_rate
            );
            dorep!(
                rep,
                KIND_ACTOR,
                "bAnimNotify",
                this.b_anim_notify(),
                recent.b_anim_notify()
            );
        }
    } else if this.draw_type == DT_SPRITE {
        if !this.b_hidden() && (!this.b_only_owner_see() || this.b_net_owner()) {
            dorep!(rep, KIND_ACTOR, "Sprite", this.sprite, recent.sprite);
        }
    } else if this.draw_type == DT_BRUSH {
        dorep!(
            rep,
            KIND_ACTOR,
            "PrePivot",
            this.pre_pivot,
            recent.pre_pivot
        );
        dorep!(rep, KIND_ACTOR, "Brush", this.brush, recent.brush);
    }
    if this.light_type != LT_NONE {
        dorep!(
            rep,
            KIND_ACTOR,
            "LightEffect",
            this.light_effect,
            recent.light_effect
        );
        dorep!(
            rep,
            KIND_ACTOR,
            "LightBrightness",
            this.light_brightness,
            recent.light_brightness
        );
        dorep!(
            rep,
            KIND_ACTOR,
            "LightHue",
            this.light_hue,
            recent.light_hue
        );
        dorep!(
            rep,
            KIND_ACTOR,
            "LightSaturation",
            this.light_saturation,
            recent.light_saturation
        );
        dorep!(
            rep,
            KIND_ACTOR,
            "bDarkLight",
            this.b_dark_light(),
            recent.b_dark_light()
        );
        dorep!(
            rep,
            KIND_ACTOR,
            "LightRadius",
            this.light_radius,
            recent.light_radius
        );
        dorep!(
            rep,
            KIND_ACTOR,
            "LightRadiusInner",
            this.light_radius_inner,
            recent.light_radius_inner
        );
        dorep!(
            rep,
            KIND_ACTOR,
            "LightPeriod",
            this.light_period,
            recent.light_period
        );
        dorep!(
            rep,
            KIND_ACTOR,
            "LightPhase",
            this.light_phase,
            recent.light_phase
        );
        dorep!(
            rep,
            KIND_ACTOR,
            "VolumeBrightness",
            this.volume_brightness,
            recent.volume_brightness
        );
        dorep!(
            rep,
            KIND_ACTOR,
            "VolumeRadius",
            this.volume_radius,
            recent.volume_radius
        );
        dorep!(
            rep,
            KIND_ACTOR,
            "bSpecialLit",
            this.b_special_lit(),
            recent.b_special_lit()
        );
    }
    if this.remote_role == ROLE_SIMULATED_PROXY {
        dorep!(rep, KIND_ACTOR, "Base", this.base, recent.base);
        if this.b_net_initial() {
            if !this.b_simulated_pawn() {
                dorep!(rep, KIND_ACTOR, "Physics", this.physics, recent.physics);
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "Acceleration",
                    this.acceleration,
                    recent.acceleration
                );
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "bBounce",
                    this.b_bounce(),
                    recent.b_bounce()
                );
            }
            if this.physics == PHYS_ROTATING {
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "bFixedRotationDir",
                    this.b_fixed_rotation_dir(),
                    recent.b_fixed_rotation_dir()
                );
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "bRotateToDesired",
                    this.b_rotate_to_desired(),
                    recent.b_rotate_to_desired()
                );
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "RotationRate",
                    this.rotation_rate,
                    recent.rotation_rate
                );
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "DesiredRotation",
                    this.desired_rotation,
                    recent.desired_rotation
                );
            }
        }
    } else if this.b_sim_fall() {
        dorep!(rep, KIND_ACTOR, "Physics", this.physics, recent.physics);
        dorep!(
            rep,
            KIND_ACTOR,
            "Acceleration",
            this.acceleration,
            recent.acceleration
        );
        dorep!(
            rep,
            KIND_ACTOR,
            "bBounce",
            this.b_bounce(),
            recent.b_bounce()
        );
    }
    if this.b_sim_fall()
        || this.b_is_mover()
        || (this.remote_role == ROLE_SIMULATED_PROXY
            && (this.b_net_initial() || this.b_simulated_pawn()))
    {
        dorep!(rep, KIND_ACTOR, "Velocity", this.velocity, recent.velocity);
    }
}

/// AActor::UpdateFrequency.
pub fn actor_update_frequency(
    svc: &Hp2ActorRepServices,
    this: &AActor,
    view_dir: &FVector,
    view_pos: &FVector,
) -> f32 {
    if this.b_net_temporary() && !this.b_net_optional() {
        // SAFETY: the service returns the class default object, which the C++ checks
        // before returning (UnClass.h GetDefaultActor).
        let default_life_span = unsafe {
            (*(svc.default_actor_of)(ptr::from_ref(this).cast()).cast::<AActor>()).life_span
        };
        if this.life_span < default_life_span - 0.2 {
            let loc = this.location;
            let d = FVector {
                x: loc.x - view_pos.x,
                y: loc.y - view_pos.y,
                z: loc.z - view_pos.z,
            };
            // Fused as the oracle emits (fmadd chains, see FMA table in notes).
            let size_squared = d.z.mul_add(d.z, d.x.mul_add(d.x, d.y * d.y));
            let dot = view_dir
                .z
                .mul_add(d.z, view_dir.x.mul_add(d.x, view_dir.y * d.y));
            if size_squared > 1_000_000.0 || dot < 0.0 {
                return min_f32(this.net_update_frequency, 8.0);
            }
        }
    }
    this.net_update_frequency
}

/// APawn::GetOptimizedRepList.
pub fn pawn_get_optimized_rep_list(rep: &mut Rep<'_>, this: &APawn, recent: &APawn, num_reps: i32) {
    if is_native(rep, KIND_PAWN) {
        if this.base.b_net_owner() || this.base.b_net_initial() || num_reps % 5 == 0 {
            actor_get_optimized_rep_list(rep, &this.base, &recent.base);
            if this.base.role == ROLE_AUTHORITY {
                dorep!(
                    rep,
                    KIND_PAWN,
                    "PlayerReplicationInfo",
                    this.player_replication_info,
                    recent.player_replication_info
                );
                dorep!(rep, KIND_PAWN, "Health", this.health, recent.health);
                if this.base.b_net_owner() {
                    dorep!(
                        rep,
                        KIND_PAWN,
                        "bIsPlayer",
                        this.b_is_player(),
                        recent.b_is_player()
                    );
                    dorep!(
                        rep,
                        KIND_PAWN,
                        "carriedDecoration",
                        this.carried_decoration,
                        recent.carried_decoration
                    );
                    dorep!(
                        rep,
                        KIND_PAWN,
                        "MoveTarget",
                        this.move_target,
                        recent.move_target
                    );
                    dorep!(
                        rep,
                        KIND_PAWN,
                        "SelectedItem",
                        this.selected_item,
                        recent.selected_item
                    );
                    dorep!(
                        rep,
                        KIND_PAWN,
                        "GroundSpeed",
                        this.ground_speed,
                        recent.ground_speed
                    );
                    dorep!(
                        rep,
                        KIND_PAWN,
                        "WaterSpeed",
                        this.water_speed,
                        recent.water_speed
                    );
                    dorep!(rep, KIND_PAWN, "AirSpeed", this.air_speed, recent.air_speed);
                    dorep!(
                        rep,
                        KIND_PAWN,
                        "AccelRate",
                        this.accel_rate,
                        recent.accel_rate
                    );
                    dorep!(rep, KIND_PAWN, "JumpZ", this.jump_z, recent.jump_z);
                    dorep!(
                        rep,
                        KIND_PAWN,
                        "AirControl",
                        this.air_control,
                        recent.air_control
                    );
                    dorep!(
                        rep,
                        KIND_PAWN,
                        "bBehindView",
                        this.b_behind_view(),
                        recent.b_behind_view()
                    );
                }
            }
        } else if this.base.role == ROLE_AUTHORITY {
            dorep!(
                rep,
                KIND_ACTOR,
                "bHidden",
                this.base.b_hidden(),
                recent.base.b_hidden()
            );
            if this.base.remote_role <= ROLE_SIMULATED_PROXY {
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "Location",
                    this.base.location,
                    recent.base.location
                );
                if this.base.draw_type == DT_MESH || this.base.draw_type == DT_BRUSH {
                    dorep!(
                        rep,
                        KIND_ACTOR,
                        "Rotation",
                        this.base.rotation,
                        recent.base.rotation
                    );
                }
            }
            if this.base.draw_type == DT_MESH {
                if this.base.remote_role <= ROLE_SIMULATED_PROXY || this.base.b_demo_recording() {
                    dorep!(
                        rep,
                        KIND_ACTOR,
                        "AnimSequence",
                        this.base.anim_sequence,
                        recent.base.anim_sequence
                    );
                    dorep!(
                        rep,
                        KIND_ACTOR,
                        "SimAnim",
                        this.base.sim_anim,
                        recent.base.sim_anim
                    );
                    dorep!(
                        rep,
                        KIND_ACTOR,
                        "AnimMinRate",
                        this.base.anim_min_rate,
                        recent.base.anim_min_rate
                    );
                    dorep!(
                        rep,
                        KIND_ACTOR,
                        "bAnimNotify",
                        this.base.b_anim_notify(),
                        recent.base.b_anim_notify()
                    );
                }
            } else if this.base.draw_type == DT_SPRITE
                && !this.base.b_hidden()
                && !this.base.b_only_owner_see()
            {
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "Sprite",
                    this.base.sprite,
                    recent.base.sprite
                );
            }
            if this.base.remote_role == ROLE_SIMULATED_PROXY {
                dorep!(rep, KIND_ACTOR, "Base", this.base.base, recent.base.base);
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "Velocity",
                    this.base.velocity,
                    recent.base.velocity
                );
            } else if this.base.b_sim_fall() {
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "Physics",
                    this.base.physics,
                    recent.base.physics
                );
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "Acceleration",
                    this.base.acceleration,
                    recent.base.acceleration
                );
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "bBounce",
                    this.base.b_bounce(),
                    recent.base.b_bounce()
                );
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "Velocity",
                    this.base.velocity,
                    recent.base.velocity
                );
            }
        }
        if this.base.role == ROLE_AUTHORITY {
            dorep!(rep, KIND_PAWN, "Weapon", this.weapon, recent.weapon);
            dorep!(
                rep,
                KIND_PAWN,
                "bCanFly",
                this.b_can_fly(),
                recent.b_can_fly()
            );
        }
        if (this.base.b_net_initial() && this.base.b_net_owner() && this.b_is_player())
            || this.base.b_demo_recording()
        {
            dorep!(
                rep,
                KIND_PAWN,
                "ViewRotation",
                this.view_rotation,
                recent.view_rotation
            );
        }
        if this.base.b_demo_recording() {
            dorep!(
                rep,
                KIND_PAWN,
                "EyeHeight",
                this.eye_height,
                recent.eye_height
            );
        }
    } else {
        actor_get_optimized_rep_list(rep, &this.base, &recent.base);
    }
}

/// APlayerPawn::GetOptimizedRepList.
pub fn player_pawn_get_optimized_rep_list(
    rep: &mut Rep<'_>,
    this: &APlayerPawn,
    recent: &APlayerPawn,
    num_reps: i32,
) {
    pawn_get_optimized_rep_list(rep, &this.base, &recent.base, num_reps);
    if !is_native(rep, KIND_PLAYER_PAWN) {
        return;
    }
    if this.base.base.role == ROLE_AUTHORITY {
        if this.base.base.b_net_owner() {
            dorep!(
                rep,
                KIND_PLAYER_PAWN,
                "ViewTarget",
                this.view_target,
                recent.view_target
            );
            dorep!(
                rep,
                KIND_PLAYER_PAWN,
                "ScoringType",
                this.scoring_type,
                recent.scoring_type
            );
            dorep!(
                rep,
                KIND_PLAYER_PAWN,
                "HUDType",
                this.hud_type,
                recent.hud_type
            );
            dorep!(
                rep,
                KIND_PLAYER_PAWN,
                "GameReplicationInfo",
                this.game_replication_info,
                recent.game_replication_info
            );
            dorep!(
                rep,
                KIND_PLAYER_PAWN,
                "bFixedCamera",
                this.b_fixed_camera(),
                recent.b_fixed_camera()
            );
            dorep!(
                rep,
                KIND_PLAYER_PAWN,
                "bNeverAutoSwitch",
                this.b_never_auto_switch(),
                recent.b_never_auto_switch()
            );
            dorep!(
                rep,
                KIND_PLAYER_PAWN,
                "bCheatsEnabled",
                this.b_cheats_enabled(),
                recent.b_cheats_enabled()
            );
            dorep!(
                rep,
                KIND_PLAYER_PAWN,
                "TargetViewRotation",
                this.target_view_rotation,
                recent.target_view_rotation
            );
            dorep!(
                rep,
                KIND_PLAYER_PAWN,
                "TargetEyeHeight",
                this.target_eye_height,
                recent.target_eye_height
            );
            dorep!(
                rep,
                KIND_PLAYER_PAWN,
                "TargetWeaponViewOffset",
                this.target_weapon_view_offset,
                recent.target_weapon_view_offset
            );
        }
        if this.base.base.b_demo_recording() {
            dorep!(
                rep,
                KIND_PLAYER_PAWN,
                "DemoViewPitch",
                this.demo_view_pitch,
                recent.demo_view_pitch
            );
            dorep!(
                rep,
                KIND_PLAYER_PAWN,
                "DemoViewYaw",
                this.demo_view_yaw,
                recent.demo_view_yaw
            );
        }
    } else {
        dorep!(
            rep,
            KIND_PLAYER_PAWN,
            "Password",
            this.password,
            recent.password
        );
        dorep!(
            rep,
            KIND_PLAYER_PAWN,
            "bReadyToPlay",
            this.b_ready_to_play(),
            recent.b_ready_to_play()
        );
    }
}

/// AMover::GetOptimizedRepList.
pub fn mover_get_optimized_rep_list(rep: &mut Rep<'_>, this: &AMover, recent: &AMover) {
    actor_get_optimized_rep_list(rep, &this.base, &recent.base);
    if !is_native(rep, KIND_MOVER) || this.base.role != ROLE_AUTHORITY {
        return;
    }
    dorep!(
        rep,
        KIND_MOVER,
        "SimOldPos",
        this.sim_old_pos,
        recent.sim_old_pos
    );
    dorep!(
        rep,
        KIND_MOVER,
        "SimOldRotPitch",
        this.sim_old_rot_pitch,
        recent.sim_old_rot_pitch
    );
    dorep!(
        rep,
        KIND_MOVER,
        "SimOldRotYaw",
        this.sim_old_rot_yaw,
        recent.sim_old_rot_yaw
    );
    dorep!(
        rep,
        KIND_MOVER,
        "SimOldRotRoll",
        this.sim_old_rot_roll,
        recent.sim_old_rot_roll
    );
    dorep!(
        rep,
        KIND_MOVER,
        "SimInterpolate",
        this.sim_interpolate,
        recent.sim_interpolate
    );
    dorep!(
        rep,
        KIND_MOVER,
        "RealPosition",
        this.real_position,
        recent.real_position
    );
    dorep!(
        rep,
        KIND_MOVER,
        "RealRotation",
        this.real_rotation,
        recent.real_rotation
    );
}

/// AZoneInfo::GetOptimizedRepList.
pub fn zone_info_get_optimized_rep_list(rep: &mut Rep<'_>, this: &AZoneInfo, recent: &AZoneInfo) {
    if this.base.base.b_net_initial() {
        actor_get_optimized_rep_list(rep, &this.base.base, &recent.base.base);
    }
    if !is_native(rep, KIND_ZONE_INFO) || this.base.base.role != ROLE_AUTHORITY {
        return;
    }
    dorep!(
        rep,
        KIND_ZONE_INFO,
        "ZoneGravity",
        this.zone_gravity,
        recent.zone_gravity
    );
    dorep!(
        rep,
        KIND_ZONE_INFO,
        "ZoneVelocity",
        this.zone_velocity,
        recent.zone_velocity
    );
    dorep!(
        rep,
        KIND_ZONE_INFO,
        "AmbientBrightness",
        this.ambient_brightness,
        recent.ambient_brightness
    );
    dorep!(
        rep,
        KIND_ZONE_INFO,
        "AmbientHue",
        this.ambient_hue,
        recent.ambient_hue
    );
    dorep!(
        rep,
        KIND_ZONE_INFO,
        "AmbientSaturation",
        this.ambient_saturation,
        recent.ambient_saturation
    );
    dorep!(
        rep,
        KIND_ZONE_INFO,
        "TexUPanSpeed",
        this.tex_u_pan_speed,
        recent.tex_u_pan_speed
    );
    dorep!(
        rep,
        KIND_ZONE_INFO,
        "TexVPanSpeed",
        this.tex_v_pan_speed,
        recent.tex_v_pan_speed
    );
    dorep!(
        rep,
        KIND_ZONE_INFO,
        "bReverbZone",
        this.b_reverb_zone(),
        recent.b_reverb_zone()
    );
    dorep!(
        rep,
        KIND_ZONE_INFO,
        "FogColor",
        this.fog_color,
        recent.fog_color
    );
    dorep!(
        rep,
        KIND_ACTOR,
        "Role",
        this.base.base.role,
        recent.base.base.role
    );
    dorep!(
        rep,
        KIND_ACTOR,
        "RemoteRole",
        this.base.base.remote_role,
        recent.base.base.remote_role
    );
    if !this.base.base.b_net_owner() || !this.base.base.b_client_anim() {
        dorep!(
            rep,
            KIND_ACTOR,
            "AmbientSound",
            this.base.base.ambient_sound,
            recent.base.base.ambient_sound
        );
        if !this.base.base.ambient_sound.is_null() {
            dorep!(
                rep,
                KIND_ACTOR,
                "SoundRadius",
                this.base.base.sound_radius,
                recent.base.base.sound_radius
            );
            dorep!(
                rep,
                KIND_ACTOR,
                "SoundVolume",
                this.base.base.sound_volume,
                recent.base.base.sound_volume
            );
            dorep!(
                rep,
                KIND_ACTOR,
                "SoundPitch",
                this.base.base.sound_pitch,
                recent.base.base.sound_pitch
            );
        }
    }
}

/// ALevelInfo::GetOptimizedRepList.
pub fn level_info_get_optimized_rep_list(
    rep: &mut Rep<'_>,
    this: &ALevelInfo,
    recent: &ALevelInfo,
) {
    zone_info_get_optimized_rep_list(rep, &this.base, &recent.base);
    if !is_native(rep, KIND_LEVEL_INFO) || this.base.base.base.role != ROLE_AUTHORITY {
        return;
    }
    dorep!(rep, KIND_LEVEL_INFO, "Pauser", this.pauser, recent.pauser);
    dorep!(
        rep,
        KIND_LEVEL_INFO,
        "TimeDilation",
        this.time_dilation,
        recent.time_dilation
    );
    dorep!(
        rep,
        KIND_LEVEL_INFO,
        "bNoCheating",
        this.b_no_cheating(),
        recent.b_no_cheating()
    );
    dorep!(
        rep,
        KIND_LEVEL_INFO,
        "bAllowFOV",
        this.b_allow_fov(),
        recent.b_allow_fov()
    );
}

/// APlayerReplicationInfo::GetOptimizedRepList.
pub fn player_replication_info_get_optimized_rep_list(
    rep: &mut Rep<'_>,
    this: &APlayerReplicationInfo,
    recent: &APlayerReplicationInfo,
) {
    if this.base.base.base.b_net_initial() {
        actor_get_optimized_rep_list(rep, &this.base.base.base, &recent.base.base.base);
    }
    if !is_native(rep, KIND_PLAYER_REPLICATION_INFO) || this.base.base.base.role != ROLE_AUTHORITY {
        return;
    }
    let k = KIND_PLAYER_REPLICATION_INFO;
    dorep!(rep, k, "PlayerName", this.player_name, recent.player_name);
    dorep!(rep, k, "OldName", this.old_name, recent.old_name);
    dorep!(rep, k, "PlayerID", this.player_id, recent.player_id);
    dorep!(rep, k, "TeamName", this.team_name, recent.team_name);
    dorep!(rep, k, "Team", this.team, recent.team);
    dorep!(rep, k, "TeamID", this.team_id, recent.team_id);
    dorep!(rep, k, "Score", this.score, recent.score);
    dorep!(rep, k, "Deaths", this.deaths, recent.deaths);
    dorep!(rep, k, "VoiceType", this.voice_type, recent.voice_type);
    dorep!(rep, k, "HasFlag", this.has_flag, recent.has_flag);
    dorep!(rep, k, "Ping", this.ping, recent.ping);
    dorep!(rep, k, "PacketLoss", this.packet_loss, recent.packet_loss);
    dorep!(
        rep,
        k,
        "bIsFemale",
        this.b_is_female(),
        recent.b_is_female()
    );
    dorep!(rep, k, "bIsABot", this.b_is_a_bot(), recent.b_is_a_bot());
    dorep!(
        rep,
        k,
        "bFeigningDeath",
        this.b_feigning_death(),
        recent.b_feigning_death()
    );
    dorep!(
        rep,
        k,
        "bIsSpectator",
        this.b_is_spectator(),
        recent.b_is_spectator()
    );
    dorep!(
        rep,
        k,
        "bWaitingPlayer",
        this.b_waiting_player(),
        recent.b_waiting_player()
    );
    dorep!(rep, k, "bAdmin", this.b_admin(), recent.b_admin());
    dorep!(
        rep,
        k,
        "TalkTexture",
        this.talk_texture,
        recent.talk_texture
    );
    dorep!(rep, k, "PlayerZone", this.player_zone, recent.player_zone);
    dorep!(
        rep,
        k,
        "PlayerLocation",
        this.player_location,
        recent.player_location
    );
    dorep!(rep, k, "StartTime", this.start_time, recent.start_time);
}

/// AGameReplicationInfo::GetOptimizedRepList.
pub fn game_replication_info_get_optimized_rep_list(
    rep: &mut Rep<'_>,
    this: &AGameReplicationInfo,
    recent: &AGameReplicationInfo,
) {
    if this.base.base.base.b_net_initial() {
        actor_get_optimized_rep_list(rep, &this.base.base.base, &recent.base.base.base);
    }
    if !is_native(rep, KIND_GAME_REPLICATION_INFO) || this.base.base.base.role != ROLE_AUTHORITY {
        return;
    }
    let k = KIND_GAME_REPLICATION_INFO;
    dorep!(
        rep,
        k,
        "RemainingMinute",
        this.remaining_minute,
        recent.remaining_minute
    );
    dorep!(
        rep,
        k,
        "bStopCountDown",
        this.b_stop_count_down(),
        recent.b_stop_count_down()
    );
    dorep!(
        rep,
        k,
        "GameEndedComments",
        this.game_ended_comments,
        recent.game_ended_comments
    );
    dorep!(rep, k, "NumPlayers", this.num_players, recent.num_players);
    if this.base.base.base.b_net_initial() {
        dorep!(rep, k, "GameName", this.game_name, recent.game_name);
        dorep!(rep, k, "GameClass", this.game_class, recent.game_class);
        dorep!(
            rep,
            k,
            "bTeamGame",
            this.b_team_game(),
            recent.b_team_game()
        );
        dorep!(
            rep,
            k,
            "RemainingTime",
            this.remaining_time,
            recent.remaining_time
        );
        dorep!(
            rep,
            k,
            "ElapsedTime",
            this.elapsed_time,
            recent.elapsed_time
        );
        dorep!(rep, k, "ServerName", this.server_name, recent.server_name);
        dorep!(rep, k, "ShortName", this.short_name, recent.short_name);
        dorep!(rep, k, "AdminName", this.admin_name, recent.admin_name);
        dorep!(rep, k, "AdminEmail", this.admin_email, recent.admin_email);
        dorep!(rep, k, "MOTDLine1", this.motd_line1, recent.motd_line1);
        dorep!(rep, k, "MOTDLine2", this.motd_line2, recent.motd_line2);
        dorep!(rep, k, "MOTDLine3", this.motd_line3, recent.motd_line3);
        dorep!(rep, k, "MOTDLine4", this.motd_line4, recent.motd_line4);
    }
}

/// AInventory::GetOptimizedRepList.
pub fn inventory_get_optimized_rep_list(
    rep: &mut Rep<'_>,
    this: &AInventory,
    recent: &AInventory,
    num_reps: i32,
) {
    if this.base.b_always_relevant() && !this.base.b_net_initial() {
        dorep!(
            rep,
            KIND_ACTOR,
            "bHidden",
            this.base.b_hidden(),
            recent.base.b_hidden()
        );
        return;
    }
    if is_native(rep, KIND_INVENTORY) {
        if this.base.b_net_initial() || num_reps % 5 == 0 {
            actor_get_optimized_rep_list(rep, &this.base, &recent.base);
            if this.base.b_net_owner() {
                dorep!(
                    rep,
                    KIND_INVENTORY,
                    "bIsAnArmor",
                    this.b_is_an_armor(),
                    recent.b_is_an_armor()
                );
                dorep!(rep, KIND_INVENTORY, "Charge", this.charge, recent.charge);
                dorep!(
                    rep,
                    KIND_INVENTORY,
                    "bActivatable",
                    this.b_activatable(),
                    recent.b_activatable()
                );
                dorep!(
                    rep,
                    KIND_INVENTORY,
                    "bActive",
                    this.b_active(),
                    recent.b_active()
                );
                dorep!(
                    rep,
                    KIND_INVENTORY,
                    "PlayerViewOffset",
                    this.player_view_offset,
                    recent.player_view_offset
                );
                dorep!(
                    rep,
                    KIND_INVENTORY,
                    "PlayerViewMesh",
                    this.player_view_mesh,
                    recent.player_view_mesh
                );
                dorep!(
                    rep,
                    KIND_INVENTORY,
                    "PlayerViewScale",
                    this.player_view_scale,
                    recent.player_view_scale
                );
            }
        } else if this.base.role == ROLE_AUTHORITY {
            dorep!(
                rep,
                KIND_ACTOR,
                "LightType",
                this.base.light_type,
                recent.base.light_type
            );
            dorep!(
                rep,
                KIND_ACTOR,
                "bCollideWorld",
                this.base.b_collide_world(),
                recent.base.b_collide_world()
            );
            dorep!(
                rep,
                KIND_ACTOR,
                "bHidden",
                this.base.b_hidden(),
                recent.base.b_hidden()
            );
            if this.base.b_net_owner() {
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "Inventory",
                    this.base.inventory,
                    recent.base.inventory
                );
            }
            if !this.base.b_carried_item() && this.base.remote_role < ROLE_SIMULATED_PROXY {
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "Location",
                    this.base.location,
                    recent.base.location
                );
                if this.base.draw_type == DT_MESH || this.base.draw_type == DT_BRUSH {
                    dorep!(
                        rep,
                        KIND_ACTOR,
                        "Rotation",
                        this.base.rotation,
                        recent.base.rotation
                    );
                }
            }
            if this.base.draw_type == DT_MESH {
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "bUnlit",
                    this.base.b_unlit(),
                    recent.base.b_unlit()
                );
                if ((this.base.remote_role <= ROLE_SIMULATED_PROXY)
                    && (!this.base.b_net_owner() || !this.base.b_client_anim()))
                    || this.base.b_demo_recording()
                {
                    dorep!(
                        rep,
                        KIND_ACTOR,
                        "AnimSequence",
                        this.base.anim_sequence,
                        recent.base.anim_sequence
                    );
                    dorep!(
                        rep,
                        KIND_ACTOR,
                        "SimAnim",
                        this.base.sim_anim,
                        recent.base.sim_anim
                    );
                    dorep!(
                        rep,
                        KIND_ACTOR,
                        "AnimMinRate",
                        this.base.anim_min_rate,
                        recent.base.anim_min_rate
                    );
                    dorep!(
                        rep,
                        KIND_ACTOR,
                        "bAnimNotify",
                        this.base.b_anim_notify(),
                        recent.base.b_anim_notify()
                    );
                }
            } else if this.base.draw_type == DT_SPRITE
                && !this.base.b_hidden()
                && (!this.base.b_only_owner_see() || this.base.b_net_owner())
            {
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "Sprite",
                    this.base.sprite,
                    recent.base.sprite
                );
            }
            if this.base.light_type != LT_NONE {
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "LightEffect",
                    this.base.light_effect,
                    recent.base.light_effect
                );
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "LightBrightness",
                    this.base.light_brightness,
                    recent.base.light_brightness
                );
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "LightHue",
                    this.base.light_hue,
                    recent.base.light_hue
                );
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "LightSaturation",
                    this.base.light_saturation,
                    recent.base.light_saturation
                );
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "LightRadius",
                    this.base.light_radius,
                    recent.base.light_radius
                );
            }
            if this.base.remote_role == ROLE_SIMULATED_PROXY {
                dorep!(rep, KIND_ACTOR, "Base", this.base.base, recent.base.base);
            } else if this.base.b_sim_fall() {
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "Physics",
                    this.base.physics,
                    recent.base.physics
                );
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "Acceleration",
                    this.base.acceleration,
                    recent.base.acceleration
                );
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "bBounce",
                    this.base.b_bounce(),
                    recent.base.b_bounce()
                );
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "Velocity",
                    this.base.velocity,
                    recent.base.velocity
                );
            }
            if this.base.b_net_owner() && instigator_weapon_is(this) {
                dorep!(
                    rep,
                    KIND_INVENTORY,
                    "PlayerViewOffset",
                    this.player_view_offset,
                    recent.player_view_offset
                );
                dorep!(
                    rep,
                    KIND_INVENTORY,
                    "PlayerViewMesh",
                    this.player_view_mesh,
                    recent.player_view_mesh
                );
                dorep!(
                    rep,
                    KIND_INVENTORY,
                    "PlayerViewScale",
                    this.player_view_scale,
                    recent.player_view_scale
                );
            }
        }
        if this.base.role == ROLE_AUTHORITY {
            if !this.base.b_net_owner()
                && this.base.remote_role == ROLE_SIMULATED_PROXY
                && !this.base.ambient_sound.is_null()
            {
                dorep!(
                    rep,
                    KIND_ACTOR,
                    "Location",
                    this.base.location,
                    recent.base.location
                );
            }
            dorep!(
                rep,
                KIND_INVENTORY,
                "FlashCount",
                this.flash_count,
                recent.flash_count
            );
            dorep!(
                rep,
                KIND_INVENTORY,
                "bSteadyFlash3rd",
                this.b_steady_flash3rd(),
                recent.b_steady_flash3rd()
            );
            dorep!(
                rep,
                KIND_INVENTORY,
                "ThirdPersonMesh",
                this.third_person_mesh,
                recent.third_person_mesh
            );
            dorep!(
                rep,
                KIND_INVENTORY,
                "ThirdPersonScale",
                this.third_person_scale,
                recent.third_person_scale
            );
        }
    } else {
        actor_get_optimized_rep_list(rep, &this.base, &recent.base);
    }
}

/// `bNetOwner && Instigator && (Instigator->Weapon == this)`. `Instigator` is an
/// APawn, so it is read as one; the C++ does the same.
fn instigator_weapon_is(this: &AInventory) -> bool {
    let instigator = this.base.instigator;
    if instigator.is_null() {
        return false;
    }
    // SAFETY: a non-null Instigator is an APawn in the engine's object graph, the same
    // object the C++ dereferences.
    let weapon = unsafe { (*instigator.cast::<APawn>()).weapon };
    weapon.cast_const() == ptr::from_ref(this).cast()
}

/// AInventory::ShouldDoScriptReplication.
pub fn inventory_should_do_script_replication(this: &AInventory) -> bool {
    this.base.b_net_owner() || this.base.b_net_initial()
}

/// AInventory::NoVariablesToReplicate.
pub fn inventory_no_variables_to_replicate(this: &AInventory, old_ver: &AActor) -> bool {
    this.base.b_hidden() == old_ver.b_hidden()
}

/// AInventory::UpdateFrequency.
pub fn inventory_update_frequency(
    svc: &Hp2ActorRepServices,
    this: &AInventory,
    viewer: *const c_void,
) -> f32 {
    let owner = this.base.owner;
    if this.base.b_hidden() && !owner.is_null() {
        // SAFETY: the service is provided by the linked C++ side.
        if unsafe { (svc.object_is_a)(owner.cast_const(), KIND_PAWN) } != 0 {
            // SAFETY: Owner passed the IsA(APawn) check above.
            let weapon = unsafe { (*owner.cast::<APawn>()).weapon };
            if weapon.cast_const() != ptr::from_ref(this).cast() {
                return min_f32(this.base.net_update_frequency, 4.0);
            }
            if owner.cast_const() != viewer {
                return min_f32(this.base.net_update_frequency, 10.0);
            }
        }
    }
    this.base.net_update_frequency
}

/// AWeapon::GetOptimizedRepList.
pub fn weapon_get_optimized_rep_list(
    rep: &mut Rep<'_>,
    this: &AWeapon,
    recent: &AWeapon,
    num_reps: i32,
) {
    if this.base.base.b_always_relevant() && !this.base.base.b_net_initial() {
        dorep!(
            rep,
            KIND_ACTOR,
            "bHidden",
            this.base.base.b_hidden(),
            recent.base.base.b_hidden()
        );
        return;
    }
    inventory_get_optimized_rep_list(rep, &this.base, &recent.base, num_reps);
    if is_native(rep, KIND_WEAPON)
        && this.base.base.b_net_owner()
        && this.base.base.role == ROLE_AUTHORITY
    {
        dorep!(
            rep,
            KIND_WEAPON,
            "AmmoType",
            this.ammo_type,
            recent.ammo_type
        );
        dorep!(
            rep,
            KIND_WEAPON,
            "bLockedOn",
            this.b_locked_on(),
            recent.b_locked_on()
        );
        dorep!(
            rep,
            KIND_WEAPON,
            "bHideWeapon",
            this.b_hide_weapon(),
            recent.b_hide_weapon()
        );
    }
}

/// AAmmo::GetOptimizedRepList.
pub fn ammo_get_optimized_rep_list(rep: &mut Rep<'_>, this: &AAmmo, recent: &AAmmo, num_reps: i32) {
    if this.base.base.base.b_always_relevant() && !this.base.base.base.b_net_initial() {
        dorep!(
            rep,
            KIND_ACTOR,
            "bHidden",
            this.base.base.base.b_hidden(),
            recent.base.base.base.b_hidden()
        );
        return;
    }
    pickup_get_optimized_rep_list(rep, &this.base, &recent.base, num_reps);
    if is_native(rep, KIND_AMMO)
        && this.base.base.base.b_net_owner()
        && this.base.base.base.role == ROLE_AUTHORITY
    {
        dorep!(
            rep,
            KIND_AMMO,
            "AmmoAmount",
            this.ammo_amount,
            recent.ammo_amount
        );
    }
}

/// APickup::GetOptimizedRepList.
pub fn pickup_get_optimized_rep_list(
    rep: &mut Rep<'_>,
    this: &APickup,
    recent: &APickup,
    num_reps: i32,
) {
    if this.base.base.b_always_relevant() && !this.base.base.b_net_initial() {
        dorep!(
            rep,
            KIND_ACTOR,
            "bHidden",
            this.base.base.b_hidden(),
            recent.base.base.b_hidden()
        );
        return;
    }
    inventory_get_optimized_rep_list(rep, &this.base, &recent.base, num_reps);
    if is_native(rep, KIND_PICKUP)
        && this.base.base.b_net_owner()
        && this.base.base.role == ROLE_AUTHORITY
    {
        dorep!(
            rep,
            KIND_PICKUP,
            "NumCopies",
            this.num_copies,
            recent.num_copies
        );
    }
}
