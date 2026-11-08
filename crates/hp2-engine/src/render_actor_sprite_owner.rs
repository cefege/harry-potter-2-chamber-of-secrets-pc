//! Core-backed actor sprite operations for the canonical renderer.
#[path = "render_actor_particle_provider.rs"]
mod particle_provider;

use super::super::URender;
use hp2_abi::engine::generated::{
    self as generated, AActor, ALevelInfo, APawn, APlayerReplicationInfo, FSceneNode,
    UBoolProperty, UBitmap, UObject, UProperty, UStruct, UTexture, UViewport, UClient,
};
use hp2_abi::render_scene::{self, FDynamicSprite};
use hp2_abi::{FCoords, FPlane, FVector};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::name_owner::FindName;
use hp2_render::actor_sprite::{
    self, ActorCoordsField, ActorF32Field, ActorI32Field, ActorObjectField, ActorRotatorField,
    ActorSpriteOps, ActorU32Field, ActorVectorField, BoxDraw, Circle, Icon, Lighting, Line, Object,
};
use std::{cell::RefCell, ffi::c_void, ptr};

struct CoreActorSpriteOps<'a> {
    access: CoreAccess<'a>,
    render: *mut URender,
    frame: *mut FSceneNode,
    sprite: *mut FDynamicSprite,
    error: RefCell<Option<RuntimeError>>,
    poly_scope: RefCell<i32>,
    editor_hack: RefCell<i32>,
}

impl CoreActorSpriteOps<'_> {
    fn record(&self, error: RuntimeError) {
        let mut slot = self.error.borrow_mut();
        if slot.is_none() {
            *slot = Some(error);
        }
    }

    fn remember<T>(&self, result: RuntimeResult<T>) -> Result<T, ()> {
        result.map_err(|error| {
            self.record(error);
        })
    }

    fn fail<T>(&self, message: &'static str) -> Result<T, ()> {
        self.record(RuntimeError::new(message));
        Err(())
    }

    fn actor_ptr(&self, object: Object) -> Result<*mut AActor, ()> {
        let actor = if object.is_null() {
            unsafe { (*self.sprite).actor.cast::<AActor>() }
        } else {
            object.cast::<AActor>()
        };
        if actor.is_null() { self.fail("Actor sprite operation requires an actor") } else { Ok(actor) }
    }

    fn weapon_ptr(&self, object: Object) -> Result<*mut generated::AWeapon, ()> {
        if object.is_null() { return self.fail("Actor sprite weapon operation requires a weapon"); }
        Ok(object.cast())
    }

    unsafe fn property<T: Copy>(
        &self,
        object: *mut UObject,
        name: &str,
        kind: &str,
        expected_size: usize,
    ) -> RuntimeResult<T> {
        if object.is_null() { return Err(RuntimeError::new("Null reflected actor-sprite property object")); }
        let name = self.access.intern_str(name, FindName::Find)?;
        let property_class = self.access.native_class(&format!("Core.{kind}Property"))?;
        let class = unsafe { (*object).class.cast::<UStruct>() };
        let property = unsafe { self.access.find_field(class, name, property_class) }.cast::<UProperty>();
        if property.is_null() { return Err(RuntimeError::new("Actor-sprite reflected property was not found")); }
        let meta = unsafe { &*property };
        if meta.array_dim != 1 || meta.element_size as usize != expected_size {
            return Err(RuntimeError::new("Actor-sprite reflected property storage mismatch"));
        }
        let address = unsafe { self.access.property_address(object, property)? };
        if (address as usize) % std::mem::align_of::<T>() != 0 {
            return Err(RuntimeError::new("Unaligned actor-sprite reflected property"));
        }
        Ok(unsafe { address.cast::<T>().read() })
    }

    unsafe fn set_property<T: Copy>(
        &self,
        object: *mut UObject,
        name: &str,
        kind: &str,
        value: T,
    ) -> RuntimeResult<()> {
        if object.is_null() { return Err(RuntimeError::new("Null reflected actor-sprite property object")); }
        let name_id = self.access.intern_str(name, FindName::Find)?;
        let property_class = self.access.native_class(&format!("Core.{kind}Property"))?;
        let class = unsafe { (*object).class.cast::<UStruct>() };
        let property = unsafe { self.access.find_field(class, name_id, property_class) }.cast::<UProperty>();
        if property.is_null() { return Err(RuntimeError::new("Actor-sprite reflected property was not found")); }
        let meta = unsafe { &*property };
        if meta.array_dim != 1 || meta.element_size as usize != std::mem::size_of::<T>() {
            return Err(RuntimeError::new("Actor-sprite reflected property storage mismatch"));
        }
        let address = unsafe { self.access.property_address(object, property)? };
        if (address as usize) % std::mem::align_of::<T>() != 0 {
            return Err(RuntimeError::new("Unaligned actor-sprite reflected property"));
        }
        unsafe { address.cast::<T>().write(value) };
        Ok(())
    }

    unsafe fn bool_property(&self, object: *mut UObject, name: &str) -> RuntimeResult<bool> {
        let name_id = self.access.intern_str(name, FindName::Find)?;
        let property_class = self.access.native_class("Core.BoolProperty")?;
        let class = unsafe { (*object).class.cast::<UStruct>() };
        let property = unsafe { self.access.find_field(class, name_id, property_class) }.cast::<UBoolProperty>();
        if property.is_null() { return Err(RuntimeError::new("Actor-sprite reflected boolean was not found")); }
        let meta = unsafe { &*property.cast::<UProperty>() };
        if meta.array_dim != 1 || meta.element_size != 4 {
            return Err(RuntimeError::new("Actor-sprite reflected boolean storage mismatch"));
        }
        let address = unsafe { self.access.property_address(object, property.cast())? };
        if (address as usize) % std::mem::align_of::<u32>() != 0 {
            return Err(RuntimeError::new("Unaligned actor-sprite reflected boolean"));
        }
        let storage = unsafe { address.cast::<u32>().read() };
        Ok(storage & unsafe { (*property).bit_mask } != 0)
    }

    unsafe fn set_bool_property(&self, object: *mut UObject, name: &str, value: bool) -> RuntimeResult<()> {
        let name_id = self.access.intern_str(name, FindName::Find)?;
        let property_class = self.access.native_class("Core.BoolProperty")?;
        let class = unsafe { (*object).class.cast::<UStruct>() };
        let property = unsafe { self.access.find_field(class, name_id, property_class) }.cast::<UBoolProperty>();
        if property.is_null() { return Err(RuntimeError::new("Actor-sprite reflected boolean was not found")); }
        let meta = unsafe { &*property.cast::<UProperty>() };
        if meta.array_dim != 1 || meta.element_size != 4 {
            return Err(RuntimeError::new("Actor-sprite reflected boolean storage mismatch"));
        }
        let address = unsafe { self.access.property_address(object, property.cast())? };
        if (address as usize) % std::mem::align_of::<u32>() != 0 {
            return Err(RuntimeError::new("Unaligned actor-sprite reflected boolean"));
        }
        let address = address.cast::<u32>();
        let mut bits = unsafe { address.read() };
        let mask = unsafe { (*property).bit_mask };
        bits = if value { bits | mask } else { bits & !mask };
        unsafe { address.write(bits) };
        Ok(())
    }

    fn frame_view(&self) -> *mut render_scene::FSceneNode { self.frame.cast() }
    fn viewport(&self) -> Result<*mut UViewport, ()> {
        if self.frame.is_null() { return self.fail("Actor sprite renderer requires a scene frame"); }
        let viewport = unsafe { (*self.frame_view()).viewport.cast::<UViewport>() };
        if viewport.is_null() { self.fail("Actor sprite frame has no viewport") } else { Ok(viewport) }
    }

    fn weapon_field_byte(&self, weapon: Object, property: &'static str) -> Result<i32, ()> {
        let weapon = self.weapon_ptr(weapon)?;
        self.remember(unsafe { self.property::<u8>(weapon.cast(), property, "Byte", 1) }).map(i32::from)
    }

    fn set_weapon_field_byte(&self, weapon: Object, property: &'static str, value: i32) -> Result<(), ()> {
        let weapon = self.weapon_ptr(weapon)?;
        self.remember(unsafe { self.set_property(weapon.cast(), property, "Byte", value as u8) })
    }

    fn weapon_field_i32(&self, weapon: Object, property: &'static str) -> Result<i32, ()> {
        let weapon = self.weapon_ptr(weapon)?;
        self.remember(unsafe { self.property::<i32>(weapon.cast(), property, "Int", 4) })
    }

    fn set_weapon_field_i32(&self, weapon: Object, property: &'static str, value: i32) -> Result<(), ()> {
        let weapon = self.weapon_ptr(weapon)?;
        self.remember(unsafe { self.set_property(weapon.cast(), property, "Int", value) })
    }

    fn weapon_field_f32(&self, weapon: Object, property: &'static str) -> Result<f32, ()> {
        let weapon = self.weapon_ptr(weapon)?;
        self.remember(unsafe { self.property::<f32>(weapon.cast(), property, "Float", 4) })
    }

    fn set_weapon_field_f32(&self, weapon: Object, property: &'static str, value: f32) -> Result<(), ()> {
        let weapon = self.weapon_ptr(weapon)?;
        self.remember(unsafe { self.set_property(weapon.cast(), property, "Float", value) })
    }

    fn weapon_field_object(&self, weapon: Object, property: &'static str) -> Result<Object, ()> {
        let weapon = self.weapon_ptr(weapon)?;
        self.remember(unsafe { self.property::<Object>(weapon.cast(), property, "Object", std::mem::size_of::<Object>()) })
    }

    fn set_weapon_field_object(&self, weapon: Object, property: &'static str, value: Object) -> Result<(), ()> {
        let weapon = self.weapon_ptr(weapon)?;
        self.remember(unsafe { self.set_property(weapon.cast(), property, "Object", value) })
    }

    fn weapon_bool(&self, weapon: Object, property: &'static str) -> Result<i32, ()> {
        let weapon = self.weapon_ptr(weapon)?;
        self.remember(unsafe { self.bool_property(weapon.cast(), property) }).map(i32::from)
    }

    fn set_weapon_bool(&self, weapon: Object, property: &'static str, value: i32) -> Result<(), ()> {
        let weapon = self.weapon_ptr(weapon)?;
        self.remember(unsafe { self.set_bool_property(weapon.cast(), property, value != 0) })
    }

    fn run_particle(&self) -> RuntimeResult<()> {
        unsafe { particle_provider::draw_particles(self.access, self.render, self.frame, self.sprite) }
    }
}

impl ActorSpriteOps for CoreActorSpriteOps<'_> {
    type Error = ();

    unsafe fn read_i32(&self, object: Object, field: ActorI32Field) -> Result<i32, ()> {
        let actor = || self.actor_ptr(object);
        match field {
            ActorI32Field::ActorStyle => actor().map(|p| unsafe { (*p).style as i32 }),
            ActorI32Field::ActorNoSmooth => actor().map(|p| unsafe { i32::from((*p).b_no_smooth()) }),
            ActorI32Field::ActorSelected => actor().map(|p| unsafe { i32::from((*p).b_selected()) }),
            ActorI32Field::ActorMeshEnviroMap => actor().map(|p| unsafe { i32::from((*p).b_mesh_enviro_map()) }),
            ActorI32Field::ActorMeshCurvy => actor().map(|p| unsafe { i32::from((*p).b_mesh_curvy()) }),
            ActorI32Field::ActorUnlit => actor().map(|p| unsafe { i32::from((*p).b_unlit()) }),
            ActorI32Field::ActorZoneNumber => actor().map(|p| unsafe { (*p).region.zone_number as i32 }),
            ActorI32Field::ActorDrawType => actor().map(|p| unsafe { (*p).draw_type as i32 }),
            ActorI32Field::ActorHidden => actor().map(|p| unsafe { i32::from((*p).b_hidden()) }),
            ActorI32Field::ActorCollideWorld => actor().map(|p| unsafe { i32::from((*p).b_collide_world()) }),
            ActorI32Field::ActorCollideActors => actor().map(|p| unsafe { i32::from((*p).b_collide_actors()) }),
            ActorI32Field::ActorLightType => actor().map(|p| unsafe { (*p).light_type as i32 }),
            ActorI32Field::ActorLightBrightness => actor().map(|p| unsafe { (*p).light_brightness as i32 }),
            ActorI32Field::ActorLightRadiusInner => actor().map(|p| unsafe { (*p).light_radius_inner as i32 }),
            ActorI32Field::ActorDeleteMe => actor().map(|p| unsafe { i32::from((*p).b_delete_me()) }),
            ActorI32Field::WeaponAnimSequence => self.weapon_ptr(object).map(|p| unsafe { i32::from_ne_bytes((*p).base.base.anim_sequence) }),
            ActorI32Field::WeaponSteadyFlash3rd => self.weapon_ptr(object).map(|p| unsafe { i32::from((*p).base.b_steady_flash3rd()) }),
            ActorI32Field::WeaponSteadyToggle => self.weapon_bool(object, "bSteadyToggle"),
            ActorI32Field::WeaponToggleSteadyFlash => self.weapon_bool(object, "bToggleSteadyFlash"),
            ActorI32Field::WeaponFirstFrame => self.weapon_bool(object, "bFirstFrame"),
            ActorI32Field::WeaponFlashCount => self.weapon_ptr(object).map(|p| unsafe { (*p).base.flash_count as i32 }),
            ActorI32Field::WeaponOldFlashCount => self.weapon_field_byte(object, "OldFlashCount"),
            ActorI32Field::WeaponMuzzleFlashStyle => self.weapon_field_byte(object, "MuzzleFlashStyle"),
            ActorI32Field::TextureWidth => if object.is_null() { self.fail("Texture width requested for null texture") } else { Ok(unsafe { (*object.cast::<UBitmap>()).u_size }) },
            ActorI32Field::TextureHeight => if object.is_null() { self.fail("Texture height requested for null texture") } else { Ok(unsafe { (*object.cast::<UBitmap>()).v_size }) },
            ActorI32Field::ViewportShowFlags => self.viewport().and_then(|vp| { let actor = unsafe { (*vp).actor.cast::<generated::APlayerPawn>() }; if actor.is_null() { self.fail("Viewport has no player pawn") } else { Ok(unsafe { (*actor).show_flags }) } }),
            ActorI32Field::RenderMode => self.viewport().and_then(|vp| { let actor = unsafe { (*vp).actor.cast::<generated::APlayerPawn>() }; if actor.is_null() { self.fail("Viewport has no player pawn") } else { Ok(unsafe { (*actor).rend_map }) } }),
            ActorI32Field::RenderPass => Ok(i32::from(self.access.process_mode().is_editor)),
            ActorI32Field::HasSpecialCoords => Ok(unsafe { hp2_render::mesh_shared::HasSpecialCoords }),
            ActorI32Field::NoLighting => self.viewport().and_then(|vp| { let client = unsafe { (*vp).outer.cast::<UClient>() }; if client.is_null() { self.fail("Viewport has no outer client") } else { Ok(i32::from(unsafe { (*client).no_lighting() })) } }),
            ActorI32Field::IsOrtho => self.viewport().and_then(|vp| { let actor = unsafe { (*vp).actor.cast::<generated::APlayerPawn>() }; if actor.is_null() { self.fail("Viewport has no player pawn") } else { Ok(i32::from(matches!(unsafe { (*actor).rend_map }, 13..=15))) } }),
            ActorI32Field::PolyScope => Ok(*self.poly_scope.borrow()),
            ActorI32Field::EditorHack => Ok(*self.editor_hack.borrow()),
            _ => self.fail("Unsupported actor sprite integer field"),
        }
    }

    unsafe fn write_i32(&self, object: Object, field: ActorI32Field, value: i32) -> Result<(), ()> {
        match field {
            ActorI32Field::WeaponAnimSequence => { let p = self.weapon_ptr(object)?; unsafe { (*p).base.base.anim_sequence = value.to_ne_bytes(); } Ok(()) }
            ActorI32Field::WeaponSteadyToggle => self.set_weapon_bool(object, "bSteadyToggle", value),
            ActorI32Field::WeaponToggleSteadyFlash => self.set_weapon_bool(object, "bToggleSteadyFlash", value),
            ActorI32Field::WeaponFirstFrame => self.set_weapon_bool(object, "bFirstFrame", value),
            ActorI32Field::ActorStyle => { let p = self.actor_ptr(object)?; unsafe { (*p).style = value as u8; } Ok(()) }
            ActorI32Field::WeaponOldFlashCount => self.set_weapon_field_byte(object, "OldFlashCount", value),
            ActorI32Field::WeaponMuzzleFlashStyle => self.set_weapon_field_byte(object, "MuzzleFlashStyle", value),
            ActorI32Field::PolyScope => { *self.poly_scope.borrow_mut() = value; Ok(()) }
            ActorI32Field::EditorHack => { *self.editor_hack.borrow_mut() = value; Ok(()) }
            _ => self.fail("Unsupported actor sprite integer field write"),
        }
    }

    unsafe fn read_f32(&self, object: Object, field: ActorF32Field) -> Result<f32, ()> {
        let actor = || self.actor_ptr(object);
        match field {
            ActorF32Field::ActorDrawScale => actor().map(|p| unsafe { (*p).draw_scale }),
            ActorF32Field::ActorScaleGlow => actor().map(|p| unsafe { (*p).scale_glow }),
            ActorF32Field::AnimFrame => actor().map(|p| unsafe { (*p).anim_frame }),
            ActorF32Field::CollisionRadius => actor().map(|p| unsafe { (*p).collision_radius }),
            ActorF32Field::LevelBrightness => actor().and_then(|p| { let level = unsafe { (*p).level.cast::<ALevelInfo>() }; if level.is_null() { self.fail("Actor has no level brightness record") } else { self.remember(unsafe { self.property::<u8>(level.cast(), "AmbientBrightness", "Byte", 1) }).map(f32::from) } }),
            ActorF32Field::LifeSpan => actor().map(|p| unsafe { (*p).life_span }),
            ActorF32Field::CollisionHeight => actor().map(|p| unsafe { (*p).collision_height }),
            ActorF32Field::WeaponThirdPersonScale => self.weapon_ptr(object).map(|p| unsafe { (*p).base.third_person_scale }),
            ActorF32Field::WeaponMuzzleFlashScale => self.weapon_field_f32(object, "MuzzleFlashScale"),
            ActorF32Field::Mirror => if self.frame.is_null() { self.fail("Actor sprite mirror requested without frame") } else { Ok(unsafe { (*self.frame.cast::<render_scene::FSceneNode>()).mirror }) },
            ActorF32Field::ScreenX => if self.sprite.is_null() { self.fail("Actor sprite screen coordinate requested without sprite") } else { Ok(unsafe { (*self.sprite).screen_x }) },
            ActorF32Field::ScreenY => if self.sprite.is_null() { self.fail("Actor sprite screen coordinate requested without sprite") } else { Ok(unsafe { (*self.sprite).screen_y }) },
            ActorF32Field::Perspective => if self.sprite.is_null() { self.fail("Actor sprite perspective requested without sprite") } else { Ok(unsafe { (*self.sprite).persp }) },
            ActorF32Field::PawnBaseEyeHeight => { let a = actor()?; if unsafe { !hp2_core::object_model::is_a(a.cast(), self.access.native_class("Engine.Pawn").map_err(|e| { self.record(e); })?.cast()) } { self.fail("Actor is not a pawn") } else { Ok(unsafe { (*a.cast::<APawn>()).base_eye_height }) } }
            ActorF32Field::RandomFrame => Ok(self.access.rand_range_float(0.0, 1.0)),
            ActorF32Field::WorldLightRadius => actor().map(|p| unsafe { (*p).light_radius as f32 }),
            _ => self.fail("Unsupported actor sprite float field"),
        }
    }

    unsafe fn write_f32(&self, object: Object, field: ActorF32Field, value: f32) -> Result<(), ()> {
        match field {
            ActorF32Field::ActorDrawScale => { let p = self.actor_ptr(object)?; unsafe { (*p).draw_scale = value; } Ok(()) }
            ActorF32Field::ActorScaleGlow => { let p = self.actor_ptr(object)?; unsafe { (*p).scale_glow = value; } Ok(()) }
            ActorF32Field::AnimFrame => { let p = self.actor_ptr(object)?; unsafe { (*p).anim_frame = value; } Ok(()) }
            ActorF32Field::WeaponThirdPersonScale => { let p = self.weapon_ptr(object)?; unsafe { (*p).base.third_person_scale = value; } Ok(()) }
            ActorF32Field::WeaponMuzzleFlashScale => self.set_weapon_field_f32(object, "MuzzleFlashScale", value),
            ActorF32Field::Mirror => { if self.frame.is_null() { return self.fail("Actor sprite mirror write without frame"); } unsafe { (*self.frame.cast::<render_scene::FSceneNode>()).mirror = value; } Ok(()) }
            _ => self.fail("Unsupported actor sprite float field write"),
        }
    }

    unsafe fn read_object(&self, object: Object, field: ActorObjectField) -> Result<Object, ()> {
        match field {
            ActorObjectField::ActorLocationAddress => { let a = self.actor_ptr(object)?; Ok(unsafe { ptr::addr_of_mut!((*a).location).cast() }) }
            ActorObjectField::ActorTexture => { let a = self.actor_ptr(object)?; Ok(unsafe { (*a).texture }) }
            ActorObjectField::ActorMesh => { let a = self.actor_ptr(object)?; Ok(unsafe { (*a).mesh }) }
            ActorObjectField::FrameCoordsAddress => { if self.frame.is_null() { self.fail("Actor sprite frame coordinates requested without frame") } else { Ok(unsafe { ptr::addr_of_mut!((*self.frame.cast::<render_scene::FSceneNode>()).coords).cast() }) } }
            ActorObjectField::WeaponThirdPersonMesh => { let p = self.weapon_ptr(object)?; Ok(unsafe { (*p).base.third_person_mesh }) }
            ActorObjectField::WeaponMuzzleFlashMesh => self.weapon_field_object(object, "MuzzleFlashMesh"),
            ActorObjectField::WeaponMuzzleFlashTexture => self.weapon_field_object(object, "MuzzleFlashTexture"),
            ActorObjectField::AnimNext => if object.is_null() { self.fail("Texture animation next requested for null texture") } else { Ok(unsafe { (*object.cast::<UTexture>()).anim_next }) },
            ActorObjectField::AnimCurrent => if object.is_null() { self.fail("Texture animation current requested for null texture") } else { Ok(unsafe { (*object.cast::<UTexture>()).anim_cur }) },
            ActorObjectField::Canvas => self.viewport().map(|vp| unsafe { (*vp).canvas }),
            ActorObjectField::SpriteActor => if self.sprite.is_null() { self.fail("Actor sprite requested without sprite") } else { Ok(unsafe { (*self.sprite).actor }) },
            ActorObjectField::ActorPawn => {
                let actor = self.actor_ptr(object)?;
                let class = self.remember(self.access.native_class("Engine.Pawn"))?;
                Ok(if unsafe { hp2_core::object_model::is_a(actor.cast(), class.cast()) } {
                    actor.cast()
                } else {
                    ptr::null_mut()
                })
            }
            ActorObjectField::PawnWeapon => { let a = self.actor_ptr(object)?; Ok(unsafe { a.cast::<APawn>().as_ref().map_or(ptr::null_mut(), |p| p.weapon) }) }
            ActorObjectField::PawnPlayerInfo => { let a = self.actor_ptr(object)?; Ok(unsafe { a.cast::<APawn>().as_ref().map_or(ptr::null_mut(), |p| p.player_replication_info) }) }
            ActorObjectField::PlayerInfoHasFlag => if object.is_null() { self.fail("Player flag requested for null replication info") } else { Ok(unsafe { (*object.cast::<APlayerReplicationInfo>()).has_flag }) }
            ActorObjectField::ActorHitActor => { let a = self.actor_ptr(object)?; Ok(unsafe { (*a).hit_actor }) }
            ActorObjectField::DefaultTexture => { let c = self.remember(self.access.native_class("Engine.Texture"))?; self.remember(unsafe { self.access.default_object(c.cast()) }).map(|p| p.cast()) }
            ActorObjectField::ActorDefault => { let a = self.actor_ptr(object)?; let class = unsafe { (*a.cast::<UObject>()).class }; self.remember(unsafe { self.access.default_object(class.cast()) }).map(|p| p.cast()) }
            ActorObjectField::TextureResolved => {
                if object.is_null() { return self.fail("Animated texture resolution requested for null texture"); }
                let texture = object.cast::<UTexture>();
                let time = self.viewport().map(|vp| unsafe { i64::from_ne_bytes((*vp).current_time) })?;
                self.remember(unsafe { crate::texture_owner::get(self.access, texture, time) }).map(|p| p.cast())
            }
            ActorObjectField::TrigTable => Ok(hp2_core::math::trig_table().as_ptr().cast_mut().cast()),
            _ => self.fail("Unsupported actor sprite object field"),
        }
    }

    unsafe fn write_object(&self, object: Object, field: ActorObjectField, value: Object) -> Result<(), ()> {
        match field {
            ActorObjectField::ActorTexture => { let a = self.actor_ptr(object)?; unsafe { (*a).texture = value; } Ok(()) }
            ActorObjectField::ActorMesh => { let a = self.actor_ptr(object)?; unsafe { (*a).mesh = value; } Ok(()) }
            ActorObjectField::WeaponThirdPersonMesh => { let p = self.weapon_ptr(object)?; unsafe { (*p).base.third_person_mesh = value; } Ok(()) }
            ActorObjectField::WeaponMuzzleFlashMesh => self.set_weapon_field_object(object, "MuzzleFlashMesh", value),
            ActorObjectField::WeaponMuzzleFlashTexture => self.set_weapon_field_object(object, "MuzzleFlashTexture", value),
            ActorObjectField::AnimNext => { if object.is_null() { return self.fail("Texture animation write for null texture"); } unsafe { (*object.cast::<UTexture>()).anim_next = value; } Ok(()) }
            ActorObjectField::AnimCurrent => { if object.is_null() { return self.fail("Texture animation write for null texture"); } unsafe { (*object.cast::<UTexture>()).anim_cur = value; } Ok(()) }
            _ => self.fail("Unsupported actor sprite object field write"),
        }
    }

    unsafe fn read_coords(&self, object: Object, field: ActorCoordsField) -> Result<FCoords, ()> {
        match field {
            ActorCoordsField::SpecialCoords => Ok(unsafe { hp2_render::mesh_shared::SpecialCoords }),
            ActorCoordsField::ViewCoords => if self.frame.is_null() { self.fail("Actor sprite view coordinates requested without frame") } else { Ok(unsafe { (*self.frame.cast::<render_scene::FSceneNode>()).coords }) },
            ActorCoordsField::LocalCoords => { let a = self.actor_ptr(object)?; Ok(unsafe { crate::actor::to_local(a) }) }
            ActorCoordsField::RenderBoundsCoords => { let a = self.actor_ptr(object)?; self.remember(unsafe { super::dynamic::render_bounds_with_ops(self.access, self.render, self.frame, a) }) }
            _ => self.fail("Unsupported actor sprite coordinates"),
        }
    }

    unsafe fn read_rotator(&self, object: Object, field: ActorRotatorField) -> Result<hp2_core::math::FRotator, ()> {
        match field {
            ActorRotatorField::ActorRotation => { let a = self.actor_ptr(object)?; Ok(unsafe { let r = (*a).rotation; hp2_core::math::FRotator { pitch: r.pitch, yaw: r.yaw, roll: r.roll } }) }
            ActorRotatorField::PawnWeaponRotation => { let a = self.actor_ptr(object)?; Ok(unsafe { let r = (*a.cast::<APawn>()).weapon_rot; hp2_core::math::FRotator { pitch: r.pitch, yaw: r.yaw, roll: r.roll } }) }
            _ => self.fail("Unsupported actor sprite rotation"),
        }
    }
    unsafe fn write_rotator(&self, object: Object, field: ActorRotatorField, value: hp2_core::math::FRotator) -> Result<(), ()> {
        match field {
            ActorRotatorField::ActorRotation => { let a = self.actor_ptr(object)?; unsafe { let r = hp2_abi::engine::generated::FRotator { pitch: value.pitch, yaw: value.yaw, roll: value.roll }; (*a).rotation = r; } Ok(()) }
            ActorRotatorField::PawnWeaponRotation => { let a = self.actor_ptr(object)?; unsafe { let r = hp2_abi::engine::generated::FRotator { pitch: value.pitch, yaw: value.yaw, roll: value.roll }; (*a.cast::<APawn>()).weapon_rot = r; } Ok(()) }
            _ => self.fail("Unsupported actor sprite rotation write"),
        }
    }
    unsafe fn read_vector(&self, object: Object, field: ActorVectorField) -> Result<FVector, ()> {
        match field {
            ActorVectorField::ActorLocation => { let a = self.actor_ptr(object)?; Ok(unsafe { (*a).location }) }
            ActorVectorField::PawnWeaponLocation => { let a = self.actor_ptr(object)?; Ok(unsafe { (*a.cast::<APawn>()).weapon_loc }) }
            _ => self.fail("Unsupported actor sprite vector"),
        }
    }
    unsafe fn write_vector(&self, object: Object, field: ActorVectorField, value: FVector) -> Result<(), ()> {
        match field {
            ActorVectorField::ActorLocation => { let a = self.actor_ptr(object)?; unsafe { (*a).location = value; } Ok(()) }
            ActorVectorField::PawnWeaponLocation => { let a = self.actor_ptr(object)?; unsafe { (*a.cast::<APawn>()).weapon_loc = value; } Ok(()) }
            _ => self.fail("Unsupported actor sprite vector write"),
        }
    }
    unsafe fn read_u32(&self, object: Object, field: ActorU32Field) -> Result<u32, ()> {
        match field {
            ActorU32Field::TexturePolyFlags => if object.is_null() { self.fail("Texture flags requested for null texture") } else { Ok(unsafe { (*object.cast::<UTexture>()).poly_flags }) },
            _ => self.fail("Unsupported actor sprite u32 field"),
        }
    }
    unsafe fn actor(&self) -> Result<Object, ()> { if self.sprite.is_null() { self.fail("Actor sprite host has no sprite") } else { Ok(unsafe { (*self.sprite).actor }) } }
    unsafe fn draw_mesh(&self, actor: Object, coords: *const FCoords, flags: u32) -> Result<(), ()> { self.remember(unsafe { super::mesh::draw_actor_mesh(self.access, self.render, self.frame, self.sprite, actor.cast(), coords, flags) }) }
    unsafe fn draw_icon(&self, icon: &mut Icon) -> Result<(), ()> {
        if icon.canvas.is_null() || icon.texture.is_null() { return self.fail("Actor sprite icon requires a canvas and texture"); }
        let args = crate::canvas_text::Hp2CanvasPatternArgs { tex: icon.texture, x: icon.x, y: icon.y, xl: icon.width, yl: icon.height, scale: 1.0, org_x: 0.0, org_y: 0.0, span: unsafe { (*self.sprite).span_buffer.cast() }, z: unsafe { (*self.sprite).base.min_z }, color: crate::canvas_text::Hp2Plane { x: icon.color.x, y: icon.color.y, z: icon.color.z, w: icon.color.w }, fog: crate::canvas_text::Hp2Plane { x: 0.0, y: 0.0, z: 0.0, w: 0.0 }, flags: icon.flags };
        self.remember(unsafe { crate::canvas_owner::draw_icon(self.access, icon.canvas.cast(), &args) })
    }
    unsafe fn push_actor_hit(&self) -> Result<(), ()> {
        let viewport = self.viewport()?;
        if unsafe { (*viewport).hit_testing } == 0 { return Ok(()); }
        #[repr(C)]
        struct ActorHitProxy {
            base: generated::HHitProxy,
            actor: *mut c_void,
        }
        let actor = self.actor_ptr(ptr::null_mut())?;
        // WGPU ignores the payload, so no native HHitProxy vtable is fabricated.
        let mut hit: ActorHitProxy = unsafe { std::mem::zeroed() };
        hit.actor = unsafe { crate::actor::get_hit_actor(actor) };
        self.remember(unsafe {
            crate::platform_owner::push_hit(
                self.access, viewport, ptr::addr_of_mut!(hit.base),
                std::mem::size_of::<ActorHitProxy>() as i32,
            )
        })
    }
    unsafe fn pop_actor_hit(&self) -> Result<(), ()> {
        let viewport = self.viewport()?;
        if unsafe { (*viewport).hit_testing } == 0 { return Ok(()); }
        self.remember(unsafe { crate::platform_owner::pop_hit(self.access, viewport, false) })
    }
    unsafe fn render_selected(&self) -> Result<(), ()> { self.remember(self.run_particle()) }
    unsafe fn render_bounds(&self, actor: Object) -> Result<(), ()> { self.remember(unsafe { super::mesh::draw_collision_bounds(self.access, self.render, self.frame, actor.cast()) }) }
    unsafe fn render_light(&self, actor: Object, lighting: &mut Lighting) -> Result<(), ()> {
        self.remember(unsafe {
            super::super::lighting::global_lighting(
                self.access,
                actor,
                &mut lighting.brightness,
                &mut lighting.color,
            )
        })
    }
    unsafe fn draw_circle(&self, circle: &mut Circle) -> Result<(), ()> { if circle.center.is_null() { return self.fail("Actor sprite circle has no center"); } self.remember(unsafe { super::mesh::draw_circle(self.access, self.render, self.frame, circle.color, *circle.center, circle.radius, false) }) }
    unsafe fn draw_line(&self, line: &mut Line) -> Result<(), ()> { if line.start.is_null() || line.end.is_null() { return self.fail("Actor sprite line has no endpoints"); } self.remember(unsafe { super::mesh::draw_line(self.access, self.render, self.frame, line.color, 2, *line.start, *line.end) }) }
    unsafe fn draw_selection_box(&self, box_draw: &mut BoxDraw) -> Result<(), ()> { self.remember(unsafe { super::mesh::draw_box(self.access, self.render, self.frame, box_draw.color, 2, box_draw.coords) }) }
    unsafe fn render_weapon_animation(&self, weapon: Object) -> Result<(), ()> {
        let name = self.remember(self.access.intern_str("All", FindName::Find))?;
        let p = self.weapon_ptr(weapon)?;
        unsafe { (*p).base.base.anim_sequence = name.to_ne_bytes(); }
        Ok(())
    }
}

/// Draws one dynamic actor sprite through the shared source policy and typed Core leaves.
///
/// # Safety
/// `render`, `frame`, and `sprite` must be live records in the active Engine render graph.
pub(super) unsafe fn draw_actor_sprite(
    access: CoreAccess<'_>,
    render: *mut URender,
    frame: *mut FSceneNode,
    sprite: *mut FDynamicSprite,
) -> RuntimeResult<()> {
    if render.is_null() || frame.is_null() || sprite.is_null() {
        return Err(RuntimeError::new("Invalid canonical actor sprite draw arguments"));
    }
    let host = CoreActorSpriteOps { access, render, frame, sprite, error: RefCell::new(None), poly_scope: RefCell::new(0), editor_hack: RefCell::new(0) };
    let result = unsafe { actor_sprite::draw_with_ops(&host) };
    if let Some(error) = host.error.into_inner() { return Err(error); }
    result.map_err(|()| RuntimeError::new("Canonical actor sprite policy failed without a recorded Core error"))
}

#[cfg(test)]
mod render_host_contracts {
    use super::*;

    #[test]
    fn weapon_attachment_updates_leave_pawn_movement_and_view_unchanged() {
        let owner = crate::render_owner::render_test_support::owner("weapon-attachment");
        let access = owner.access();
        let pawn = crate::render_owner::render_test_support::construct(access, "Engine.Camera")
            .cast::<APawn>();
        let location = FVector { x: 80.0, y: 48.0, z: -80.0 };
        let view = generated::FRotator { pitch: 1024, yaw: 2048, roll: -512 };
        unsafe {
            (*pawn).base.location = location;
            (*pawn).view_rotation = view;
        }
        assert!(unsafe { (*pawn).weapon.is_null() });
        let host = CoreActorSpriteOps {
            access, render: ptr::null_mut(), frame: ptr::null_mut(), sprite: ptr::null_mut(),
            error: RefCell::new(None), poly_scope: RefCell::new(0), editor_hack: RefCell::new(0),
        };
        let attachment = FVector { x: 86.25, y: 256.5, z: -94.0 };
        let rotation = hp2_core::math::FRotator { pitch: -3072, yaw: 4096, roll: 768 };
        // Mesh continuation publishes attachment transforms before testing
        // whether this pawn currently has a drawable weapon.
        unsafe {
            host.write_rotator(pawn.cast(), ActorRotatorField::PawnWeaponRotation, rotation).unwrap();
            host.write_vector(pawn.cast(), ActorVectorField::PawnWeaponLocation, attachment).unwrap();
        }
        let moved = unsafe { (*pawn).base.location };
        let viewed = unsafe { (*pawn).view_rotation };
        assert_eq!(moved, location);
        assert_eq!((viewed.pitch, viewed.yaw, viewed.roll), (view.pitch, view.yaw, view.roll));
        let weapon_location = unsafe { (*pawn).weapon_loc };
        let weapon_rotation = unsafe { (*pawn).weapon_rot };
        assert_eq!(weapon_location, attachment);
        assert_eq!(
            (weapon_rotation.pitch, weapon_rotation.yaw, weapon_rotation.roll),
            (rotation.pitch, rotation.yaw, rotation.roll),
        );
        owner.shutdown().unwrap();
    }

    #[cfg(feature = "renderer-wgpu")]
    #[test]
    fn actor_sprite_hit_scopes_preserve_nested_viewport_ownership() {
        let owner = crate::render_owner::render_test_support::owner("actor-hit-scopes");
        let access = owner.access();
        let actor = crate::render_owner::render_test_support::construct(access, "Engine.Camera");
        let device = crate::render_owner::render_test_support::construct(access, "WgpuDrv.WgpuRenderDevice");
        let mut viewport: UViewport = unsafe { std::mem::zeroed() };
        let mut frame: render_scene::FSceneNode = unsafe { std::mem::zeroed() };
        let mut sprite: FDynamicSprite = unsafe { std::mem::zeroed() };
        frame.viewport = ptr::addr_of_mut!(viewport).cast();
        sprite.actor = actor.cast();
        let host = CoreActorSpriteOps {
            access, render: ptr::null_mut(), frame: ptr::addr_of_mut!(frame).cast(),
            sprite: ptr::addr_of_mut!(sprite), error: RefCell::new(None),
            poly_scope: RefCell::new(0), editor_hack: RefCell::new(0),
        };
        unsafe {
            // Normal drawing neither requires a hit device nor enters a global
            // renderer frame scope.
            host.push_actor_hit().unwrap();
            host.pop_actor_hit().unwrap();
            assert_eq!(viewport.hit_sizes.num, 0);

            viewport.ren_dev = device.cast();
            viewport.hit_testing = 1;
            host.push_actor_hit().unwrap();
            host.push_actor_hit().unwrap();
            assert_eq!(
                std::slice::from_raw_parts(viewport.hit_sizes.data.cast::<i32>(), viewport.hit_sizes.num as usize),
                &[24, 24],
            );
            host.pop_actor_hit().unwrap();
            assert_eq!(viewport.hit_sizes.num, 1);
            host.pop_actor_hit().unwrap();
            assert_eq!(viewport.hit_sizes.num, 0);
            assert!(crate::platform_owner::pop_hit(access, &mut viewport, false).is_err());
            assert_eq!(viewport.hit_sizes.num, 0);
        }
        owner.shutdown().unwrap();
    }
}
