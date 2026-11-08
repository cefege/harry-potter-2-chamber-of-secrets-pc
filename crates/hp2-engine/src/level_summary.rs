//! Rust-owned `ULevelSummary` behavior and the legacy native caller adapter.
//!
//! The reflected subclass, storage, properties and class operations live in
//! Core's canonical graph; this module supplies its Engine-specific behavior.


use hp2_abi::engine::generated::{TArrayRaw, UObject, ULevelSummary};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::core_class_specs::{NativeClassSpec, NativeRecordKind, StartupProperty, StartupPropertyKind};
use hp2_core::object_owner::ClassOps;
use std::alloc::Layout;
use std::ffi::c_void;
use std::ptr;

pub const TITLE: u32 = 0;
pub const IDEAL_PLAYER_COUNT: u32 = 1;

const EDIT: u32 = 0x0000_0001;
const LOCALIZED: u32 = 0x0000_8000;
const PROPERTY_OBJECT_FLAGS: u32 = 4;
const CLASS_OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;

const LOCALIZATION_SECTION: &[u32] = &[76, 101, 118, 101, 108, 73, 110, 102, 111, 48];
const TITLE_KEY: &[u32] = &[84, 105, 116, 108, 101];
const IDEAL_PLAYER_COUNT_KEY: &[u32] = &[73, 100, 101, 97, 108, 80, 108, 97, 121, 101, 114, 67, 111, 117, 110, 116];
const PROPERTIES: &[StartupProperty] = &[
    StartupProperty { name: "Title", kind: StartupPropertyKind::String, category: "", property_flags: EDIT | LOCALIZED, object_flags: PROPERTY_OBJECT_FLAGS, inner_name: None, offset: std::mem::offset_of!(ULevelSummary, title), array_dim: 1 },
    StartupProperty { name: "Author", kind: StartupPropertyKind::String, category: "", property_flags: EDIT, object_flags: PROPERTY_OBJECT_FLAGS, inner_name: None, offset: std::mem::offset_of!(ULevelSummary, author), array_dim: 1 },
    StartupProperty { name: "IdealPlayerCount", kind: StartupPropertyKind::String, category: "", property_flags: EDIT | LOCALIZED, object_flags: PROPERTY_OBJECT_FLAGS, inner_name: None, offset: std::mem::offset_of!(ULevelSummary, ideal_player_count), array_dim: 1 },
    StartupProperty { name: "RecommendedEnemies", kind: StartupPropertyKind::Int, category: "", property_flags: EDIT, object_flags: PROPERTY_OBJECT_FLAGS, inner_name: None, offset: std::mem::offset_of!(ULevelSummary, recommended_enemies), array_dim: 1 },
    StartupProperty { name: "RecommendedTeammates", kind: StartupPropertyKind::Int, category: "", property_flags: EDIT, object_flags: PROPERTY_OBJECT_FLAGS, inner_name: None, offset: std::mem::offset_of!(ULevelSummary, recommended_teammates), array_dim: 1 },
    StartupProperty { name: "LevelEnterText", kind: StartupPropertyKind::String, category: "", property_flags: EDIT | LOCALIZED, object_flags: PROPERTY_OBJECT_FLAGS, inner_name: None, offset: std::mem::offset_of!(ULevelSummary, level_enter_text), array_dim: 1 },
];

const fn level_summary_operations() -> ClassOps {
    let mut operations = NativeRecordKind::Object.operations();
    operations.post_load = rust_post_load;
    operations
}

const OPERATIONS: ClassOps = level_summary_operations();

/// Canonical Engine declaration consumed by the narrow LevelSummary owner.
pub static NATIVE_CLASSES: &[NativeClassSpec] = &[NativeClassSpec {
    short_name: "LevelSummary",
    package: "Engine",
    super_name: Some("Object"),
    within: "Object",
    flags: 0,
    object_flags: CLASS_OBJECT_FLAGS,
    config: "System",
    guid: [0; 4],
    kind: NativeRecordKind::Object,
    layout: Some(Layout::new::<ULevelSummary>()),
    operations: Some(OPERATIONS),
    properties: PROPERTIES,
    defaults: &[],
}];


fn policy<E, T>(
    mut superclass: impl FnMut() -> Result<(), E>,
    mut localize: impl FnMut(u32) -> Result<Option<T>, E>,
    mut assign: impl FnMut(u32, T) -> Result<(), E>,
) -> Result<(), E> {
    superclass()?;
    for field in [TITLE, IDEAL_PLAYER_COUNT] {
        if let Some(value) = localize(field)? {
            assign(field, value)?;
        }
    }
    Ok(())
}

/// # Safety
/// `object` must be the live, canonical `ULevelSummary` selected by Core dispatch.
unsafe fn rust_post_load(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    let superclass = NativeRecordKind::Object.operations().post_load;
    let summary = object.cast::<ULevelSummary>();
    let mut package_name = None;
    policy(
        || unsafe { superclass(access, object) },
        |field| {
            if package_name.is_none() {
                // SAFETY: Core dispatch selected a live object with a canonical outer.
                let outer = unsafe { (*object).outer.cast::<UObject>() };
                if outer.is_null() {
                    return Err(RuntimeError::new("LevelSummary has no package outer"));
                }
                package_name = Some(access.name_text(i32::from_ne_bytes(unsafe { (*outer).name }))?);
            }
            let package_name = package_name.as_deref()
                .ok_or_else(|| RuntimeError::new("LevelSummary package name is unavailable"))?;
            let key = match field {
                TITLE => TITLE_KEY,
                IDEAL_PLAYER_COUNT => IDEAL_PLAYER_COUNT_KEY,
                _ => return Err(RuntimeError::new("Unknown LevelSummary localization field")),
            };
            let text = access.localize_text(
                LOCALIZATION_SECTION, key, Some(package_name), None, true,
            )?;
            Ok((!text.is_empty()).then_some(text))
        },
        |field, mut text| {
            text.push(0);
            let value = match field {
                TITLE => unsafe { ptr::addr_of_mut!((*summary).title) },
                IDEAL_PLAYER_COUNT => unsafe { ptr::addr_of_mut!((*summary).ideal_player_count) },
                _ => return Err(RuntimeError::new("Unknown LevelSummary localization field")),
            };
            // SAFETY: the selected reflected FString is initialized and
            // exclusively borrowed for this synchronous assignment.
            unsafe { access.assign_fstring(value.cast::<TArrayRaw>(), &text) }
        },
    )
}

#[repr(C)]
pub struct Hp2LevelSummaryServices {
    pub super_post_load: unsafe extern "C" fn(*mut c_void) -> i32,
    pub localize: unsafe extern "C" fn(*mut c_void, u32, *mut *const u32) -> i32,
    pub assign: unsafe extern "C" fn(*mut c_void, u32, *const u32) -> i32,
}

/// Preserves superclass-first localization and assignment order for native callers.
///
/// # Safety
/// Services and the summary pointer must remain valid for all synchronous callbacks.
pub unsafe fn post_load(svc: &Hp2LevelSummaryServices, summary: *mut c_void) -> Result<(), i32> {
    policy(
        || {
            let status = unsafe { (svc.super_post_load)(summary) };
            if status == 0 { Ok(()) } else { Err(status) }
        },
        |field| {
            let mut text = ptr::null();
            let status = unsafe { (svc.localize)(summary, field, &mut text) };
            if status != 0 {
                return Err(status);
            }
            Ok(if text.is_null() || unsafe { *text } == 0 { None } else { Some(text) })
        },
        |field, text| {
            let status = unsafe { (svc.assign)(summary, field, text) };
            if status == 0 { Ok(()) } else { Err(status) }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::c_void;

    struct State {
        log: Vec<&'static str>,
        text: [u32; 2],
        fail: i32,
        super_status: i32,
    }
    unsafe extern "C" fn sup(ctx: *mut c_void) -> i32 {
        unsafe {
            let s = &mut *ctx.cast::<State>();
            s.log.push("super");
            s.super_status
        }
    }
    unsafe extern "C" fn loc(ctx: *mut c_void, field: u32, out: *mut *const u32) -> i32 {
        unsafe {
            let s = &mut *ctx.cast::<State>();
            s.log.push(if field == TITLE { "title" } else { "ideal" });
            if s.fail == field as i32 + 1 {
                return 9;
            };
            out.write((&s.text[field as usize]) as *const u32);
        }
        0
    }
    unsafe extern "C" fn assign(ctx: *mut c_void, field: u32, _: *const u32) -> i32 {
        unsafe {
            (&mut *ctx.cast::<State>()).log.push(if field == TITLE {
                "assign-title"
            } else {
                "assign-ideal"
            })
        };
        0
    }
    fn svc() -> Hp2LevelSummaryServices {
        Hp2LevelSummaryServices {
            super_post_load: sup,
            localize: loc,
            assign,
        }
    }

    #[test]
    fn preserves_order_and_skips_empty_localization() {
        let mut s = State {
            log: vec![],
            text: [b'T' as u32, 0],
            fail: 0,
            super_status: 0,
        };
        unsafe { post_load(&svc(), (&mut s as *mut State).cast()).unwrap() };
        assert_eq!(s.log, ["super", "title", "assign-title", "ideal"]);
    }
    #[test]
    fn stops_after_a_localization_failure() {
        let mut s = State {
            log: vec![],
            text: [b'T' as u32, b'I' as u32],
            fail: 2,
            super_status: 0,
        };
        assert_eq!(
            unsafe { post_load(&svc(), (&mut s as *mut State).cast()) },
            Err(9)
        );
        assert_eq!(s.log, ["super", "title", "assign-title", "ideal"]);
    }

    #[test]
    fn stops_before_localization_when_super_post_load_fails() {
        let mut s = State {
            log: vec![],
            text: [b'T' as u32, b'I' as u32],
            fail: 0,
            super_status: 7,
        };
        assert_eq!(
            unsafe { post_load(&svc(), (&mut s as *mut State).cast()) },
            Err(7)
        );
        assert_eq!(s.log, ["super"]);
    }
}
