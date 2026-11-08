//! Movement and interpolation policy from `Engine/Src/UnPhysic.cpp`.
//! Throwing engine operations return checked status through `Hp2PhysicServices`;
//! policy propagates the first error without further mutation or callbacks.
//! Family transitions reenter the qualified native member before returning to
//! Rust, preserving native guard nesting, including recursive waterline searches.
//! Callback-mutable actors, interpolation points and native hits remain raw pointers.
//! Only the immutable GMath sine/cosine lookup callbacks are infallible.
pub mod dispatch;
pub mod fall;
pub mod interp;
pub mod motion;
pub mod swim;
pub mod walk;

use std::ffi::c_void;

use hp2_abi::engine::generated::{
    AActor, ACarcass, ADecoration, AInterpolationPoint, APawn, AProjectile, AZoneInfo,
    FCheckResult, FRotator,
};
use hp2_abi::{FCoords, FVector};

use crate::actor::Hp2ActorServices;

/// Class ids for `Hp2PhysicServices::is_a` (`Hp2PhysicClass` in C++).
const CLASS_PAWN: i32 = 0;
const CLASS_PLAYER_PAWN: i32 = 1;
const CLASS_DECORATION: i32 = 2;
const CLASS_CARCASS: i32 = 3;

/// Trace kinds for `Hp2PhysicServices::single_line_check` (`Hp2PhysicTrace`).
const TRACE_VIS_BLOCKING: i32 = 0;
const TRACE_ALL_BLOCKING: i32 = 1;

/// `EPhysics` values (`EngineClasses.h`).
const PHYS_NONE: u8 = 0;
const PHYS_WALKING: u8 = 1;
const PHYS_FALLING: u8 = 2;

/// `SMALL_NUMBER` from `UnMath.h`.
const SMALL_NUMBER: f32 = 1.0e-8;

/// A native leaf captured an exception; no further policy work may run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicsError(pub i32);
pub type PhysicsResult<T> = Result<T, PhysicsError>;

fn physics_status(status: i32) -> PhysicsResult<()> {
    if status == 0 {
        Ok(())
    } else {
        Err(PhysicsError(status))
    }
}

/// Checked engine leaves and qualified native member routes.
/// Field order matches `Hp2PhysicServices.h`; zero status means success.
#[repr(C)]
pub struct Hp2PhysicServices {
    pub is_a: unsafe extern "C" fn(obj: *mut c_void, class: i32, result: *mut i32) -> i32,
    pub is_probing_hit_wall: unsafe extern "C" fn(actor: *mut c_void, result: *mut i32) -> i32,
    pub app_frand: unsafe extern "C" fn(result: *mut f32) -> i32,
    pub bsp_surf_special:
        unsafe extern "C" fn(hit: *const FCheckResult, radius: f32, result: *mut i32) -> i32,
    pub rotation_vector: unsafe extern "C" fn(rot: *const FRotator, out: *mut FVector) -> i32,
    pub move_actor: unsafe extern "C" fn(
        actor: *mut c_void,
        delta: *const FVector,
        rot: *const FRotator,
        hit: *mut FCheckResult,
        result: *mut i32,
    ) -> i32,
    pub single_line_check: unsafe extern "C" fn(
        actor: *mut c_void,
        hit: *mut FCheckResult,
        end: *const FVector,
        start: *const FVector,
        trace: i32,
        extent: *const FVector,
        result: *mut i32,
    ) -> i32,
    pub event_mount: unsafe extern "C" fn(actor: *mut c_void, dir: *const FVector) -> i32,
    pub event_hit_wall: unsafe extern "C" fn(
        actor: *mut c_void,
        normal: *const FVector,
        hit_actor: *mut c_void,
    ) -> i32,
    pub event_landed: unsafe extern "C" fn(actor: *mut c_void, normal: *const FVector) -> i32,
    pub point_region_zone: unsafe extern "C" fn(
        actor: *mut c_void,
        location: *const FVector,
        result: *mut *mut c_void,
    ) -> i32,
    pub debugf_flew_out: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub destroy_actor: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub event_falling: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub log_fell_out_of_world: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub event_fell_out_of_world: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub rotation_from_vector:
        unsafe extern "C" fn(vector: *const FVector, out: *mut FRotator) -> i32,
    pub find_view_target: unsafe extern "C" fn(
        actor: *mut c_void,
        tag: *const [u8; 4],
        result: *mut *mut c_void,
    ) -> i32,
    pub event_update_camera: unsafe extern "C" fn(manager: *mut c_void, old_alpha: f32) -> i32,
    pub event_interpolate_end:
        unsafe extern "C" fn(point: *mut c_void, manager: *mut c_void, forward: i32) -> i32,
    pub event_finished_interpolation: unsafe extern "C" fn(manager: *mut c_void) -> i32,
    pub event_key_frame_reached: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub mover_freeze_log: unsafe extern "C" fn(
        actor: *mut c_void,
        phys_alpha: f32,
        prev_key_num: i32,
        key_num: i32,
        hit_actor: *mut c_void,
    ) -> i32,
    pub event_post_touch: unsafe extern "C" fn(touched: *mut c_void, toucher: *mut c_void) -> i32,
    pub is_probing_ended_rotation:
        unsafe extern "C" fn(actor: *mut c_void, result: *mut i32) -> i32,
    pub event_ended_rotation: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub sin_tab: unsafe extern "C" fn(i: i32) -> f32,
    pub cos_tab: unsafe extern "C" fn(i: i32) -> f32,
    pub is_probing_may_fall: unsafe extern "C" fn(actor: *mut c_void, result: *mut i32) -> i32,
    pub event_may_fall: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub event_do_jump: unsafe extern "C" fn(actor: *mut c_void, value: f32) -> i32,
    pub far_move_actor: unsafe extern "C" fn(actor: *mut c_void, location: *const FVector) -> i32,
    pub debugf_fell_out: unsafe extern "C" fn(actor: *mut c_void) -> i32,
    pub single_line_check_no_source: unsafe extern "C" fn(
        actor: *mut c_void,
        hit: *mut FCheckResult,
        end: *const FVector,
        start: *const FVector,
        trace: i32,
        extent: *const FVector,
        result: *mut i32,
    ) -> i32,
    pub mesh_bone_coords: unsafe extern "C" fn(
        mesh: *mut c_void,
        owner: *mut c_void,
        bone: i32,
        out: *mut c_void,
    ) -> i32,
    pub trailer_far_move_actor:
        unsafe extern "C" fn(actor: *mut c_void, location: *const FVector) -> i32,
    pub call_calc_velocity: unsafe extern "C" fn(
        pawn: *mut APawn,
        accel_dir: *const FVector,
        delta_time: f32,
        max_speed: f32,
        friction: f32,
        b_fluid: i32,
        b_brake: i32,
        b_buoyant: i32,
        services: *const Hp2PhysicServices,
    ) -> i32,
    pub call_mount: unsafe extern "C" fn(
        pawn: *mut APawn,
        mount_dir: *const FVector,
        hit: *mut FCheckResult,
        services: *const Hp2PhysicServices,
        result: *mut i32,
    ) -> i32,
    pub call_step_up: unsafe extern "C" fn(
        pawn: *mut APawn,
        grav_dir: *const FVector,
        desired_dir: *const FVector,
        delta: *const FVector,
        hit: *mut FCheckResult,
        services: *const Hp2PhysicServices,
    ) -> i32,
    pub call_process_hit_wall: unsafe extern "C" fn(
        this: *mut AActor,
        hit_normal: *const FVector,
        hit_actor: *mut c_void,
        services: *const Hp2PhysicServices,
    ) -> i32,
    pub call_process_landed: unsafe extern "C" fn(
        this: *mut AActor,
        hit_normal: *const FVector,
        hit_actor: *mut c_void,
        remaining_time: f32,
        iterations: i32,
        services: *const Hp2PhysicServices,
    ) -> i32,
    pub call_find_base:
        unsafe extern "C" fn(this: *mut AActor, services: *const Hp2PhysicServices) -> i32,
    pub call_phys_trailer: unsafe extern "C" fn(
        this: *mut AActor,
        _delta_time: f32,
        services: *const Hp2PhysicServices,
    ) -> i32,
    pub call_phys_moving_brush: unsafe extern "C" fn(
        this: *mut AActor,
        delta_time: f32,
        services: *const Hp2PhysicServices,
    ) -> i32,
    pub call_physics_rotation_actor: unsafe extern "C" fn(
        this: *mut AActor,
        delta_time: f32,
        services: *const Hp2PhysicServices,
    ) -> i32,
    pub call_physics_rotation_pawn: unsafe extern "C" fn(
        this: *mut APawn,
        delta_time: f32,
        old_velocity: *const FVector,
        services: *const Hp2PhysicServices,
    ) -> i32,
    pub call_get_desired_rotation_at_pause: unsafe extern "C" fn(
        this: *mut AInterpolationPoint,
        pause_num: i32,
        new_location: *const FVector,
        services: *const Hp2PhysicServices,
        result: *mut FRotator,
    ) -> i32,
    pub call_get_desired_rotation_at_position: unsafe extern "C" fn(
        this: *mut AInterpolationPoint,
        pos_offset: i32,
        pause_num: i32,
        new_location: *const FVector,
        services: *const Hp2PhysicServices,
        result: *mut FRotator,
    ) -> i32,
    pub call_get_interpolated_position: unsafe extern "C" fn(
        this: *mut AInterpolationPoint,
        old_coords: *const FCoords,
        start_control: *const FVector,
        phys_alpha: f32,
        pause_num: i32,
        services: *const Hp2PhysicServices,
        result: *mut FCoords,
    ) -> i32,
    pub call_start_swimming: unsafe extern "C" fn(
        this: *mut APawn,
        old_velocity: *const FVector,
        time_tick: f32,
        remaining_time: f32,
        iterations: i32,
        services: *const Hp2PhysicServices,
    ) -> i32,
    pub call_phys_flying: unsafe extern "C" fn(
        this: *mut APawn,
        delta_time: f32,
        iterations: i32,
        services: *const Hp2PhysicServices,
    ) -> i32,
    pub call_swim: unsafe extern "C" fn(
        this: *mut APawn,
        delta: *const FVector,
        hit: *mut FCheckResult,
        services: *const Hp2PhysicServices,
        result: *mut f32,
    ) -> i32,
    pub call_find_water_line: unsafe extern "C" fn(
        this: *mut APawn,
        start: *const FVector,
        end: *mut FVector,
        services: *const Hp2PhysicServices,
    ) -> i32,
    pub call_phys_swimming: unsafe extern "C" fn(
        this: *mut APawn,
        delta_time: f32,
        iterations: i32,
        services: *const Hp2PhysicServices,
    ) -> i32,
    pub call_phys_falling: unsafe extern "C" fn(
        this: *mut AActor,
        delta_time: f32,
        iterations: i32,
        services: *const Hp2PhysicServices,
    ) -> i32,
    pub call_phys_projectile: unsafe extern "C" fn(
        this: *mut AActor,
        delta_time: f32,
        iterations: i32,
        services: *const Hp2PhysicServices,
    ) -> i32,
    pub call_phys_rolling: unsafe extern "C" fn(
        this: *mut AActor,
        delta_time: f32,
        iterations: i32,
        services: *const Hp2PhysicServices,
    ) -> i32,
    pub call_check_floor: unsafe extern "C" fn(
        this: *mut APawn,
        dir: *const FVector,
        hit: *mut FCheckResult,
        services: *const Hp2PhysicServices,
        result: *mut i32,
    ) -> i32,
    pub call_find_new_floor: unsafe extern "C" fn(
        this: *mut APawn,
        old_location: *const FVector,
        delta_time: f32,
        remaining_time: f32,
        iterations: i32,
        services: *const Hp2PhysicServices,
        result: *mut i32,
    ) -> i32,
    pub call_phys_spider: unsafe extern "C" fn(
        this: *mut APawn,
        delta_time: f32,
        iterations: i32,
        services: *const Hp2PhysicServices,
    ) -> i32,
    pub call_phys_walking: unsafe extern "C" fn(
        this: *mut APawn,
        delta_time: f32,
        iterations: i32,
        services: *const Hp2PhysicServices,
    ) -> i32,
}

impl Hp2PhysicServices {
    #[allow(clippy::too_many_arguments)]
    unsafe fn is_a(&self, obj: *mut c_void, class: i32) -> PhysicsResult<i32> {
        unsafe {
            let mut result = core::mem::MaybeUninit::<i32>::uninit();
            physics_status((self.is_a)(obj, class, result.as_mut_ptr()))?;
            Ok(result.assume_init())
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn is_probing_hit_wall(&self, actor: *mut c_void) -> PhysicsResult<i32> {
        unsafe {
            let mut result = core::mem::MaybeUninit::<i32>::uninit();
            physics_status((self.is_probing_hit_wall)(actor, result.as_mut_ptr()))?;
            Ok(result.assume_init())
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn app_frand(&self) -> PhysicsResult<f32> {
        unsafe {
            let mut result = core::mem::MaybeUninit::<f32>::uninit();
            physics_status((self.app_frand)(result.as_mut_ptr()))?;
            Ok(result.assume_init())
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn bsp_surf_special(&self, hit: *const FCheckResult, radius: f32) -> PhysicsResult<i32> {
        unsafe {
            let mut result = core::mem::MaybeUninit::<i32>::uninit();
            physics_status((self.bsp_surf_special)(hit, radius, result.as_mut_ptr()))?;
            Ok(result.assume_init())
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn rotation_vector(&self, rot: *const FRotator, out: *mut FVector) -> PhysicsResult<()> {
        unsafe { physics_status((self.rotation_vector)(rot, out)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn move_actor(
        &self,
        actor: *mut c_void,
        delta: *const FVector,
        rot: *const FRotator,
        hit: *mut FCheckResult,
    ) -> PhysicsResult<i32> {
        unsafe {
            let mut result = core::mem::MaybeUninit::<i32>::uninit();
            physics_status((self.move_actor)(
                actor,
                delta,
                rot,
                hit,
                result.as_mut_ptr(),
            ))?;
            Ok(result.assume_init())
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn single_line_check(
        &self,
        actor: *mut c_void,
        hit: *mut FCheckResult,
        end: *const FVector,
        start: *const FVector,
        trace: i32,
        extent: *const FVector,
    ) -> PhysicsResult<i32> {
        unsafe {
            let mut result = core::mem::MaybeUninit::<i32>::uninit();
            physics_status((self.single_line_check)(
                actor,
                hit,
                end,
                start,
                trace,
                extent,
                result.as_mut_ptr(),
            ))?;
            Ok(result.assume_init())
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn event_mount(&self, actor: *mut c_void, dir: *const FVector) -> PhysicsResult<()> {
        unsafe { physics_status((self.event_mount)(actor, dir)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn event_hit_wall(
        &self,
        actor: *mut c_void,
        normal: *const FVector,
        hit_actor: *mut c_void,
    ) -> PhysicsResult<()> {
        unsafe { physics_status((self.event_hit_wall)(actor, normal, hit_actor)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn event_landed(&self, actor: *mut c_void, normal: *const FVector) -> PhysicsResult<()> {
        unsafe { physics_status((self.event_landed)(actor, normal)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn point_region_zone(
        &self,
        actor: *mut c_void,
        location: *const FVector,
    ) -> PhysicsResult<*mut c_void> {
        unsafe {
            let mut result = core::mem::MaybeUninit::<*mut c_void>::uninit();
            physics_status((self.point_region_zone)(
                actor,
                location,
                result.as_mut_ptr(),
            ))?;
            Ok(result.assume_init())
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn debugf_flew_out(&self, actor: *mut c_void) -> PhysicsResult<()> {
        unsafe { physics_status((self.debugf_flew_out)(actor)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn destroy_actor(&self, actor: *mut c_void) -> PhysicsResult<()> {
        unsafe { physics_status((self.destroy_actor)(actor)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn event_falling(&self, actor: *mut c_void) -> PhysicsResult<()> {
        unsafe { physics_status((self.event_falling)(actor)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn projectile_max_speed(&self, actor: *mut c_void) -> PhysicsResult<f32> {
        Ok(unsafe { (*actor.cast::<AProjectile>()).max_speed })
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn log_fell_out_of_world(&self, actor: *mut c_void) -> PhysicsResult<()> {
        unsafe { physics_status((self.log_fell_out_of_world)(actor)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn event_fell_out_of_world(&self, actor: *mut c_void) -> PhysicsResult<()> {
        unsafe { physics_status((self.event_fell_out_of_world)(actor)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn rotation_from_vector(
        &self,
        vector: *const FVector,
        out: *mut FRotator,
    ) -> PhysicsResult<()> {
        unsafe { physics_status((self.rotation_from_vector)(vector, out)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn find_view_target(
        &self,
        actor: *mut c_void,
        tag: *const [u8; 4],
    ) -> PhysicsResult<*mut c_void> {
        unsafe {
            let mut result = core::mem::MaybeUninit::<*mut c_void>::uninit();
            physics_status((self.find_view_target)(actor, tag, result.as_mut_ptr()))?;
            Ok(result.assume_init())
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn event_update_camera(
        &self,
        manager: *mut c_void,
        old_alpha: f32,
    ) -> PhysicsResult<()> {
        unsafe { physics_status((self.event_update_camera)(manager, old_alpha)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn event_interpolate_end(
        &self,
        point: *mut c_void,
        manager: *mut c_void,
        forward: i32,
    ) -> PhysicsResult<()> {
        unsafe { physics_status((self.event_interpolate_end)(point, manager, forward)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn event_finished_interpolation(&self, manager: *mut c_void) -> PhysicsResult<()> {
        unsafe { physics_status((self.event_finished_interpolation)(manager)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn event_key_frame_reached(&self, actor: *mut c_void) -> PhysicsResult<()> {
        unsafe { physics_status((self.event_key_frame_reached)(actor)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn mover_freeze_log(
        &self,
        actor: *mut c_void,
        phys_alpha: f32,
        prev_key_num: i32,
        key_num: i32,
        hit_actor: *mut c_void,
    ) -> PhysicsResult<()> {
        unsafe {
            physics_status((self.mover_freeze_log)(
                actor,
                phys_alpha,
                prev_key_num,
                key_num,
                hit_actor,
            ))
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn event_post_touch(
        &self,
        touched: *mut c_void,
        toucher: *mut c_void,
    ) -> PhysicsResult<()> {
        unsafe { physics_status((self.event_post_touch)(touched, toucher)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn is_probing_ended_rotation(&self, actor: *mut c_void) -> PhysicsResult<i32> {
        unsafe {
            let mut result = core::mem::MaybeUninit::<i32>::uninit();
            physics_status((self.is_probing_ended_rotation)(actor, result.as_mut_ptr()))?;
            Ok(result.assume_init())
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn event_ended_rotation(&self, actor: *mut c_void) -> PhysicsResult<()> {
        unsafe { physics_status((self.event_ended_rotation)(actor)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn is_probing_may_fall(&self, actor: *mut c_void) -> PhysicsResult<i32> {
        unsafe {
            let mut result = core::mem::MaybeUninit::<i32>::uninit();
            physics_status((self.is_probing_may_fall)(actor, result.as_mut_ptr()))?;
            Ok(result.assume_init())
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn event_may_fall(&self, actor: *mut c_void) -> PhysicsResult<()> {
        unsafe { physics_status((self.event_may_fall)(actor)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn event_do_jump(&self, actor: *mut c_void, value: f32) -> PhysicsResult<()> {
        unsafe { physics_status((self.event_do_jump)(actor, value)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn far_move_actor(
        &self,
        actor: *mut c_void,
        location: *const FVector,
    ) -> PhysicsResult<()> {
        unsafe { physics_status((self.far_move_actor)(actor, location)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn debugf_fell_out(&self, actor: *mut c_void) -> PhysicsResult<()> {
        unsafe { physics_status((self.debugf_fell_out)(actor)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn single_line_check_no_source(
        &self,
        actor: *mut c_void,
        hit: *mut FCheckResult,
        end: *const FVector,
        start: *const FVector,
        trace: i32,
        extent: *const FVector,
    ) -> PhysicsResult<i32> {
        unsafe {
            let mut result = core::mem::MaybeUninit::<i32>::uninit();
            physics_status((self.single_line_check_no_source)(
                actor,
                hit,
                end,
                start,
                trace,
                extent,
                result.as_mut_ptr(),
            ))?;
            Ok(result.assume_init())
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn trailer_state(&self, actor: *mut c_void, out: *mut c_void) -> PhysicsResult<()> {
        unsafe {
            let a = &*actor.cast::<AActor>();
            let s = &mut *out.cast::<TrailerState>();
            s.owner = a.owner;
            s.mesh = if a.owner.is_null() {
                core::ptr::null_mut()
            } else {
                (*a.owner.cast::<AActor>()).mesh
            };
            s.pre_pivot = [a.pre_pivot.x, a.pre_pivot.y, a.pre_pivot.z];
            s.mass = a.mass;
            s.draw_type = i32::from(a.draw_type);
            s.anim_bone = i32::from(a.anim_bone);
            s.flags = u32::from(a.b_trailer_pre_pivot()) | (u32::from(a.b_trailer_same_rotation()) << 1);
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn mesh_bone_coords(
        &self,
        mesh: *mut c_void,
        owner: *mut c_void,
        bone: i32,
        out: *mut c_void,
    ) -> PhysicsResult<()> {
        unsafe { physics_status((self.mesh_bone_coords)(mesh, owner, bone, out)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn trailer_far_move_actor(
        &self,
        actor: *mut c_void,
        location: *const FVector,
    ) -> PhysicsResult<()> {
        unsafe { physics_status((self.trailer_far_move_actor)(actor, location)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_calc_velocity(
        &self,
        pawn: *mut APawn,
        accel_dir: FVector,
        delta_time: f32,
        max_speed: f32,
        friction: f32,
        b_fluid: i32,
        b_brake: i32,
        b_buoyant: i32,
    ) -> PhysicsResult<()> {
        unsafe {
            physics_status((self.call_calc_velocity)(
                pawn, &accel_dir, delta_time, max_speed, friction, b_fluid, b_brake, b_buoyant,
                self,
            ))
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_mount(
        &self,
        pawn: *mut APawn,
        mount_dir: FVector,
        hit: *mut FCheckResult,
    ) -> PhysicsResult<bool> {
        unsafe {
            let mut result = core::mem::MaybeUninit::<i32>::uninit();
            physics_status((self.call_mount)(
                pawn,
                &mount_dir,
                hit,
                self,
                result.as_mut_ptr(),
            ))?;
            Ok(result.assume_init() != 0)
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_step_up(
        &self,
        pawn: *mut APawn,
        grav_dir: FVector,
        desired_dir: FVector,
        delta: FVector,
        hit: *mut FCheckResult,
    ) -> PhysicsResult<()> {
        unsafe {
            physics_status((self.call_step_up)(
                pawn,
                &grav_dir,
                &desired_dir,
                &delta,
                hit,
                self,
            ))
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_process_hit_wall(
        &self,
        this: *mut AActor,
        hit_normal: FVector,
        hit_actor: *mut c_void,
    ) -> PhysicsResult<()> {
        unsafe {
            physics_status((self.call_process_hit_wall)(
                this,
                &hit_normal,
                hit_actor,
                self,
            ))
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_process_landed(
        &self,
        this: *mut AActor,
        hit_normal: FVector,
        hit_actor: *mut c_void,
        remaining_time: f32,
        iterations: i32,
    ) -> PhysicsResult<()> {
        unsafe {
            physics_status((self.call_process_landed)(
                this,
                &hit_normal,
                hit_actor,
                remaining_time,
                iterations,
                self,
            ))
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_find_base(&self, this: *mut AActor) -> PhysicsResult<()> {
        unsafe { physics_status((self.call_find_base)(this, self)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_set_physics(
        &self,
        this: *mut AActor,
        new_physics: u8,
        new_floor: *mut AActor,
    ) -> PhysicsResult<()> {
        if let Some(result) = unsafe {
            crate::actor_physics_owner::canonical_set_physics_if_active(
                this,
                new_physics,
                new_floor,
            )
        } {
            return result;
        }
        unsafe { dispatch::set_physics(this, self, new_physics, new_floor) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_phys_trailer(&self, this: *mut AActor, _delta_time: f32) -> PhysicsResult<()> {
        unsafe { physics_status((self.call_phys_trailer)(this, _delta_time, self)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_phys_moving_brush(
        &self,
        this: *mut AActor,
        delta_time: f32,
    ) -> PhysicsResult<()> {
        unsafe { physics_status((self.call_phys_moving_brush)(this, delta_time, self)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_physics_rotation_actor(
        &self,
        this: *mut AActor,
        delta_time: f32,
    ) -> PhysicsResult<()> {
        unsafe { physics_status((self.call_physics_rotation_actor)(this, delta_time, self)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_physics_rotation_pawn(
        &self,
        this: *mut APawn,
        delta_time: f32,
        old_velocity: FVector,
    ) -> PhysicsResult<()> {
        unsafe {
            physics_status((self.call_physics_rotation_pawn)(
                this,
                delta_time,
                &old_velocity,
                self,
            ))
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_get_desired_rotation_at_pause(
        &self,
        this: *mut AInterpolationPoint,
        pause_num: i32,
        new_location: FVector,
    ) -> PhysicsResult<FRotator> {
        unsafe {
            let mut result = core::mem::MaybeUninit::<FRotator>::uninit();
            physics_status((self.call_get_desired_rotation_at_pause)(
                this,
                pause_num,
                &new_location,
                self,
                result.as_mut_ptr(),
            ))?;
            Ok(result.assume_init())
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_get_desired_rotation_at_position(
        &self,
        this: *mut AInterpolationPoint,
        pos_offset: i32,
        pause_num: i32,
        new_location: FVector,
    ) -> PhysicsResult<FRotator> {
        unsafe {
            let mut result = core::mem::MaybeUninit::<FRotator>::uninit();
            physics_status((self.call_get_desired_rotation_at_position)(
                this,
                pos_offset,
                pause_num,
                &new_location,
                self,
                result.as_mut_ptr(),
            ))?;
            Ok(result.assume_init())
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_get_interpolated_position(
        &self,
        this: *mut AInterpolationPoint,
        old_coords: FCoords,
        start_control: FVector,
        phys_alpha: f32,
        pause_num: i32,
    ) -> PhysicsResult<FCoords> {
        unsafe {
            let mut result = core::mem::MaybeUninit::<FCoords>::uninit();
            physics_status((self.call_get_interpolated_position)(
                this,
                &old_coords,
                &start_control,
                phys_alpha,
                pause_num,
                self,
                result.as_mut_ptr(),
            ))?;
            Ok(result.assume_init())
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_start_swimming(
        &self,
        this: *mut APawn,
        old_velocity: FVector,
        time_tick: f32,
        remaining_time: f32,
        iterations: i32,
    ) -> PhysicsResult<()> {
        unsafe {
            physics_status((self.call_start_swimming)(
                this,
                &old_velocity,
                time_tick,
                remaining_time,
                iterations,
                self,
            ))
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_phys_flying(
        &self,
        this: *mut APawn,
        delta_time: f32,
        iterations: i32,
    ) -> PhysicsResult<()> {
        unsafe { physics_status((self.call_phys_flying)(this, delta_time, iterations, self)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_swim(
        &self,
        this: *mut APawn,
        delta: FVector,
        hit: *mut FCheckResult,
    ) -> PhysicsResult<f32> {
        unsafe {
            let mut result = core::mem::MaybeUninit::<f32>::uninit();
            physics_status((self.call_swim)(
                this,
                &delta,
                hit,
                self,
                result.as_mut_ptr(),
            ))?;
            Ok(result.assume_init())
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_find_water_line(
        &self,
        this: *mut APawn,
        start: FVector,
        end: *mut FVector,
    ) -> PhysicsResult<()> {
        unsafe { physics_status((self.call_find_water_line)(this, &start, end, self)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_phys_swimming(
        &self,
        this: *mut APawn,
        delta_time: f32,
        iterations: i32,
    ) -> PhysicsResult<()> {
        unsafe {
            physics_status((self.call_phys_swimming)(
                this, delta_time, iterations, self,
            ))
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_phys_falling(
        &self,
        this: *mut AActor,
        delta_time: f32,
        iterations: i32,
    ) -> PhysicsResult<()> {
        unsafe { physics_status((self.call_phys_falling)(this, delta_time, iterations, self)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_phys_projectile(
        &self,
        this: *mut AActor,
        delta_time: f32,
        iterations: i32,
    ) -> PhysicsResult<()> {
        unsafe {
            physics_status((self.call_phys_projectile)(
                this, delta_time, iterations, self,
            ))
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_phys_rolling(
        &self,
        this: *mut AActor,
        delta_time: f32,
        iterations: i32,
    ) -> PhysicsResult<()> {
        unsafe { physics_status((self.call_phys_rolling)(this, delta_time, iterations, self)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_check_floor(
        &self,
        this: *mut APawn,
        dir: FVector,
        hit: *mut FCheckResult,
    ) -> PhysicsResult<i32> {
        unsafe {
            let mut result = core::mem::MaybeUninit::<i32>::uninit();
            physics_status((self.call_check_floor)(
                this,
                &dir,
                hit,
                self,
                result.as_mut_ptr(),
            ))?;
            Ok(result.assume_init())
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_find_new_floor(
        &self,
        this: *mut APawn,
        old_location: FVector,
        delta_time: f32,
        remaining_time: f32,
        iterations: i32,
    ) -> PhysicsResult<i32> {
        unsafe {
            let mut result = core::mem::MaybeUninit::<i32>::uninit();
            physics_status((self.call_find_new_floor)(
                this,
                &old_location,
                delta_time,
                remaining_time,
                iterations,
                self,
                result.as_mut_ptr(),
            ))?;
            Ok(result.assume_init())
        }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_phys_spider(
        &self,
        this: *mut APawn,
        delta_time: f32,
        iterations: i32,
    ) -> PhysicsResult<()> {
        unsafe { physics_status((self.call_phys_spider)(this, delta_time, iterations, self)) }
    }
    #[allow(clippy::too_many_arguments)]
    unsafe fn call_phys_walking(
        &self,
        this: *mut APawn,
        delta_time: f32,
        iterations: i32,
    ) -> PhysicsResult<()> {
        unsafe { physics_status((self.call_phys_walking)(this, delta_time, iterations, self)) }
    }
}
/// Plain snapshot returned by the C++ adapter for `AActor::physTrailer`.
#[repr(C)]
struct TrailerState {
    owner: *mut c_void,
    mesh: *mut c_void,
    pre_pivot: [f32; 3],
    mass: f32,
    draw_type: i32,
    anim_bone: i32,
    flags: u32,
}

/// `FVector(x, y, z)`.
const fn v3(x: f32, y: f32, z: f32) -> FVector {
    FVector { x, y, z }
}

const ZERO: FVector = v3(0.0, 0.0, 0.0);

/// `A + B`.
fn add(a: FVector, b: FVector) -> FVector {
    v3(a.x + b.x, a.y + b.y, a.z + b.z)
}

/// `A - B`.
fn sub(a: FVector, b: FVector) -> FVector {
    v3(a.x - b.x, a.y - b.y, a.z - b.z)
}

/// `A * S` (`FVector::operator*(FLOAT)`).
fn scale(a: FVector, s: f32) -> FVector {
    v3(a.x * s, a.y * s, a.z * s)
}

/// `A / S` (`FVector::operator/(FLOAT)` multiplies by `1.f/S`).
fn div(a: FVector, s: f32) -> FVector {
    scale(a, 1.0 / s)
}

/// `-A`.
fn neg(a: FVector) -> FVector {
    v3(-a.x, -a.y, -a.z)
}

/// `A | B`.
fn dot(a: FVector, b: FVector) -> f32 {
    // `-ffp-contract=on`: clang fuses the second product into the first FMA.
    a.z.mul_add(b.z, a.x.mul_add(b.x, a.y * b.y))
}

/// `A ^ B` (`UnMath.h` cross product).
fn cross(a: FVector, b: FVector) -> FVector {
    // `-ffp-contract=on`: each `P*Q - R*S` fuses the left product into an FMA.
    v3(
        a.y.mul_add(b.z, -(a.z * b.y)),
        a.z.mul_add(b.x, -(a.x * b.z)),
        a.x.mul_add(b.y, -(a.y * b.x)),
    )
}

/// `SizeSquared()`.
fn size_sq(a: FVector) -> f32 {
    a.z.mul_add(a.z, a.x.mul_add(a.x, a.y * a.y))
}

/// `appSqrt(DOUBLE)` from `UnFile.h`.
fn app_sqrt(v: f64) -> f64 {
    v.sqrt()
}

/// `Size()`: `appSqrt` takes and returns `DOUBLE`, narrowed to `FLOAT`.
fn size(a: FVector) -> f32 {
    app_sqrt(size_sq(a) as f64) as f32
}

/// `IsZero()`.
fn is_zero(a: FVector) -> bool {
    a.x == 0.0 && a.y == 0.0 && a.z == 0.0
}

/// `SafeNormal()` (`UnMath.cpp`): zero below `SMALL_NUMBER`.
fn safe_normal(a: FVector) -> FVector {
    let sq = size_sq(a);
    if sq < SMALL_NUMBER {
        return ZERO;
    }
    let len = app_sqrt(sq as f64) as f32;
    scale(a, 1.0 / len)
}

/// `Normalize()` (`UnMath.h`): in place, unchanged below `SMALL_NUMBER`.
fn normalize(a: &mut FVector) {
    let sq = size_sq(*a);
    if sq >= SMALL_NUMBER {
        let s = (1.0 / app_sqrt(sq as f64)) as f32;
        *a = scale(*a, s);
    }
}

/// `Abs(A)` from `UnTemplate.h`: `A >= 0 ? A : -A`.
fn abs_f(a: f32) -> f32 {
    if a >= 0.0 { a } else { -a }
}

/// `Max(A, B)` from `UnTemplate.h`.
fn max_f(a: f32, b: f32) -> f32 {
    if a >= b { a } else { b }
}

/// `Clamp(X, Min, Max)` from `UnTemplate.h`.
fn clamp_f(x: f32, min: f32, max: f32) -> f32 {
    if x < min {
        min
    } else if x < max {
        x
    } else {
        max
    }
}

/// `FVector::MaxVal()`.
fn max_val(a: FVector) -> f32 {
    max_f(max_f(abs_f(a.x), abs_f(a.y)), abs_f(a.z))
}

/// `FCheckResult Hit(time)`. Only the fields the physics bodies read are set;
/// the rest is zero.
fn blank_hit(time: f32) -> FCheckResult {
    // SAFETY: FCheckResult is plain old data (pointers, floats, ints); a zero
    // bit pattern is a valid value and every field the bodies read is set here
    // or written by the engine call that fills it.
    let mut hit: FCheckResult = unsafe { std::mem::zeroed() };
    hit.time = time;
    hit.item = -1;
    hit
}

/// `Obj->IsA(class)` through the services table.
unsafe fn is_a(
    svc: &Hp2PhysicServices,
    obj: *mut c_void,
    class: i32,
) -> crate::physic::PhysicsResult<bool> {
    Ok({ unsafe { svc.is_a(obj, class)? != 0 } })
}

/// `Actor->GetCylinderExtent()`, reached directly through the actor services
/// table. The deleted `Hp2PhysicServices::CylinderExtent` leaf was a
/// pass-through to the same native call. Canonical Core calls use the active
/// canonical context instead of the native published table.
unsafe fn cylinder_extent(
    actor_svc: Option<&Hp2ActorServices>,
    actor: *mut c_void,
    out: *mut FVector,
) -> crate::physic::PhysicsResult<()> {
    if let Some(result) =
        unsafe { crate::actor_physics_owner::canonical_cylinder_extent_if_active(actor.cast(), out) }
    {
        return result.map_err(|_| PhysicsError(1));
    }
    let actor_svc = actor_svc.ok_or(PhysicsError(1))?;
    unsafe { crate::actor::get_cylinder_extent(actor_svc, actor.cast(), out).map_err(PhysicsError) }
}

/// `Actor->SetBase(Base)` with the default `bNotifyActor=1`
/// (`AActor.h: void SetBase(AActor *NewBase, int bNotifyActor=1);`), reached
/// directly through the actor services table. The deleted
/// `Hp2PhysicServices::SetBase` leaf was a pass-through to the same native
/// call, and every physics call site invokes it with the default. Canonical
/// Core calls resolve `SetBase` through the active canonical context.
unsafe fn set_base(
    actor_svc: Option<&Hp2ActorServices>,
    actor: *mut c_void,
    base: *mut c_void,
) -> crate::physic::PhysicsResult<()> {
    if let Some(result) =
        unsafe { crate::actor_physics_owner::canonical_set_base_if_active(actor.cast(), base.cast()) }
    {
        return result.map_err(|_| PhysicsError(1));
    }
    let actor_svc = actor_svc.ok_or(PhysicsError(1))?;
    unsafe {
        crate::actor::set_base(actor_svc, actor.cast(), base.cast(), true).map_err(PhysicsError)
    }
}

/// `GetLevel()->MoveActor(this, delta, Rotation, hit)`.
unsafe fn move_actor(
    pawn: *mut APawn,
    svc: &Hp2PhysicServices,
    delta: FVector,
    hit: *mut FCheckResult,
) -> crate::physic::PhysicsResult<i32> {
    Ok({
        unsafe {
            svc.move_actor(
                pawn as *mut c_void,
                &delta,
                &raw const (*pawn).base.rotation,
                hit,
            )?
        }
    })
}

/// `APawn::calcVelocity(AccelDir, deltaTime, maxSpeed, friction, bFluid, bBrake, bBuoyant)`.
///
/// # Safety
/// `pawn` must point to a live `APawn`. When `b_buoyant` is nonzero,
/// `Region.Zone` must point to a live `AZoneInfo` (the C++ dereferences it).
#[allow(clippy::too_many_arguments)]
pub unsafe fn calc_velocity(
    pawn: *mut APawn,
    svc: &Hp2PhysicServices,
    accel_dir: FVector,
    delta_time: f32,
    mut max_speed: f32,
    friction: f32,
    b_fluid: i32,
    b_brake: i32,
    b_buoyant: i32,
) -> crate::physic::PhysicsResult<()> {
    Ok({
        unsafe {
            let this = pawn as *mut c_void;
            let mut velocity = (*pawn).base.velocity;
            let mut acceleration = (*pawn).base.acceleration;
            let accel_rate = (*pawn).accel_rate;
            let effective_friction = max_f(b_fluid as f32, friction);
            let walking_player = is_a(svc, this, CLASS_PLAYER_PAWN)? && (*pawn).b_is_walking();

            if b_brake != 0 && is_zero(acceleration) {
                let old_vel = velocity;
                let mut sum_vel = ZERO;
                let mut remaining_time = delta_time;
                // Subdivide braking to keep results consistent at low frame rates.
                while remaining_time > 0.03 {
                    let brake = scale(scale(scale(velocity, 2.0), 0.03), effective_friction);
                    velocity = sub(velocity, brake);
                    if dot(velocity, old_vel) > 0.0 {
                        sum_vel = add(sum_vel, div(scale(velocity, 0.03), delta_time));
                    }
                    remaining_time -= 0.03;
                }
                let brake = scale(
                    scale(scale(velocity, 2.0), remaining_time),
                    effective_friction,
                );
                velocity = sub(velocity, brake);
                if dot(velocity, old_vel) > 0.0 {
                    sum_vel = add(sum_vel, div(scale(velocity, remaining_time), delta_time));
                }
                velocity = sum_vel;
                // Brake to a stop, not backwards.
                if dot(old_vel, velocity) < 0.0 || size_sq(velocity) < 100.0 {
                    velocity = ZERO;
                }
            } else {
                let vel_size = size(velocity);
                if walking_player {
                    if size_sq(acceleration) > 0.09 * accel_rate * accel_rate {
                        acceleration = scale(scale(accel_dir, accel_rate), 0.3);
                    }
                } else if size_sq(acceleration) > accel_rate * accel_rate {
                    acceleration = scale(accel_dir, accel_rate);
                }
                let drift = sub(velocity, scale(accel_dir, vel_size));
                velocity = sub(
                    velocity,
                    scale(scale(drift, delta_time), effective_friction),
                );
            }

            let damp = (-(b_fluid as f32 * friction)).mul_add(delta_time, 1.0);
            velocity = add(scale(velocity, damp), scale(acceleration, delta_time));

            if !is_a(svc, this, CLASS_PLAYER_PAWN)? {
                max_speed *= (*pawn).desired_speed;
            }

            if b_buoyant != 0 {
                let zone = (*pawn).base.region.zone as *const AZoneInfo;
                let buoyancy_factor = 1.0 - (*pawn).base.buoyancy / (*pawn).base.mass;
                let lift = scale((*zone).zone_gravity, delta_time);
                velocity = add(velocity, scale(lift, buoyancy_factor));
            }

            if walking_player && size_sq(velocity) > 0.09 * max_speed * max_speed {
                let speed = size(velocity);
                velocity = div(velocity, speed);
                let cap = max_f(
                    0.3 * max_speed,
                    speed * (-(delta_time * 2.0)).mul_add(effective_friction, 1.0),
                );
                velocity = scale(velocity, cap);
            } else if size_sq(velocity) > max_speed * max_speed {
                velocity = safe_normal(velocity);
                velocity = scale(velocity, max_speed);
            }

            (*pawn).base.velocity = velocity;
            (*pawn).base.acceleration = acceleration;
        }
    })
}

/// `APawn::Mount(MountDir, Hit)`. Returns true when the mount is taken.
///
/// # Safety
/// `pawn` must point to a live `APawn`; `hit` must point to a live `FCheckResult`.
pub unsafe fn mount(
    pawn: *mut APawn,
    svc: &Hp2PhysicServices,
    mount_dir: FVector,
    hit: *mut FCheckResult,
) -> crate::physic::PhysicsResult<bool> {
    Ok({
        unsafe {
            let actor_svc = crate::actor::published_services().ok();
            let this = pawn as *mut c_void;
            let max_mount = (*pawn).max_mount_height;
            let hit_normal = (*hit).normal;
            if max_mount > 0.0 && (hit_normal.z as f64) > -0.1 && (hit_normal.z as f64) < 0.7 {
                let mut rotation_vector = ZERO;
                svc.rotation_vector(&raw const (*pawn).base.rotation, &mut rotation_vector)?;
                if dot(hit_normal, rotation_vector) < 0.0 {
                    let mut extent = ZERO;
                    cylinder_extent(actor_svc.as_ref(), this, &mut extent)?;
                    if svc.bsp_surf_special(hit, max_val(extent))? != 0 {
                        let mut move_dir = neg(hit_normal);
                        move_dir.z = 0.0;
                        normalize(&mut move_dir);

                        let loc = (*pawn).base.location;
                        let cr = (*pawn).base.collision_radius;
                        let mut loc1 = add(loc, scale(mount_dir, max_mount));
                        let loc2 = add(loc1, scale(move_dir, 2.0 * cr + hit_normal.z * max_mount));
                        let loc3 = sub(
                            sub(loc2, scale(mount_dir, max_mount)),
                            scale(move_dir, hit_normal.z * max_mount),
                        );

                        let mut hit2 = blank_hit(0.0);
                        if svc.single_line_check(
                            this,
                            &mut hit2,
                            &loc3,
                            &loc2,
                            TRACE_VIS_BLOCKING,
                            &extent,
                        )? == 0
                        {
                            let threshold = if (*pawn).base.physics == PHYS_FALLING {
                                0.0
                            } else {
                                (*pawn).max_step_height
                            };
                            if hit2.location.z - loc.z >= threshold {
                                let mut mount_loc = hit2.location;
                                mount_loc.z += 2.0;
                                loc1.z = mount_loc.z;
                                if svc.single_line_check(
                                    this,
                                    &mut hit2,
                                    &loc1,
                                    &loc,
                                    TRACE_VIS_BLOCKING,
                                    &extent,
                                )? != 0
                                    && svc.single_line_check(
                                        this,
                                        &mut hit2,
                                        &mount_loc,
                                        &loc1,
                                        TRACE_VIS_BLOCKING,
                                        &extent,
                                    )? != 0
                                {
                                    set_base(actor_svc.as_ref(), this, (*hit).actor)?;
                                    svc.event_mount(this, &sub(mount_loc, loc))?;
                                    return Ok(true);
                                }
                            }
                        }
                    }
                }
            }
            false
        }
    })
}

/// `TwoWallAdjust(DesiredDir, Delta, HitNormal, OldHitNormal, HitTime)`
/// (`AActor.h`).
pub fn two_wall_adjust(
    desired_dir: FVector,
    delta: &mut FVector,
    hit_normal: FVector,
    old_hit_normal: FVector,
    hit_time: f32,
) {
    if dot(old_hit_normal, hit_normal) <= 0.0 {
        let new_dir = safe_normal(cross(hit_normal, old_hit_normal));
        let along = dot(*delta, new_dir) * (1.0 - hit_time);
        *delta = scale(new_dir, along);
        if dot(desired_dir, *delta) < 0.0 {
            *delta = scale(*delta, -1.0);
        }
    } else {
        let slide = sub(*delta, scale(hit_normal, dot(*delta, hit_normal)));
        *delta = scale(slide, 1.0 - hit_time);
        if dot(*delta, desired_dir) <= 0.0 {
            *delta = ZERO;
        }
    }
}

/// `APawn::stepUp(GravDir, DesiredDir, Delta, Hit)`.
///
/// # Safety
/// `pawn` must point to a live `APawn`; `hit` must point to a live `FCheckResult`
/// whose `actor` is a live object whenever `time < 1` (the C++ dereferences it).
pub unsafe fn step_up(
    pawn: *mut APawn,
    svc: &Hp2PhysicServices,
    grav_dir: FVector,
    desired_dir: FVector,
    mut delta: FVector,
    hit: *mut FCheckResult,
) -> crate::physic::PhysicsResult<()> {
    Ok({
        unsafe {
            if svc.call_mount(pawn, neg(grav_dir), hit)? {
                return Ok(());
            }
            let this = pawn as *mut c_void;
            let actor = pawn as *mut AActor;
            let down = scale(grav_dir, (*pawn).max_step_height);
            let up = scale(down, -1.0);
            move_actor(pawn, svc, up, hit)?;
            move_actor(pawn, svc, delta, hit)?;

            if (*hit).time < 1.0 {
                let hit_actor = (*hit).actor;
                let hit_normal = (*hit).normal;
                if is_a(svc, this, CLASS_PLAYER_PAWN)?
                    && is_a(svc, hit_actor, CLASS_DECORATION)?
                    && (*(hit_actor as *mut ADecoration)).b_pushable()
                    && (dot(hit_normal, desired_dir) as f64) < -0.9
                {
                    (*pawn).base.set_b_just_teleported(true);
                    let mass = (*pawn).base.mass;
                    let hit_mass = (*(hit_actor as *mut AActor)).mass;
                    (*pawn).base.velocity = scale((*pawn).base.velocity, mass / (mass + hit_mass));
                    svc.call_process_hit_wall(actor, hit_normal, hit_actor)?;
                    if (*pawn).base.physics == PHYS_FALLING {
                        return Ok(());
                    }
                } else if abs_f(hit_normal.z) < 0.2 && (*hit).time * size_sq(delta) > 144.0 {
                    svc.call_step_up(
                        pawn,
                        grav_dir,
                        desired_dir,
                        scale(delta, 1.0 - (*hit).time),
                        hit,
                    )?;
                    if (*pawn).base.physics == PHYS_FALLING {
                        return Ok(());
                    }
                } else {
                    svc.call_process_hit_wall(actor, hit_normal, hit_actor)?;
                    let original_delta = delta;
                    let old_hit_normal = hit_normal;
                    let slide = sub(delta, scale(hit_normal, dot(delta, hit_normal)));
                    delta = scale(slide, 1.0 - (*hit).time);
                    if dot(delta, original_delta) >= 0.0 {
                        move_actor(pawn, svc, delta, hit)?;
                        if (*hit).time < 1.0 {
                            svc.call_process_hit_wall(actor, (*hit).normal, (*hit).actor)?;
                            if (*pawn).base.physics == PHYS_FALLING {
                                return Ok(());
                            }
                            two_wall_adjust(
                                desired_dir,
                                &mut delta,
                                (*hit).normal,
                                old_hit_normal,
                                (*hit).time,
                            );
                            move_actor(pawn, svc, delta, hit)?;
                        }
                    }
                }
            }

            move_actor(pawn, svc, down, hit)?;
            if (*hit).time < 1.0 && (*hit).normal.z < 0.5 {
                let hit_normal = (*hit).normal;
                let slide = sub(down, scale(hit_normal, dot(down, hit_normal)));
                delta = scale(slide, 1.0 - (*hit).time);
                if dot(delta, down) >= 0.0 {
                    move_actor(pawn, svc, delta, hit)?;
                }
            }
        }
    })
}

/// `AActor::processHitWall(HitNormal, HitActor)`.
///
/// # Safety
/// `this` must point to a live `AActor`; `hit_actor` must be a live object.
pub unsafe fn process_hit_wall(
    this: *mut AActor,
    svc: &Hp2PhysicServices,
    mut hit_normal: FVector,
    hit_actor: *mut c_void,
) -> crate::physic::PhysicsResult<()> {
    Ok({
        unsafe {
            let this_v = this as *mut c_void;
            if is_a(svc, hit_actor, CLASS_PAWN)? {
                return Ok(());
            }
            if is_a(svc, this_v, CLASS_PAWN)? {
                let pawn = this as *mut APawn;
                if is_zero((*this).acceleration) {
                    return Ok(());
                }
                let mut dir = safe_normal(sub((*pawn).destination, (*this).location));
                if (*this).physics == PHYS_WALKING {
                    hit_normal.z = 0.0;
                    dir.z = 0.0;
                }
                if (*pawn).min_hit_wall < dot(dir, hit_normal) {
                    return Ok(());
                }
                if svc.is_probing_hit_wall(this_v)? == 0 && (*this).physics != PHYS_FALLING {
                    (*pawn).set_b_from_wall(true);
                    return Ok(());
                }
            } else if svc.is_probing_hit_wall(this_v)? == 0 {
                return Ok(());
            }
            svc.event_hit_wall(this_v, &hit_normal, hit_actor)?;
        }
    })
}


/// `AActor::moveSmooth(Delta)`. Returns the first `MoveActor` result.
///
/// # Safety
/// `this` must point to a live `AActor`.
pub unsafe fn move_smooth(
    this: *mut AActor,
    svc: &Hp2PhysicServices,
    delta: FVector,
) -> crate::physic::PhysicsResult<i32> {
    Ok(unsafe {
        crate::actor_physics_owner::move_smooth_with_ops(
            &mut NativeMoveSmoothOps(svc),
            this,
            delta,
        )? as i32
    })
}

struct NativeMoveSmoothOps<'a>(&'a Hp2PhysicServices);

impl crate::actor_physics_owner::MoveSmoothOps for NativeMoveSmoothOps<'_> {
    type Error = PhysicsError;

    unsafe fn move_actor(
        &mut self,
        actor: *mut AActor,
        delta: FVector,
        rotation: FRotator,
        hit: *mut FCheckResult,
    ) -> PhysicsResult<bool> {
        Ok(unsafe { self.0.move_actor(actor.cast(), &delta, &rotation, hit)? != 0 })
    }

    unsafe fn event_hit_wall(
        &mut self,
        actor: *mut AActor,
        normal: FVector,
        hit_actor: *mut c_void,
    ) -> PhysicsResult<()> {
        unsafe { self.0.event_hit_wall(actor.cast(), &normal, hit_actor) }
    }
}

/// One of the four `SingleLineCheck` probes `processLanded` runs under a
/// decoration. `signs` is the (x, y) quadrant; `half` is `0.5f*CollisionRadius`.
unsafe fn landing_quad(
    svc: &Hp2PhysicServices,
    this: *mut c_void,
    hit: *mut FCheckResult,
    loc: FVector,
    signs: (f32, f32),
    half: f32,
    extent: &FVector,
) -> crate::physic::PhysicsResult<i32> {
    Ok({
        let (sx, sy) = signs;
        let end = v3(loc.x + sx * half, loc.y + sy * half, loc.z + -8.0);
        let start = v3(loc.x + sx * half, loc.y + sy * half, loc.z + 0.0);
        unsafe { svc.single_line_check(this, hit, &end, &start, TRACE_ALL_BLOCKING, extent)? }
    })
}

/// `AActor::processLanded(HitNormal, HitActor, remainingTime, Iterations)`.
///
/// # Safety
/// `this` must point to a live `AActor` (a pawn when `bIsPawn` is set, a
/// decoration or carcass when the class checks pass). `hit_actor` must be a live
/// object. `Region.Zone` must be live when `bIsPawn` is clear.
pub unsafe fn process_landed(
    this: *mut AActor,
    svc: &Hp2PhysicServices,
    hit_normal: FVector,
    hit_actor: *mut c_void,
    remaining_time: f32,
    iterations: i32,
) -> crate::physic::PhysicsResult<()> {
    Ok({
        unsafe {
            let actor_svc = crate::actor::published_services().ok();
            let this_v = this as *mut c_void;
            if !(*this).b_is_pawn() {
                let zone = (*this).region.zone as *const AZoneInfo;
                if (*zone).b_bounce_velocity() && !is_zero((*zone).zone_velocity) {
                    (*this).velocity = add((*zone).zone_velocity, v3(0.0, 0.0, 80.0));
                    return Ok(());
                }
            }

            if is_a(svc, this_v, CLASS_DECORATION)? {
                let dec = this as *mut ADecoration;
                if (*dec).num_landings < 5 {
                    let loc = (*this).location;
                    let ch = (*this).collision_height;
                    let cr = (*this).collision_radius;
                    let mut hit = blank_hit(1.0);
                    let end = sub(loc, v3(0.0, 0.0, ch + cr + 8.0));
                    let start = sub(loc, v3(0.0, 0.0, 0.8 * ch));
                    svc.single_line_check(
                        this_v,
                        &mut hit,
                        &end,
                        &start,
                        TRACE_ALL_BLOCKING,
                        &ZERO,
                    )?;
                    if hit.actor.is_null() {
                        let mut extent = ZERO;
                        cylinder_extent(actor_svc.as_ref(), this_v, &mut extent)?;
                        let mut part = scale(extent, 0.5);
                        part.z *= 2.0;
                        let half = 0.5 * cr;
                        let hit_ptr: *mut FCheckResult = &mut hit;
                        let q1 = landing_quad(svc, this_v, hit_ptr, loc, (1.0, 1.0), half, &part)?;
                        let q2 = landing_quad(svc, this_v, hit_ptr, loc, (-1.0, 1.0), half, &part)?;
                        let q3 =
                            landing_quad(svc, this_v, hit_ptr, loc, (-1.0, -1.0), half, &part)?;
                        let q4 = landing_quad(svc, this_v, hit_ptr, loc, (1.0, -1.0), half, &part)?;
                        if q1 + q2 + q3 + q4 > 1 && q1 + q3 != 0 && q2 + q4 != 0 {
                            (*dec).num_landings += 1;
                            let speed = 2.0 * clamp_f(-(*this).velocity.z, 30.0, 30.0 + cr);
                            let dir =
                                v3((q1 + q4 - q2 - q3) as f32, (q1 + q2 - q3 - q4) as f32, 0.5);
                            (*this).velocity = scale(dir, speed);
                            return Ok(());
                        }
                    }
                    if is_a(svc, this_v, CLASS_CARCASS)?
                        && (hit_normal.z as f64) < 0.9
                        && (*(this as *mut ACarcass)).b_sliding_carcass()
                    {
                        if svc.app_frand()? < 0.2 {
                            (*dec).num_landings += 1;
                        }
                        (*this).velocity = scale(hit_normal, 120.0);
                        (*this).velocity.z = 70.0;
                        return Ok(());
                    }
                    (*dec).num_landings = 0;
                } else {
                    (*dec).num_landings = 0;
                }
            }

            svc.event_landed(this_v, &hit_normal)?;

            if (*this).physics == PHYS_FALLING {
                if is_a(svc, this_v, CLASS_PAWN)? {
                    svc.call_set_physics(this, PHYS_WALKING, hit_actor as *mut AActor)?;
                } else {
                    svc.call_set_physics(this, PHYS_NONE, hit_actor as *mut AActor)?;
                    (*this).velocity = ZERO;
                }
            }

            if (*this).physics == PHYS_WALKING && is_a(svc, this_v, CLASS_PAWN)? {
                (*this).acceleration = safe_normal((*this).acceleration);
                if remaining_time > 0.01 {
                    svc.call_phys_walking(this as *mut APawn, remaining_time, iterations)?;
                }
            }
        }
    })
}

/// `Min(A, B)` from `UnTemplate.h` for `INT`.
fn min_i(a: i32, b: i32) -> i32 {
    if a <= b { a } else { b }
}

/// `Max(A, B)` from `UnTemplate.h` for `INT`.
fn max_i(a: i32, b: i32) -> i32 {
    if a >= b { a } else { b }
}

/// `AActor::fixedTurn(current, desired, deltaRate)`. Angles are 16-bit
/// `FRotator` components carried in `int`. `fixed_dir` is
/// `bFixedRotationDir`, `rotate_to_desired` is `bRotateToDesired`.
pub fn fixed_turn(
    current: i32,
    desired: i32,
    delta_rate: i32,
    fixed_dir: bool,
    rotate_to_desired: bool,
) -> i32 {
    if delta_rate == 0 {
        return current & 65535;
    }

    let mut result = current & 65535;
    let mut current = result;
    let mut desired = desired & 65535;

    if fixed_dir {
        if rotate_to_desired {
            if delta_rate > 0 {
                if current > desired {
                    desired += 65536;
                }
                result = result.wrapping_add(min_i(delta_rate, desired - current));
            } else {
                if current < desired {
                    current += 65536;
                }
                result = result.wrapping_add(max_i(delta_rate, desired - current));
            }
        } else {
            result = result.wrapping_add(delta_rate);
        }
    } else if rotate_to_desired {
        if current > desired {
            if current - desired < 32768 {
                result = result.wrapping_sub(min_i(current - desired, delta_rate.wrapping_abs()));
            } else {
                result = result
                    .wrapping_add(min_i(desired + 65536 - current, delta_rate.wrapping_abs()));
            }
        } else if desired - current < 32768 {
            result = result.wrapping_add(min_i(desired - current, delta_rate.wrapping_abs()));
        } else {
            result =
                result.wrapping_sub(min_i(current + 65536 - desired, delta_rate.wrapping_abs()));
        }
    }

    result & 65535
}

/// `AActor::execMoveSmooth` body after `P_FINISH` (`Engine/Src/UnPhysic.cpp`).
/// Clears `bJustTeleported` before routing through the live `moveSmooth` vtable
/// dispatch (`AActor`/`APawn`/`AInterpolationManager` each override it) via
/// `Hp2ActorServices`, the same entry point `Hp2LevTicServices::LevTicActorMoveSmooth`
/// still reaches directly.
///
/// # Safety
/// `actor` must be a live `AActor`; `delta` must be a valid `FVector`.
pub unsafe fn exec_move_smooth(
    svc: &Hp2ActorServices,
    actor: *mut AActor,
    delta: *const FVector,
) -> PhysicsResult<u32> {
    unsafe {
        (*actor).set_b_just_teleported(false);
        let did_hit = svc
            .actor_move_smooth(actor.cast(), delta.cast())
            .map_err(PhysicsError)?;
        Ok(did_hit as u32)
    }
}

/// `AActor::execSetPhysics` body after `P_FINISH` (`Engine/Src/UnPhysic.cpp`).
///
/// # Safety
/// `actor` must be a live `AActor`.
pub unsafe fn exec_set_physics(
    svc: &Hp2ActorServices,
    actor: *mut AActor,
    new_physics: u8,
) -> PhysicsResult<()> {
    unsafe {
        svc.actor_set_physics(actor.cast(), new_physics)
            .map_err(PhysicsError)
    }
}

/// `AActor::execAutonomousPhysics` body after `P_FINISH` (`Engine/Src/UnPhysic.cpp`).
/// `RoundAcceleration` is the infallible `physic::dispatch` leaf; `performPhysics`
/// only runs while `Physics != PHYS_None`, through `Hp2ActorServices` (the same
/// vtable dispatch `Hp2LevTicServices::LevTicActorPerformPhysics` still reaches
/// directly).
///
/// # Safety
/// `actor` must be a live `AActor`.
pub unsafe fn exec_autonomous_physics(
    svc: &Hp2ActorServices,
    actor: *mut AActor,
    delta_seconds: f32,
) -> PhysicsResult<()> {
    unsafe { exec_autonomous_physics_with(actor, delta_seconds, |actor, delta| {
        svc.actor_perform_physics(actor.cast(), delta).map_err(PhysicsError)
    }) }
}

/// Shared rounding-before-dispatch policy for autonomous script physics.
///
/// # Safety
/// Actor and its physics provider remain live through synchronous dispatch.
pub(crate) unsafe fn exec_autonomous_physics_with<E>(
    actor: *mut AActor, delta: f32,
    perform: impl FnOnce(*mut AActor, f32) -> Result<(), E>,
) -> Result<(), E> {
    unsafe {
        dispatch::round_acceleration(actor);
        if (*actor).physics != PHYS_NONE { perform(actor, delta)?; }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::fixed_turn;

    #[test]
    fn zero_rate_masks_current() {
        assert_eq!(fixed_turn(0x1_0005, 7, 0, true, true), 5);
    }

    #[test]
    fn shortest_way_wraps_down() {
        // 65000 -> 100 is 636 steps up through zero, capped by the rate.
        assert_eq!(fixed_turn(65000, 100, 1000, false, true), 100);
    }
}
