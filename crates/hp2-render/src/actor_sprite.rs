//! Actor draw policy. Each service invocation is a typed SDK leaf; no engine
//! reference is retained across a callback. Failure deliberately skips restoration.
use core::ffi::c_void;
use hp2_abi::{FCoords, FPlane, FVector};
use hp2_core::math::{self, FRotator, plane_scale as scale_color};

/// Opaque SDK object pointer used by actor sprite operations.
pub type Object = *mut c_void;
type Result<T, E = ()> = core::result::Result<T, E>;
#[repr(C)]
pub struct ActorSpriteServices {
    pub context: Object,
    pub call: unsafe extern "C" fn(Object, u32, Object, Object) -> i32,
}
/// Parameters passed to the SDK icon drawing leaf.
#[repr(C)]
pub struct Icon {
    pub canvas: Object,
    pub texture: Object,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: FPlane,
    pub flags: u32,
}
#[repr(C)]
struct Mesh {
    actor: Object,
    coords: *const FCoords,
    flags: u32,
}
/// Parameters passed to the SDK circular bounds leaf.
#[repr(C)]
pub struct Circle {
    pub color: FPlane,
    pub center: *mut FVector,
    pub radius: f32,
}
/// Parameters passed to the SDK line drawing leaf.
#[repr(C)]
pub struct Line {
    pub color: FVector,
    pub start: *const FVector,
    pub end: *const FVector,
}
/// Parameters passed to the SDK selection-box drawing leaf.
#[repr(C)]
pub struct BoxDraw {
    pub color: FPlane,
    pub coords: FCoords,
}
/// Parameters passed to actor-light calculation.
#[repr(C)]
pub struct Lighting {
    pub brightness: f32,
    pub color: FPlane,
}
const NIL: Object = core::ptr::null_mut();
const SET: u32 = 0x1000;

// Internal field selectors are translated to typed accessors before host dispatch.
/// Typed source-field access and SDK leaves for actor sprite rendering.
///
/// Implementations retain any required frame/Core context in `self`.
pub trait ActorSpriteOps: Sized {
    type Error;
    unsafe fn read_i32(&self, object: Object, field: ActorI32Field) -> Result<i32, Self::Error>;
    unsafe fn write_i32(&self, object: Object, field: ActorI32Field, value: i32) -> Result<(), Self::Error>;
    unsafe fn read_f32(&self, object: Object, field: ActorF32Field) -> Result<f32, Self::Error>;
    unsafe fn write_f32(&self, object: Object, field: ActorF32Field, value: f32) -> Result<(), Self::Error>;
    unsafe fn read_object(&self, object: Object, field: ActorObjectField) -> Result<Object, Self::Error>;
    unsafe fn write_object(&self, object: Object, field: ActorObjectField, value: Object) -> Result<(), Self::Error>;
    unsafe fn read_coords(&self, object: Object, field: ActorCoordsField) -> Result<FCoords, Self::Error>;
    unsafe fn read_rotator(&self, object: Object, field: ActorRotatorField) -> Result<FRotator, Self::Error>;
    unsafe fn write_rotator(&self, object: Object, field: ActorRotatorField, value: FRotator) -> Result<(), Self::Error>;
    unsafe fn read_vector(&self, object: Object, field: ActorVectorField) -> Result<FVector, Self::Error>;
    unsafe fn write_vector(&self, object: Object, field: ActorVectorField, value: FVector) -> Result<(), Self::Error>;
    unsafe fn read_u32(&self, object: Object, field: ActorU32Field) -> Result<u32, Self::Error>;
    unsafe fn actor(&self) -> Result<Object, Self::Error>;
    unsafe fn draw_mesh(&self, actor: Object, coords: *const FCoords, flags: u32) -> Result<(), Self::Error>;
    unsafe fn draw_icon(&self, icon: &mut Icon) -> Result<(), Self::Error>;
    unsafe fn push_actor_hit(&self) -> Result<(), Self::Error>;
    unsafe fn pop_actor_hit(&self) -> Result<(), Self::Error>;
    unsafe fn render_selected(&self) -> Result<(), Self::Error>;
    unsafe fn render_bounds(&self, actor: Object) -> Result<(), Self::Error>;
    unsafe fn render_light(&self, actor: Object, lighting: &mut Lighting) -> Result<(), Self::Error>;
    unsafe fn draw_circle(&self, circle: &mut Circle) -> Result<(), Self::Error>;
    unsafe fn draw_line(&self, line: &mut Line) -> Result<(), Self::Error>;
    unsafe fn draw_selection_box(&self, box_draw: &mut BoxDraw) -> Result<(), Self::Error>;
    unsafe fn render_weapon_animation(&self, weapon: Object) -> Result<(), Self::Error>;
}

macro_rules! actor_fields {
    ($name:ident { $($variant:ident = $id:literal),* $(,)? }) => {
        #[derive(Clone, Copy, Debug)]
        #[non_exhaustive]
        pub enum $name { $($variant),* }
        impl $name {
            const fn id(self) -> u32 {
                match self { $(Self::$variant => $id),* }
            }
            fn from_id(id: u32) -> Self {
                match id { $($id => Self::$variant,)* _ => panic!("unknown actor sprite field id {id}") }
            }
        }
    };
}

actor_fields!(ActorI32Field {
    ActorStyle = 0, ActorNoSmooth = 1, ActorSelected = 2, ActorMeshEnviroMap = 3,
    ActorMeshCurvy = 4, ActorUnlit = 5, ActorZoneNumber = 6, ActorDrawType = 7,
    ActorHidden = 11, ActorCollideWorld = 17, ActorCollideActors = 18,
    ActorLightType = 19, ActorLightBrightness = 20, ActorLightRadiusInner = 21,
    ActorDeleteMe = 26, WeaponAnimSequence = 14, WeaponSteadyFlash3rd = 32,
    WeaponSteadyToggle = 33, WeaponToggleSteadyFlash = 34, WeaponFirstFrame = 35,
    WeaponFlashCount = 36, WeaponOldFlashCount = 37, WeaponMuzzleFlashStyle = 40,
    TextureWidth = 52, TextureHeight = 53, ViewportShowFlags = 60, RenderMode = 61,
    RenderPass = 90, HasSpecialCoords = 91, NoLighting = 103, IsOrtho = 104,
    PolyScope = 130, EditorHack = 131
});
actor_fields!(ActorF32Field {
    ActorDrawScale = 9, ActorScaleGlow = 10, AnimFrame = 15, CollisionRadius = 16,
    LevelBrightness = 22, LifeSpan = 23, CollisionHeight = 25,
    WeaponThirdPersonScale = 31, WeaponMuzzleFlashScale = 39, Mirror = 63,
    ScreenX = 71, ScreenY = 72, Perspective = 73, PawnBaseEyeHeight = 84,
    RandomFrame = 119, WorldLightRadius = 111
});
actor_fields!(ActorObjectField {
    ActorLocationAddress = 27, ActorTexture = 8, ActorMesh = 24, FrameCoordsAddress = 64,
    WeaponThirdPersonMesh = 30, WeaponMuzzleFlashMesh = 38, WeaponMuzzleFlashTexture = 41,
    AnimNext = 50, AnimCurrent = 51, Canvas = 67, SpriteActor = 70,
    PawnWeapon = 80, PawnPlayerInfo = 83, PlayerInfoHasFlag = 85,
    ActorHitActor = 102, ActorPawn = 110, DefaultTexture = 105, ActorDefault = 120,
    TextureResolved = 121, TrigTable = 92
});
actor_fields!(ActorCoordsField {
    SpecialCoords = 93, ViewCoords = 62, LocalCoords = 114, RenderBoundsCoords = 117
});
actor_fields!(ActorRotatorField { ActorRotation = 13, PawnWeaponRotation = 81 });
actor_fields!(ActorVectorField { ActorLocation = 12, PawnWeaponLocation = 82 });
actor_fields!(ActorU32Field { TexturePolyFlags = 54 });

trait ActorSpriteFieldAccess: Copy {
    unsafe fn read_from<O: ActorSpriteOps>(ops: &O, object: Object, id: u32) -> Result<Self, O::Error>;
}

trait ActorSpriteFieldWrite: ActorSpriteFieldAccess {
    unsafe fn write_to<O: ActorSpriteOps>(
        ops: &O,
        object: Object,
        id: u32,
        value: Self,
    ) -> Result<(), O::Error>;
}

macro_rules! impl_actor_field_access {
    ($ty:ty, $field:ty, $read:ident, $write:ident) => {
        impl ActorSpriteFieldAccess for $ty {
            unsafe fn read_from<O: ActorSpriteOps>(
                ops: &O,
                object: Object,
                id: u32,
            ) -> Result<Self, O::Error> {
                unsafe { ops.$read(object, <$field>::from_id(id)) }
            }
        }
        impl ActorSpriteFieldWrite for $ty {
            unsafe fn write_to<O: ActorSpriteOps>(
                ops: &O,
                object: Object,
                id: u32,
                value: Self,
            ) -> Result<(), O::Error> {
                unsafe { ops.$write(object, <$field>::from_id(id), value) }
            }
        }
    };
}

impl_actor_field_access!(i32, ActorI32Field, read_i32, write_i32);
impl_actor_field_access!(f32, ActorF32Field, read_f32, write_f32);
impl_actor_field_access!(Object, ActorObjectField, read_object, write_object);
impl_actor_field_access!(FRotator, ActorRotatorField, read_rotator, write_rotator);
impl_actor_field_access!(FVector, ActorVectorField, read_vector, write_vector);

impl ActorSpriteFieldAccess for FCoords {
    unsafe fn read_from<O: ActorSpriteOps>(
        ops: &O,
        object: Object,
        id: u32,
    ) -> Result<Self, O::Error> {
        unsafe { ops.read_coords(object, ActorCoordsField::from_id(id)) }
    }
}

impl ActorSpriteFieldAccess for u32 {
    unsafe fn read_from<O: ActorSpriteOps>(
        ops: &O,
        object: Object,
        id: u32,
    ) -> Result<Self, O::Error> {
        unsafe { ops.read_u32(object, ActorU32Field::from_id(id)) }
    }
}
impl ActorSpriteOps for ActorSpriteServices {
    type Error = ();

    unsafe fn read_i32(&self, object: Object, field: ActorI32Field) -> Result<i32> {
        unsafe { self.get(field.id(), object) }
    }
    unsafe fn write_i32(&self, object: Object, field: ActorI32Field, value: i32) -> Result<()> {
        unsafe { self.put(field.id(), object, value) }
    }
    unsafe fn read_f32(&self, object: Object, field: ActorF32Field) -> Result<f32> {
        unsafe { self.get(field.id(), object) }
    }
    unsafe fn write_f32(&self, object: Object, field: ActorF32Field, value: f32) -> Result<()> {
        unsafe { self.put(field.id(), object, value) }
    }
    unsafe fn read_object(&self, object: Object, field: ActorObjectField) -> Result<Object> {
        unsafe { self.get(field.id(), object) }
    }
    unsafe fn write_object(&self, object: Object, field: ActorObjectField, value: Object) -> Result<()> {
        unsafe { self.put(field.id(), object, value) }
    }
    unsafe fn read_coords(&self, object: Object, field: ActorCoordsField) -> Result<FCoords> {
        unsafe { self.get(field.id(), object) }
    }
    unsafe fn read_rotator(&self, object: Object, field: ActorRotatorField) -> Result<FRotator> {
        unsafe { self.get(field.id(), object) }
    }
    unsafe fn write_rotator(&self, object: Object, field: ActorRotatorField, value: FRotator) -> Result<()> {
        unsafe { self.put(field.id(), object, value) }
    }
    unsafe fn read_vector(&self, object: Object, field: ActorVectorField) -> Result<FVector> {
        unsafe { self.get(field.id(), object) }
    }
    unsafe fn write_vector(&self, object: Object, field: ActorVectorField, value: FVector) -> Result<()> {
        unsafe { self.put(field.id(), object, value) }
    }
    unsafe fn read_u32(&self, object: Object, field: ActorU32Field) -> Result<u32> {
        unsafe { self.get(field.id(), object) }
    }
    unsafe fn actor(&self) -> Result<Object> {
        unsafe { self.ptr(70, NIL) }
    }
    unsafe fn draw_mesh(&self, actor: Object, coords: *const FCoords, flags: u32) -> Result<()> {
        unsafe { self.leaf(107, NIL, &mut Mesh { actor, coords, flags }) }
    }
    unsafe fn draw_icon(&self, icon: &mut Icon) -> Result<()> {
        unsafe { self.leaf(106, NIL, icon) }
    }
    unsafe fn push_actor_hit(&self) -> Result<()> {
        unsafe { self.run(108, NIL) }
    }
    unsafe fn pop_actor_hit(&self) -> Result<()> {
        unsafe { self.run(109, NIL) }
    }
    unsafe fn render_selected(&self) -> Result<()> {
        unsafe { self.run(100, NIL) }
    }
    unsafe fn render_bounds(&self, actor: Object) -> Result<()> {
        unsafe { self.run(113, actor) }
    }
    unsafe fn render_light(&self, actor: Object, lighting: &mut Lighting) -> Result<()> {
        unsafe { self.leaf(112, actor, lighting) }
    }
    unsafe fn draw_circle(&self, circle: &mut Circle) -> Result<()> {
        unsafe { self.leaf(116, NIL, circle) }
    }
    unsafe fn draw_line(&self, line: &mut Line) -> Result<()> {
        unsafe { self.leaf(115, NIL, line) }
    }
    unsafe fn draw_selection_box(&self, box_draw: &mut BoxDraw) -> Result<()> {
        unsafe { self.leaf(118, NIL, box_draw) }
    }
    unsafe fn render_weapon_animation(&self, weapon: Object) -> Result<()> {
        unsafe { self.run(122, weapon) }
    }
}
trait ActorSpriteLeafAccess {
    unsafe fn invoke(
        ops: &impl ActorSpriteOps<Error = ()>,
        object: Object,
        value: &mut Self,
    ) -> Result<()>;
}

impl ActorSpriteLeafAccess for Icon {
    unsafe fn invoke(ops: &impl ActorSpriteOps<Error = ()>, _object: Object, value: &mut Self) -> Result<()> {
        unsafe { ops.draw_icon(value) }
    }
}
impl ActorSpriteLeafAccess for Lighting {
    unsafe fn invoke(ops: &impl ActorSpriteOps<Error = ()>, object: Object, value: &mut Self) -> Result<()> {
        unsafe { ops.render_light(object, value) }
    }
}
impl ActorSpriteLeafAccess for Circle {
    unsafe fn invoke(ops: &impl ActorSpriteOps<Error = ()>, _object: Object, value: &mut Self) -> Result<()> {
        unsafe { ops.draw_circle(value) }
    }
}
impl ActorSpriteLeafAccess for Line {
    unsafe fn invoke(ops: &impl ActorSpriteOps<Error = ()>, _object: Object, value: &mut Self) -> Result<()> {
        unsafe { ops.draw_line(value) }
    }
}
impl ActorSpriteLeafAccess for BoxDraw {
    unsafe fn invoke(ops: &impl ActorSpriteOps<Error = ()>, _object: Object, value: &mut Self) -> Result<()> {
        unsafe { ops.draw_selection_box(value) }
    }
}

impl ActorSpriteServices {
    unsafe fn get<T: Copy>(&self, op: u32, object: Object) -> Result<T> {
        let mut value = core::mem::MaybeUninit::<T>::uninit();
        if unsafe { (self.call)(self.context, op, object, value.as_mut_ptr().cast()) } == 0 {
            return Err(());
        }
        Ok(unsafe { value.assume_init() })
    }
    unsafe fn put<T>(&self, op: u32, object: Object, mut value: T) -> Result<()> {
        if unsafe {
            (self.call)(
                self.context,
                op | SET,
                object,
                (&mut value as *mut T).cast(),
            )
        } == 0
        {
            Err(())
        } else {
            Ok(())
        }
    }
    unsafe fn leaf<T>(&self, op: u32, object: Object, value: &mut T) -> Result<()> {
        if unsafe { (self.call)(self.context, op, object, (value as *mut T).cast()) } == 0 {
            Err(())
        } else {
            Ok(())
        }
    }
    unsafe fn run(&self, op: u32, object: Object) -> Result<()> {
        unsafe { self.leaf(op, object, &mut 0i32) }
    }
    unsafe fn int(&self, op: u32, object: Object) -> Result<i32> {
        unsafe { self.get(op, object) }
    }
    unsafe fn float(&self, op: u32, object: Object) -> Result<f32> {
        unsafe { self.get(op, object) }
    }
    unsafe fn ptr(&self, op: u32, object: Object) -> Result<Object> {
        unsafe { self.get(op, object) }
    }
}

trait ActorSpritePolicyOps: ActorSpriteOps<Error = ()> {
    unsafe fn get<T: ActorSpriteFieldAccess>(&self, op: u32, object: Object) -> Result<T> {
        unsafe { T::read_from(self, object, op) }
    }
    unsafe fn put<T: ActorSpriteFieldWrite>(&self, op: u32, object: Object, value: T) -> Result<()> {
        unsafe { T::write_to(self, object, op, value) }
    }
    unsafe fn leaf<T: ActorSpriteLeafAccess>(&self, _op: u32, object: Object, value: &mut T) -> Result<()> {
        unsafe { T::invoke(self, object, value) }
    }
    unsafe fn run(&self, op: u32, object: Object) -> Result<()> {
        match op {
            100 => unsafe { self.render_selected() },
            108 => unsafe { self.push_actor_hit() },
            109 => unsafe { self.pop_actor_hit() },
            113 => unsafe { self.render_bounds(object) },
            122 => unsafe { self.render_weapon_animation(object) },
            _ => panic!("unknown actor sprite callback id {op}"),
        }
    }
    unsafe fn int(&self, op: u32, object: Object) -> Result<i32> {
        unsafe { self.get(op, object) }
    }
    unsafe fn float(&self, op: u32, object: Object) -> Result<f32> {
        unsafe { self.get(op, object) }
    }
    unsafe fn ptr(&self, op: u32, object: Object) -> Result<Object> {
        unsafe { self.get(op, object) }
    }
    unsafe fn exchange<T: ActorSpriteFieldWrite>(&self, a: u32, b: u32, object: Object) -> Result<()> {
        unsafe {
            let old: T = self.get(a, object)?;
            let new: T = self.get(b, object)?;
            self.put(a, object, new)?;
            self.put(b, object, old)
        }
    }
    unsafe fn flags(&self, actor: Object) -> Result<u32> {
        unsafe {
            self.put(131, NIL, 1i32)?;
            let mut flags = match self.int(0, actor)? {
                2 => 2,
                3 => 4,
                4 => 0x40,
                _ => 0,
            };
            if self.int(1, actor)? != 0 {
                flags |= 0x800;
            }
            if self.int(2, actor)? != 0 {
                flags |= 0x02000000;
            }
            if self.int(3, actor)? != 0 {
                flags |= 0x10;
            }
            if self.int(4, actor)? == 0 {
                flags |= 0x4000;
            }
            if self.int(5, actor)? != 0
                || self.int(6, actor)? == 0
                || self.int(61, NIL)? != 5
                || self.int(103, NIL)? != 0
            {
                flags |= 0x00400000;
            }
            self.put(131, NIL, 0i32)?;
            Ok(flags)
        }
    }
    unsafe fn mesh_leaf(&self, actor: Object, coords: *const FCoords, flags: u32) -> Result<()> {
        unsafe { ActorSpriteOps::draw_mesh(self, actor, coords, flags) }
    }
    unsafe fn trig(&self) -> Result<*const [f32; math::NUM_ANGLES]> {
        unsafe { Ok(self.ptr(92, NIL)?.cast()) }
    }
    unsafe fn weapon_coords(&self, weapon: Object) -> Result<FCoords> {
        unsafe {
            let special: FCoords = self.get(93, NIL)?;
            let rotation: FRotator = self.get(13, weapon)?;
            let trig = self.trig()?;
            let rotated = math::coords_div_rotator(&special, rotation, &*trig);
            Ok(math::coords_div_point(rotated, self.get(12, weapon)?))
        }
    }
    unsafe fn sprite(&self, owner: Object, flags: u32) -> Result<()> {
        unsafe {
            self.put(130, NIL, 1i32)?;
            let mut color =
                if self.int(90, NIL)? != 0 && self.int(2, self.ptr(102, self.actor()?)?)? != 0 {
                    plane(0.5, 0.9, 0.5, 0.0)
                } else {
                    plane(1.0, 1.0, 1.0, 0.0)
                };
            let mut texture = self.ptr(8, self.actor()?)?;
            let mut scale = self.float(9, self.actor()?)?;
            let mut saved_next = NIL;
            let mut saved_cur = NIL;
            if self.float(10, self.actor()?)? != 1.0 {
                color = scale_color(color, self.float(10, self.actor()?)?);
                if color.x > 1.0 {
                    color.x = 1.0;
                }
                if color.y > 1.0 {
                    color.y = 1.0;
                }
                if color.z > 1.0 {
                    color.z = 1.0;
                }
            }
            if self.int(7, self.actor()?)? == 7 {
                let mut count = 1i32;
                let mut next = self.ptr(50, texture)?;
                while !next.is_null() && next != texture {
                    count = count.wrapping_add(1);
                    next = self.ptr(50, next)?;
                }
                let actor = self.actor()?;
                let lifespan = self.float(23, actor)?;
                let default_actor = self.ptr(120, actor)?;
                let fraction = clamp(1.0 - lifespan / self.float(23, default_actor)?, 0.0, 1.0);
                let mut num = ((fraction * count as f32).floor() as i32).clamp(0, count - 1);
                while num > 0 {
                    texture = self.ptr(50, texture)?;
                    num -= 1;
                }
                saved_next = self.ptr(50, texture)?;
                saved_cur = self.ptr(51, texture)?;
                self.put(50, texture, NIL)?;
                self.put(51, texture, NIL)?;
            }
            if self.int(60, NIL)? & 0x20 != 0 {
                scale = 1.0;
                if texture.is_null() {
                    texture = self.ptr(105, NIL)?;
                }
            }
            let width = self.float(73, NIL)? * scale * self.int(52, texture)? as f32;
            let height = self.float(73, NIL)? * scale * self.int(53, texture)? as f32;
            if !texture.is_null() {
                let canvas = self.ptr(67, NIL)?;
                let resolved = self.ptr(121, texture)?;
                let mut icon = Icon {
                    canvas,
                    texture: resolved,
                    x: self.float(71, NIL)? - width / 2.0,
                    y: self.float(72, NIL)? - height / 2.0,
                    width,
                    height,
                    color,
                    flags: flags | 0x100 | self.get::<u32>(54, texture)?,
                };
                self.leaf(106, NIL, &mut icon)?;
            }
            // Intentionally LIVE, not the draw type which selected animation.
            if self.int(7, self.actor()?)? == 7 {
                self.put(50, texture, saved_next)?;
                self.put(51, texture, saved_cur)?;
            }
            if self.int(7, owner)? == 8 && self.int(104, NIL)? == 0 {
                self.run(100, NIL)?;
            }
            self.put(130, NIL, 0i32)
        }
    }
    unsafe fn mesh(&self, mut flags: u32) -> Result<()> {
        unsafe {
            self.put(130, NIL, 2i32)?;
            if matches!(self.int(61, NIL)?, 1 | 2 | 3 | 4) {
                flags |= 0x40000000;
            }
            let actor = self.actor()?;
            let coords = self.ptr(64, NIL)?.cast();
            self.mesh_leaf(actor, coords, flags)?;
            let pawn = self.ptr(110, self.actor()?)?;
            if self.int(91, NIL)? != 0 && !pawn.is_null() {
                let special: FCoords = self.get(93, NIL)?;
                let coords: FCoords = self.get(62, NIL)?;
                let ug = math::coords_div_assign(math::coords_inverse(&special), &coords);
                let trig = self.trig()?;
                self.put(81, pawn, math::coords_ortho_rotation(&ug, &*trig))?;
                self.put(82, pawn, ug.origin)?;
                let weapon = self.ptr(80, pawn)?;
                if !weapon.is_null()
                    && !self.ptr(30, weapon)?.is_null()
                    && self.int(11, weapon)? == 0
                {
                    self.exchange::<Object>(30, 24, weapon)?;
                    self.exchange::<f32>(31, 9, weapon)?;
                    self.put(
                        13,
                        weapon,
                        FRotator {
                            pitch: 0,
                            yaw: 0,
                            roll: 0,
                        },
                    )?;
                    let mirror = self.float(63, NIL)?;
                    self.put(63, NIL, 1.0f32)?;
                    let mut coords = self.weapon_coords(weapon)?;
                    let flags = self.flags(weapon)?;
                    self.mesh_leaf(weapon, core::ptr::addr_of_mut!(coords), flags)?;
                    self.exchange::<Object>(30, 24, weapon)?;
                    self.exchange::<f32>(31, 9, weapon)?;
                    if self.int(32, weapon)? != 0 {
                        self.put(33, weapon, (self.int(33, weapon)? == 0) as i32)?;
                    }
                    if (self.int(32, weapon)? != 0
                        && (self.int(34, weapon)? == 0 || self.int(33, weapon)? != 0))
                        || (self.int(35, weapon)? == 0
                            && self.int(36, weapon)? != self.int(37, weapon)?)
                    {
                        if !self.ptr(38, weapon)?.is_null() {
                            self.exchange::<Object>(38, 24, weapon)?;
                            self.exchange::<f32>(39, 9, weapon)?;
                            self.exchange::<i32>(0, 40, weapon)?;
                            self.exchange::<Object>(8, 41, weapon)?;
                            let anim = self.int(14, weapon)?;
                            self.run(122, weapon)?; // typed FName(NAME_All) construction
                            let unlit = self.int(5, weapon)?;
                            let frame = self.float(15, weapon)?;
                            let random = self.float(119, NIL)?;
                            self.put(15, weapon, random)?;
                            self.put(5, weapon, 1i32)?;
                            let mut coords = self.weapon_coords(weapon)?;
                            let flags = self.flags(weapon)?;
                            self.mesh_leaf(weapon, core::ptr::addr_of_mut!(coords), flags)?;
                            self.put(14, weapon, anim)?;
                            self.put(5, weapon, unlit)?;
                            self.put(15, weapon, frame)?;
                            self.exchange::<Object>(8, 41, weapon)?;
                            self.exchange::<Object>(38, 24, weapon)?;
                            self.exchange::<f32>(39, 9, weapon)?;
                            self.exchange::<i32>(0, 40, weapon)?;
                        }
                    }
                    self.put(37, weapon, self.int(36, weapon)?)?;
                    self.put(35, weapon, 0i32)?;
                    self.put(63, NIL, mirror)?;
                }
                let info = self.ptr(83, pawn)?;
                if !info.is_null() {
                    let flag = self.ptr(85, info)?;
                    if !flag.is_null() {
                        let location: FVector = self.get(12, flag)?;
                        let rotation: FRotator = self.get(13, flag)?;
                        self.put(13, flag, self.get::<FRotator>(13, self.actor()?)?)?;
                        let rot: FRotator = self.get(13, flag)?;
                        let trig = self.trig()?;
                        let sine = math::sin_tab_inline(rot.pitch, &*trig);
                        #[cfg(target_arch = "aarch64")]
                        let distance = clamp(20.0f32.mul_add(sine, 2.0), 2.0, 3.0);
                        #[cfg(not(target_arch = "aarch64"))]
                        let distance = clamp(2.0 + 20.0 * sine, 2.0, 3.0);
                        let loc: FVector = self.get(12, self.actor()?)?;
                        let radius = self.float(16, self.actor()?)?;
                        let direction = math::rotator_vector(self.get(13, self.actor()?)?, &*trig);
                        let height = 0.7 * self.float(84, pawn)?;
                        let position = math::vector_add(
                            math::vector_sub(loc, math::vector_scale(direction, distance * radius)),
                            FVector {
                                x: 0.0,
                                y: 0.0,
                                z: height,
                            },
                        );
                        self.put(12, flag, position)?;
                        let mirror = self.float(63, NIL)?;
                        self.put(63, NIL, 1.0f32)?;
                        let coords = self.ptr(64, NIL)?.cast();
                        let flags = self.flags(flag)?;
                        self.mesh_leaf(flag, coords, flags)?;
                        self.put(63, NIL, mirror)?;
                        self.put(12, flag, location)?;
                        self.put(13, flag, rotation)?;
                    }
                }
            }
            self.put(130, NIL, 0i32)
        }
    }
    unsafe fn bounds(&self, owner: Object, show: i32) -> Result<()> {
        unsafe {
            if (show & 0x8000 != 0 && (self.int(17, owner)? != 0 || self.int(18, owner)? != 0))
                || (self.int(90, NIL)? != 0 && self.int(2, owner)? != 0)
            {
                if self.int(19, owner)? != 0 && self.int(20, owner)? != 0 {
                    let radius = self.float(111, owner)?;
                    let inner = radius * self.int(21, owner)?.max(4) as f32 / 256.0;
                    let brightness = self.int(20, owner)? as f32 / 255.0;
                    let mut light = Lighting {
                        brightness,
                        color: plane(0.0, 0.0, 0.0, 0.0),
                    };
                    self.leaf(112, owner, &mut light)?;
                    let color = scale_color(light.color, light.brightness * self.float(22, owner)?);
                    self.leaf(
                        116,
                        NIL,
                        &mut Circle {
                            color,
                            center: self.ptr(27, owner)?.cast(),
                            radius: inner,
                        },
                    )?;
                    self.leaf(
                        116,
                        NIL,
                        &mut Circle {
                            color: scale_color(color, 0.5),
                            center: self.ptr(27, owner)?.cast(),
                            radius,
                        },
                    )?;
                } else {
                    self.run(113, owner)?;
                }
                let mut coords: FCoords = self.get(114, owner)?;
                for index in 0..3 {
                    let (color, axis) = match index {
                        0 => (
                            FVector {
                                x: 1.0,
                                y: 0.0,
                                z: 0.0,
                            },
                            coords.x_axis,
                        ),
                        1 => (
                            FVector {
                                x: 0.0,
                                y: 1.0,
                                z: 0.0,
                            },
                            coords.y_axis,
                        ),
                        _ => (
                            FVector {
                                x: 0.0,
                                y: 0.0,
                                z: 1.0,
                            },
                            coords.z_axis,
                        ),
                    };
                    let mut end = math::vector_add(
                        coords.origin,
                        math::vector_scale(axis, self.float(25, owner)? / 8.0),
                    );
                    self.leaf(
                        115,
                        NIL,
                        &mut Line {
                            color,
                            start: core::ptr::addr_of_mut!(coords.origin),
                            end: core::ptr::addr_of_mut!(end),
                        },
                    )?;
                }
            }
            if show & 0x8000 != 0 && self.int(26, owner)? == 0 {
                let coords: FCoords = self.get(117, owner)?;
                if math::vector_size_sq(coords.x_axis) != 0.0 {
                    let color = if self.int(7, owner)? == 8 {
                        plane(1.0, 0.0, 0.0, 0.0)
                    } else {
                        plane(1.0, 1.0, 1.0, 0.0)
                    };
                    self.leaf(118, NIL, &mut BoxDraw { color, coords })?;
                }
            }
            Ok(())
        }
    }
}
impl<T: ActorSpriteOps<Error = ()>> ActorSpritePolicyOps for T {}
fn plane(x: f32, y: f32, z: f32, w: f32) -> FPlane {
    FPlane { x, y, z, w }
}
fn clamp(x: f32, lo: f32, hi: f32) -> f32 {
    if x < lo {
        lo
    } else if x < hi {
        x
    } else {
        hi
    }
}
/// Executes shared actor-sprite policy through a typed host, preserving its failure result.
///
/// Source restoration behavior is unchanged: a failed operation stops immediately and
/// does not roll back fields already changed by the source policy.
///
/// # Safety
/// The host must keep every object and coordinate buffer live for each operation.
pub unsafe fn draw_with_ops<S: ActorSpriteOps<Error = ()>>(ops: &S) -> Result<(), ()> {
    unsafe fn body<S: ActorSpritePolicyOps>(s: &S) -> Result<()> {
        unsafe {
            s.run(108, NIL)?;
            let flags = s.flags(s.actor()?)?;
            let show = s.int(60, NIL)?;
            let owner = s.actor()?;
            if ((s.int(7, owner)? == 8 && s.int(60, NIL)? & 0x20 != 0)
                || s.int(7, s.actor()?)? == 1
                || s.int(7, s.actor()?)? == 7
                || s.int(60, NIL)? & 0x20 != 0)
                && !s.ptr(8, s.actor()?)?.is_null()
            {
                s.sprite(owner, flags)?;
            } else if s.int(7, s.actor()?)? == 8 {
                if s.int(104, NIL)? == 0 {
                    s.run(100, NIL)?;
                }
            } else if s.int(7, s.actor()?)? == 2 && !s.ptr(24, s.actor()?)?.is_null() {
                s.mesh(flags)?;
            }
            s.bounds(owner, show)?;
            s.run(109, NIL)
        }
    }
    unsafe { body(ops) }
}

/// Runs actor sprite policy through the legacy Native C ABI adapter.
///
/// # Safety
/// The callback must implement the existing actor sprite service contract.
pub unsafe fn draw(services: &ActorSpriteServices) -> bool {
    unsafe { draw_with_ops(services).is_ok() }
}

