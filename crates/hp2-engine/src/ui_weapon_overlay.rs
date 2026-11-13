//! Canonical first-person weapon overlays, including the explicitly authorized render-coupled flash clock.
use std::ptr;

use hp2_abi::{FVector, engine::generated::{AActor, FCheckResult, FRotator, FSceneNode, UCanvas, UClass, UFunction, UObject, UStructProperty, UViewport}};
use hp2_core::{ScriptPreCallDecision, core_access::{CoreAccess, RuntimeError, RuntimeResult}, name_owner::FindName};
use hp2_ui::{BlendMode, Rect};

use super::{CanvasTarget, f32_of, find_function, int_of, param_object, set_f32};
use super::player_health::{Picture, canvas_ready, color, paint_picture, signature, texture_size, virtual_function};
use crate::{stock_graphics_menu::{Weak, boolean, field, is_a, live, object, prop, read_boolean}, ui_vm};

/// Retail uses this reference width, not the current live MuzzleScale, on every qualifying draw.
const MUZZLE_REFERENCE_WIDTH: f32 = 640.;
/// Hand zero is centered with a small lateral bias rather than a zero lateral multiplier.
const CENTER_HAND: f32 = -0.20000000298023224_f32;
const ZERO: FVector = FVector { x: 0., y: 0., z: 0. };

#[derive(Clone, Copy)]
struct FlashGeometry {
    clip: (f32, f32),
    scale: f32,
    size: i32,
    default_offset_y: f32,
    lateral: f32,
    vertical: f32,
    center_vertical: f32,
}

impl FlashGeometry {
    fn position(self, hand: i32) -> (f32, f32) {
        let lateral_hand = if hand == 0 { CENTER_HAND } else { hand as f32 };
        let x = (self.clip.0 / 2. - (0.5 * self.scale) * self.size as f32)
            + self.clip.0 * ((lateral_hand * self.default_offset_y) * self.lateral);
        let vertical = if hand == 0 { self.vertical + self.center_vertical } else { self.vertical };
        let y = (self.clip.1 / 2. - (0.5 * self.scale) * self.size as f32)
            + self.clip.1 * vertical;
        (x, y)
    }
}

fn flash_picture(texture: i32, position: (f32, f32), extent: (f32, f32), scale: f32, color: [u8; 4]) -> Picture {
    Picture { texture, dest: Rect::new(position.0, position.1, extent.0 * scale, extent.1 * scale),
        source: Rect::new(0., 0., extent.0, extent.1), color, blend: BlendMode::Inherit }
}

fn expired(flash_time: f32, time: f32) -> bool { flash_time < time }
fn roll(default_roll: i32, hand: i32) -> i32 {
    if hand == 0 { (-2_i32).wrapping_mul(default_roll) } else { default_roll.wrapping_mul(hand) }
}

// Unlike an object-table object, class-default storage is a class-owned buffer.
unsafe fn defaults(a: CoreAccess<'_>, receiver: *mut UObject) -> RuntimeResult<*mut UObject> {
    // SAFETY: admission/revalidation established a live receiver and its Core-owned class.
    unsafe { a.default_object((*receiver).class.cast()) }
}

unsafe fn structure(a: CoreAccess<'_>, owner: *mut UObject, name: &str, path: &str) -> RuntimeResult<()> {
    // SAFETY: callers first validate the property's size and owner storage.
    unsafe {
        let property = a.find_field(owner.cast(), a.intern_str(name, FindName::Find)?, a.native_class("Core.StructProperty")?).cast::<UStructProperty>();
        if property.is_null() || !live(a, (*property).r#struct.cast())
            || !a.object_path_text((*property).r#struct.cast())?.into_iter().eq(path.chars().map(u32::from)) {
            return Err(RuntimeError::new("Weapon overlay struct layout unsupported"));
        }
        Ok(())
    }
}

unsafe fn vector(a: CoreAccess<'_>, receiver: *mut UObject, name: &str) -> RuntimeResult<FVector> {
    // SAFETY: reflected storage is checked before the unaligned copy.
    unsafe { Ok(prop(a, receiver, name, "Core.StructProperty", size_of::<FVector>())?.cast::<FVector>().read_unaligned()) }
}
unsafe fn rotation(a: CoreAccess<'_>, receiver: *mut UObject) -> RuntimeResult<FRotator> {
    // SAFETY: reflected storage is checked before the unaligned copy.
    unsafe { Ok(prop(a, receiver, "ViewRotation", "Core.StructProperty", size_of::<FRotator>())?.cast::<FRotator>().read_unaligned()) }
}
unsafe fn byte(a: CoreAccess<'_>, receiver: *mut UObject, name: &str) -> RuntimeResult<u8> {
    // SAFETY: reflected storage is checked before reading the byte.
    unsafe { Ok(prop(a, receiver, name, "Core.ByteProperty", 1)?.read()) }
}

unsafe fn need_actor(a: CoreAccess<'_>, receiver: *mut UObject, class: *mut UClass) -> RuntimeResult<()> {
    // SAFETY: is_a checks object-table liveness before dereferencing the receiver.
    unsafe {
        if !is_a(a, receiver, class) || read_boolean(a, receiver, "bDeleteMe")? {
            return Err(RuntimeError::new("Weapon overlay actor unavailable"));
        }
        if (*(*receiver).class.cast::<UClass>()).properties_size < size_of::<AActor>() as i32 {
            return Err(RuntimeError::new("Weapon overlay native actor layout unsupported"));
        }
        Ok(())
    }
}

unsafe fn movement_level(a: CoreAccess<'_>, receiver: *mut UObject) -> RuntimeResult<*mut hp2_abi::engine::generated::ULevel> {
    // SAFETY: need_actor established the canonical native header before this access.
    unsafe {
        let level = ptr::addr_of!((*receiver.cast::<AActor>()).x_level).read_unaligned().cast();
        if !is_a(a, level, a.native_class("Engine.Level")?) {
            return Err(RuntimeError::new("Weapon overlay movement level unavailable"));
        }
        Ok(level.cast())
    }
}

unsafe fn canvas_graph(a: CoreAccess<'_>, canvas: *mut UObject) -> RuntimeResult<()> {
    // SAFETY: Canvas type, allocation size and graph objects precede native graph accesses.
    unsafe {
        if !canvas_ready(a, canvas)? || (*(*canvas).class.cast::<UClass>()).properties_size < size_of::<UCanvas>() as i32 {
            return Err(RuntimeError::new("Weapon overlay Canvas unsupported"));
        }
        let c = canvas.cast::<UCanvas>();
        let viewport = ptr::addr_of!((*c).viewport).read_unaligned().cast::<UViewport>();
        let scene = ptr::addr_of!((*c).frame).read_unaligned().cast::<FSceneNode>();
        if scene.is_null() || !live(a, ptr::addr_of!((*c).render).read_unaligned().cast())
            || !is_a(a, viewport.cast(), a.native_class("Engine.Viewport")?) {
            return Err(RuntimeError::new("Weapon overlay render graph unavailable"));
        }
        if !is_a(a, ptr::addr_of!((*viewport).actor).read_unaligned().cast(), a.native_class("Engine.PlayerPawn")?)
            || !live(a, ptr::addr_of!((*viewport).ren_dev).read_unaligned().cast()) {
            return Err(RuntimeError::new("Weapon overlay viewport unavailable"));
        }
        Ok(())
    }
}

unsafe fn declared(a: CoreAccess<'_>, owner: &str, name: &str) -> RuntimeResult<*mut UFunction> {
    // SAFETY: only live canonical metadata is returned; inherited lookups cannot impersonate a declaration.
    unsafe {
        let function = find_function(a, owner, "Core.Class", name)?;
        if !live(a, function.cast())
            || !a.object_path_text(function.cast())?.into_iter().eq(owner.chars().chain(['.']).chain(name.chars()).map(u32::from)) {
            return Err(RuntimeError::new("Weapon overlay declared function unavailable"));
        }
        Ok(function)
    }
}

unsafe fn native_ready(a: CoreAccess<'_>, owner: &str, name: &str, index: u16) -> RuntimeResult<*mut UFunction> {
    // SAFETY: declared validates function metadata; the catalog query executes no native or script.
    unsafe {
        let function = declared(a, owner, name)?;
        let descriptor = crate::engine_vm_native::ENGINE_VM_NATIVES.iter()
            .find(|native| native.class_name == owner && native.function_name == name && native.native_index == Some(index));
        if (*function).i_native != index || (*function).function_flags & 0x400 == 0
            || !descriptor.is_some_and(|native| a.effective_external_vm_native_is(index, native)) {
            return Err(RuntimeError::new("Weapon overlay canonical native unavailable"));
        }
        Ok(function)
    }
}

unsafe fn leaves_ready(a: CoreAccess<'_>) -> RuntimeResult<*mut UFunction> {
    // SAFETY: every signature is checked before the adapter performs its first mutation.
    unsafe {
        let calc = declared(a, "Engine.Inventory", "CalcDrawOffset")?;
        let bytes = usize::from((*calc).parms_size);
        let result = field(a, calc.cast(), "ReturnValue", "Core.StructProperty", size_of::<FVector>(), bytes, true)?;
        structure(a, calc.cast(), "ReturnValue", "Core.Object.Vector")?;
        if (*calc).i_native != 0 || (*calc).function_flags & 0x401 != 1 || (*calc).num_parms != 1
            || usize::from((*calc).return_value_offset) != result {
            return Err(RuntimeError::new("Weapon overlay CalcDrawOffset is not the declared script callback"));
        }
        let set_pos = declared(a, "Engine.Canvas", "SetPos")?;
        let icon = declared(a, "Engine.Canvas", "DrawIcon")?;
        if (*set_pos).i_native != 0 || (*icon).i_native != 0
            || (*set_pos).function_flags & 0x401 != 1 || (*icon).function_flags & 0x401 != 1
            || !signature(a, set_pos, &[("X", "Core.FloatProperty", 4), ("Y", "Core.FloatProperty", 4)], None)?
            || !signature(a, icon, &[("Tex", "Core.ObjectProperty", size_of::<*mut UObject>()), ("Scale", "Core.FloatProperty", 4)], None)? {
            return Err(RuntimeError::new("Weapon overlay canonical image leaves unsupported"));
        }
        for (name, index, parameter, kind) in [
            ("SetLocation", 267, "NewLocation", "Core.Object.Vector"),
            ("SetRotation", 299, "NewRotation", "Core.Object.Rotator"),
        ] {
            let function = native_ready(a, "Engine.Actor", name, index)?;
            if !signature(a, function, &[(parameter, "Core.StructProperty", 12)], Some(("Core.BoolProperty", 4)))? {
                return Err(RuntimeError::new("Weapon overlay movement parameters unsupported"));
            }
            structure(a, function.cast(), parameter, kind)?;
        }
        let actor = native_ready(a, "Engine.Canvas", "DrawActor", 467)?;
        if !signature(a, actor, &[("A", "Core.ObjectProperty", size_of::<*mut UObject>()), ("WireFrame", "Core.BoolProperty", 4), ("ClearZ", "Core.BoolProperty", 4)], None)? {
            return Err(RuntimeError::new("Weapon overlay DrawActor parameters unsupported"));
        }
        native_ready(a, "Engine.Canvas", "DrawTile", 466)?;
        Ok(calc)
    }
}

struct Render {
    receiver: Weak,
    canvas: Weak,
    calc: Weak,
    weapon_class: *mut UClass,
    pawn_class: *mut UClass,
    player_class: *mut UClass,
}
enum Admission { Return, Hide, Render(Render) }

unsafe fn level_info(a: CoreAccess<'_>, receiver: *mut UObject) -> RuntimeResult<*mut UObject> {
    // SAFETY: callers establish a live canonical actor; null contexts are left stock at admission.
    unsafe {
        let level = object(a, receiver, "Level")?;
        if !is_a(a, level, a.native_class("Engine.LevelInfo")?) {
            return Err(RuntimeError::new("Weapon overlay level info unavailable"));
        }
        Ok(level)
    }
}

unsafe fn preflight(a: CoreAccess<'_>, receiver: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<Admission> {
    // SAFETY: all work here is admission-only: no callback, presentation write or drawing.
    unsafe {
        let weapon_class = a.native_class("Engine.Weapon")?;
        need_actor(a, receiver, weapon_class)?;
        if virtual_function(a, receiver, "RenderOverlays")? != function
            || !signature(a, function, &[("Canvas", "Core.ObjectProperty", size_of::<*mut UObject>())], None)? {
            return Err(RuntimeError::new("Weapon overlay custom route or parameters unsupported"));
        }
        if read_boolean(a, receiver, "bHideWeapon")? { return Ok(Admission::Return); }
        let owner = object(a, receiver, "Owner")?;
        if owner.is_null() { return Ok(Admission::Return); }
        let pawn_class = a.native_class("Engine.Pawn")?;
        // A non-Pawn owner has observable null-cast warning paths; run those in stock, not an approximation.
        need_actor(a, owner, pawn_class)?;
        let player_class = a.native_class("Engine.PlayerPawn")?;
        if is_a(a, owner, player_class) {
            if f32_of(a, owner, "DesiredFOV")? != f32_of(a, owner, "DefaultFOV")? { return Ok(Admission::Return); }
            let hand = f32_of(a, owner, "Handedness")? as i32;
            if byte(a, level_info(a, receiver)?, "NetMode")? == 3 && hand == 2 { return Ok(Admission::Hide); }
            object(a, owner, "Player")?;
        }
        prop(a, owner, "WalkBob", "Core.StructProperty", size_of::<FVector>())?;
        prop(a, owner, "Location", "Core.StructProperty", size_of::<FVector>())?;
        prop(a, owner, "ViewRotation", "Core.StructProperty", size_of::<FRotator>())?;
        for (name, kind) in [("WalkBob", "Core.Object.Vector"), ("Location", "Core.Object.Vector"), ("ViewRotation", "Core.Object.Rotator")] {
            structure(a, (*owner).class.cast(), name, kind)?;
        }
        for name in ["bSetFlashTime", "bDrawMuzzleFlash"] { read_boolean(a, receiver, name)?; }
        for name in ["MuzzleScale", "FlashTime", "FlashLength", "FlashO", "FlashY", "FlashC"] { f32_of(a, receiver, name)?; }
        int_of(a, receiver, "FlashS")?;
        byte(a, receiver, "bMuzzleFlash")?;
        let level = level_info(a, receiver)?;
        read_boolean(a, level, "bHighDetailMode")?;
        f32_of(a, level, "TimeSeconds")?;
        let default = defaults(a, receiver)?;
        f32_of(a, default, "MuzzleScale")?;
        vector(a, default, "FireOffset")?;
        prop(a, default, "Rotation", "Core.StructProperty", size_of::<FRotator>())?;
        structure(a, (*receiver).class.cast(), "FireOffset", "Core.Object.Vector")?;
        structure(a, (*receiver).class.cast(), "Rotation", "Core.Object.Rotator")?;
        let texture = object(a, receiver, "MFTexture")?;
        if !texture.is_null() { texture_size(a, texture)?; }
        let canvas = param_object(a, function, params, size, "Canvas").ok_or_else(|| RuntimeError::new("Weapon overlay Canvas parameter unavailable"))?;
        canvas_graph(a, canvas)?;
        movement_level(a, receiver)?;
        let calc = leaves_ready(a)?;
        Ok(Admission::Render(Render { receiver: Weak::new(receiver), canvas: Weak::new(canvas), calc: Weak::new(calc.cast()), weapon_class, pawn_class, player_class }))
    }
}

impl Render {
    unsafe fn revalidate(&self, a: CoreAccess<'_>) -> RuntimeResult<()> {
        // SAFETY: validate the receiver at its next use, not resources needed only later.
        unsafe {
            if !self.receiver.live(a) {
                return Err(RuntimeError::new("Weapon overlay receiver invalidated during callback"));
            }
            need_actor(a, self.receiver.p, self.weapon_class)?;
            Ok(())
        }
    }
}

unsafe fn draw(a: CoreAccess<'_>, prepared: Render) -> RuntimeResult<()> {
    // SAFETY: admission established the complete route. After the first write, every failure
    // propagates; native movement/rendering retain their own partial effects without stock replay.
    unsafe {
        let receiver = prepared.receiver.p;
        let canvas = prepared.canvas.p;
        let owner = object(a, receiver, "Owner")?;
        let player_owner = is_a(a, owner, prepared.player_class);
        let hand = if player_owner { f32_of(a, owner, "Handedness")? as i32 } else { 0 };
        if !player_owner || object(a, owner, "Player")?.is_null() {
            prop(a, owner, "WalkBob", "Core.StructProperty", size_of::<FVector>())?.cast::<FVector>().write_unaligned(ZERO);
        }
        // Core natives 130/132 are short-circuit AND/OR. BOOL_VARIABLE consumes its following
        // property expression; it is not an independent operand as the offline AST prints it.
        if byte(a, receiver, "bMuzzleFlash")? > 0
            && read_boolean(a, receiver, "bDrawMuzzleFlash")?
            && read_boolean(a, level_info(a, receiver)?, "bHighDetailMode")?
            && !object(a, receiver, "MFTexture")?.is_null() {
            let scale = (f32_of(a, defaults(a, receiver)?, "MuzzleScale")? * f32_of(a, canvas, "ClipX")?) / MUZZLE_REFERENCE_WIDTH;
            set_f32(a, receiver, "MuzzleScale", scale)?;
            if !read_boolean(a, receiver, "bSetFlashTime")? {
                boolean(a, receiver, "bSetFlashTime", true)?;
                let time = f32_of(a, level_info(a, receiver)?, "TimeSeconds")?;
                set_f32(a, receiver, "FlashTime", time + f32_of(a, receiver, "FlashLength")?)?;
            } else if expired(f32_of(a, receiver, "FlashTime")?, f32_of(a, level_info(a, receiver)?, "TimeSeconds")?) {
                prop(a, receiver, "bMuzzleFlash", "Core.ByteProperty", 1)?.write(0);
            }
            if byte(a, receiver, "bMuzzleFlash")? > 0 {
                let geometry = FlashGeometry {
                    clip: (f32_of(a, canvas, "ClipX")?, f32_of(a, canvas, "ClipY")?),
                    scale: f32_of(a, receiver, "MuzzleScale")?, size: int_of(a, receiver, "FlashS")?,
                    default_offset_y: vector(a, defaults(a, receiver)?, "FireOffset")?.y,
                    lateral: f32_of(a, receiver, "FlashO")?, vertical: f32_of(a, receiver, "FlashY")?,
                    center_vertical: if hand == 0 { f32_of(a, receiver, "FlashC")? } else { 0. },
                };
                let (x, y) = geometry.position(hand);
                let target = CanvasTarget { canvas, tile_style: None, fonts: [ptr::null_mut(); 4] };
                target.set_pos(a, x, y)?;
                target.set_style(a, 3)?;
                let texture = object(a, receiver, "MFTexture")?;
                let (w, h) = texture_size(a, texture)?;
                let scale = f32_of(a, receiver, "MuzzleScale")?;
                // The shared retained image view emits one unclipped signed region. Its target
                // calls native_draw_tile, so SpaceX/CurYL/origin/Z/smoothing remain native-owned.
                paint_picture(a, receiver, canvas, flash_picture((*texture).index, (x, y), (w, h), scale, color(a, canvas)?))?;
                if !prepared.canvas.live(a) { return Err(RuntimeError::new("Weapon overlay Canvas invalidated during drawing")); }
                target.set_style(a, 1)?;
            }
        } else {
            boolean(a, receiver, "bSetFlashTime", false)?;
        }
        prepared.revalidate(a)?;
        // Evaluate Owner.Location before CalcDrawOffset: the callback may replace/move Owner.
        let owner = object(a, receiver, "Owner")?;
        need_actor(a, owner, prepared.pawn_class)?;
        let location = vector(a, owner, "Location")?;
        if !prepared.calc.live(a) { return Err(RuntimeError::new("Weapon overlay callback unavailable")); }
        let mut offset = ZERO;
        ui_vm::invoke_resolved(a, receiver, prepared.calc.p.cast(), &[], &[], (&raw mut offset).cast())?;
        prepared.revalidate(a)?;
        let destination = FVector { x: location.x + offset.x, y: location.y + offset.y, z: location.z + offset.z };
        crate::level_move_owner::far_move_actor(a, movement_level(a, receiver)?, receiver.cast(), destination, false, false)?;
        prepared.revalidate(a)?;
        // SetLocation can run touch/movement callbacks; read the current Owner only now.
        let owner = object(a, receiver, "Owner")?;
        need_actor(a, owner, prepared.pawn_class)?;
        let mut new_rotation = rotation(a, owner)?;
        let default_rotation = prop(a, defaults(a, receiver)?, "Rotation", "Core.StructProperty", size_of::<FRotator>())?.cast::<FRotator>().read_unaligned();
        new_rotation.roll = roll(default_rotation.roll, hand);
        let mut hit: FCheckResult = std::mem::zeroed();
        crate::level_move_owner::move_actor(a, movement_level(a, receiver)?, receiver.cast(), ZERO, new_rotation, &mut hit, false, false, false, false)?;
        prepared.revalidate(a)?;
        if !prepared.canvas.live(a) { return Err(RuntimeError::new("Weapon overlay Canvas invalidated during movement")); }
        canvas_graph(a, canvas)?;
        crate::canvas_owner::draw_actor(a, canvas.cast(), receiver.cast(), false, false)
    }
}

unsafe fn callback(a: CoreAccess<'_>, receiver: *mut UObject, function: *mut UFunction, params: *const u8, size: usize) -> RuntimeResult<ScriptPreCallDecision> {
    if !super::enabled() || !live(a, receiver) { return Ok(ScriptPreCallDecision::Continue); }
    // SAFETY: preflight is effect-free; only the accepted branch below may mutate or draw.
    let prepared = match unsafe { preflight(a, receiver, function, params, size) } {
        Ok(prepared) => prepared,
        Err(_) => return Ok(ScriptPreCallDecision::Continue),
    };
    // SAFETY: all reflected layouts and required callable/native capabilities were admitted.
    unsafe {
        match prepared {
            Admission::Return => {},
            Admission::Hide => boolean(a, receiver, "bHideWeapon", true)?,
            Admission::Render(prepared) => draw(a, prepared)?,
        }
    }
    Ok(ScriptPreCallDecision::Consume)
}

/// Installs only the canonical Weapon leaf at its existing first-person overlay event boundary.
/// # Safety
/// Core metadata and the returned observer guard must outlive all overlay dispatch.
pub(super) unsafe fn install(a: CoreAccess<'_>) -> RuntimeResult<Option<impl Drop + '_>> {
    if !super::enabled() { return Ok(None); }
    // SAFETY: registration does not execute the function; ui_hud retains the returned guard.
    unsafe {
        let function = find_function(a, "Engine.Weapon", "Core.Class", "RenderOverlays")?;
        if function.is_null() { return Ok(None); }
        Ok(Some(a.install_script_pre_observer("ui-weapon-overlay", &[function], callback)?))
    }
}

#[cfg(test)]
pub(super) fn assert_edges() {
    let geometry = FlashGeometry { clip: (800., 600.), scale: 5., size: 16,
        default_offset_y: -6., lateral: 0.01, vertical: 0.1, center_vertical: 0.05 };
    assert_eq!(geometry.position(0), (369.6, 350.), "Centered hand retains both retail biases");
    assert_eq!(geometry.position(-1), (408., 320.), "Other hands omit FlashC");
    let signed = FlashGeometry { scale: -2., ..geometry };
    assert_eq!(signed.position(2), (320., 376.), "Signed scale is not clamped");
    let picture = flash_picture(17, signed.position(2), (31., 47.), signed.scale, [17, 33, 65, 73]);
    assert_eq!(picture.dest, Rect::new(320., 376., -62., -94.), "FlashS centers but does not size the image");
    assert_eq!(picture.source, Rect::new(0., 0., 31., 47.), "DrawIcon uses the complete MFTexture region");
    assert!(!expired(1., 1.), "Equality does not expire the flash");
    assert!(expired(1., f32::from_bits(1_f32.to_bits() + 1)), "The next representable time expires it");
    assert!(!expired(f32::NAN, 1.) && !expired(1., f32::NAN), "Unordered clocks do not expire");
    assert_eq!(roll(i32::MAX, 2), -2, "Native integer multiplication wraps");
    assert_eq!(roll(i32::MIN, 0), 0, "Centered roll also uses wrapping multiplication");
}
