//! Canonical ownership of the complete shared `ULevel::Listen` policy.

use crate::level_net::{self, ListenOps};
use hp2_abi::{FVector, engine::generated::{
    FName, FRotator, TArrayRaw, UClass, UField, UGameEngine, ULevel,
    UNetDriver, UObject, UProperty, UStruct,
}};
use hp2_core::{core_access::{CoreAccess, RuntimeError, RuntimeResult}, lit, name_owner::FindName};
use std::{ffi::{c_void, CStr}, ptr};

/// # Safety
/// Level and the source error text belong to this live canonical LoadMap call.
pub(crate) unsafe fn listen(
    access: CoreAccess<'_>, level: *mut ULevel, error_text: &mut Vec<u32>,
) -> RuntimeResult<bool> {
    if level.is_null() { return Err(RuntimeError::new("Null canonical Listen level")); }
    let mut error = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
    if !error_text.is_empty() {
        if error_text.last() != Some(&0) { error_text.push(0); }
        unsafe { access.assign_fstring(&mut error, error_text)?; }
    }
    let mut ops = CoreListenOps { access, property_name: Vec::new() };
    let outcome = unsafe { level_net::listen_with(&mut ops, level.cast(), (&raw mut error).cast()) };
    // Mirror committed source FString mutations even if a later operation failed.
    error_text.clear();
    if error.num > 0 {
        error_text.extend_from_slice(unsafe {
            std::slice::from_raw_parts(error.data.cast::<u32>(), error.num as usize)
        });
    }
    let cleanup = unsafe { access.destroy_owned_fstring(&mut error) };
    match (outcome, cleanup) {
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        (Ok(value), Ok(())) => Ok(value != 0),
    }
}

struct CoreListenOps<'a> {
    access: CoreAccess<'a>,
    property_name: Vec<u32>,
}

struct CoreProperties {
    class: *mut UStruct,
    field: *mut UField,
    property_class: *mut UClass,
}
impl CoreProperties {
    unsafe fn seek(&mut self) {
        unsafe {
            loop {
                while self.field.is_null() {
                    if self.class.is_null() { return; }
                    self.class = (*self.class.cast::<UField>()).super_field.cast();
                    if self.class.is_null() { return; }
                    self.field = (*self.class).children.cast();
                }
                if hp2_core::object_model::is_a(self.field.cast(), self.property_class.cast()) { return; }
                self.field = (*self.field).next.cast();
            }
        }
    }
}

/// The caller owns a live terminated source string throughout this synchronous borrow.
unsafe fn wide<'a>(text: *const u32) -> &'a [u32] {
    let mut length = 0;
    while unsafe { text.add(length).read() } != 0 { length += 1; }
    unsafe { std::slice::from_raw_parts(text, length + 1) }
}

impl ListenOps for CoreListenOps<'_> {
    type Error = RuntimeError;
    type Properties = CoreProperties;

    unsafe fn localize_error(&mut self, _level: *mut c_void, error: *mut c_void, key: &'static CStr) -> RuntimeResult<()> {
        // The shared policy has only NetAlready (10 bytes) and NetListen keys.
        let mut key_text = [0u32; 11];
        for (out, byte) in key_text.iter_mut().zip(key.to_bytes()) { *out = u32::from(*byte); }
        let mut text = self.access.localize_text(lit!("Errors"), &key_text,
            Some(lit!("Engine")), None, false)?;
        text.push(0);
        unsafe { self.access.assign_fstring(error.cast(), &text) }
    }
    unsafe fn level_linker(&mut self, level: *mut c_void) -> RuntimeResult<*mut c_void> {
        Ok(unsafe { (*level.cast::<UObject>())._linker })
    }
    unsafe fn load_class(&mut self, actor: bool, name: *const u32) -> RuntimeResult<*mut c_void> {
        let base = self.access.native_class(if actor { "Engine.Actor" } else { "Engine.NetDriver" })?;
        unsafe { self.access.static_load_class(base, ptr::null_mut(), wide(name), 1).map(|class| class.cast()) }
    }
    unsafe fn construct_driver(&mut self, class: *mut c_void) -> RuntimeResult<*mut c_void> {
        unsafe {
            self.access.construct_object(class.cast(), self.access.transient_package(),
                0, 0, ptr::null_mut()).map(|driver| driver.cast())
        }
    }
    unsafe fn init_driver(&mut self, driver: *mut c_void, level: *mut c_void, url: *mut c_void, error: *mut c_void) -> RuntimeResult<bool> {
        // Canonical Notify is the real level receiver identity, never a C++ vptr.
        unsafe { crate::network_owner::init_listen(self.access, driver.cast(), level, url.cast(), error.cast()) }
    }
    unsafe fn delete_driver(&mut self, driver: *mut c_void) -> RuntimeResult<()> {
        unsafe { self.access.delete_object(driver.cast()) }
    }
    unsafe fn error_text(&mut self, error: *mut c_void) -> RuntimeResult<*const u32> {
        Ok(unsafe { hp2_core::native_text::fstring_text(error.cast()) })
    }
    unsafe fn log(&mut self, text: *const u32) -> RuntimeResult<()> {
        let category = self.access.intern_str("Log", FindName::Find)?;
        self.access.emit_log(category, unsafe { wide(text) })
    }
    unsafe fn game_engine(&mut self, engine: *mut c_void) -> RuntimeResult<*mut c_void> {
        let class = self.access.native_class("Engine.GameEngine")?;
        Ok(if unsafe { hp2_core::object_model::is_a(engine, class.cast()) } { engine } else { ptr::null_mut() })
    }
    unsafe fn build_master_map(&mut self, engine: *mut c_void, driver: *mut c_void, level: *mut c_void) -> RuntimeResult<()> {
        unsafe { crate::package_map_level_owner::build_server_master_map(self.access, engine.cast(), driver.cast(), level.cast()) }
    }
    unsafe fn server_actor_count(&mut self, engine: *mut c_void) -> RuntimeResult<i32> {
        Ok(unsafe { (*engine.cast::<UGameEngine>()).server_actors.num })
    }
    unsafe fn server_actor_text(&mut self, engine: *mut c_void, index: i32) -> RuntimeResult<*const u32> {
        let actors = unsafe { (*engine.cast::<UGameEngine>()).server_actors };
        if index < 0 || index >= actors.num {
            return Err(RuntimeError::new("GameEngine.ServerActors index is out of range"));
        }
        Ok(unsafe { hp2_core::native_text::fstring_text(actors.data.cast::<TArrayRaw>().add(index as usize)) })
    }
    unsafe fn parse_token(&mut self, stream: &mut *const u32, buffer: &mut [u32; 240]) -> RuntimeResult<bool> {
        let mut cursor = (*stream).cast::<hp2_core::ansi::Tchar>();
        let found = unsafe { hp2_core::tokenize::parse_token(&mut cursor, buffer.as_mut_ptr().cast(), 240, true) };
        *stream = cursor.cast();
        Ok(found)
    }
    unsafe fn spawn_actor(&mut self, level: *mut c_void, class: *mut c_void) -> RuntimeResult<*mut c_void> {
        unsafe {
            crate::level_spawn_actor_owner::spawn_actor(self.access, level.cast(), class.cast(),
                FName { index: 0 }, ptr::null_mut(), ptr::null_mut(),
                FVector { x: 0.0, y: 0.0, z: 0.0 }, FRotator { pitch: 0, yaw: 0, roll: 0 },
                ptr::null_mut(), false, false).map(|actor| actor.cast())
        }
    }
    unsafe fn find_equals(&mut self, text: *const u32) -> RuntimeResult<*const u32> {
        Ok(unsafe { hp2_core::ansi::strchr(text.cast(), i32::from(b'=')).cast() })
    }
    unsafe fn with_properties<T>(&mut self, class: *mut c_void,
        body: impl FnOnce(&mut Self, &mut CoreProperties) -> RuntimeResult<T>) -> RuntimeResult<T> {
        let property_class = self.access.native_class("Core.Property")?;
        let mut properties = CoreProperties {
            class: class.cast(), field: unsafe { (*class.cast::<UStruct>()).children.cast() }, property_class,
        };
        unsafe { properties.seek(); }
        body(self, &mut properties)
    }
    unsafe fn property_valid(&mut self, properties: &mut CoreProperties) -> RuntimeResult<bool> {
        Ok(!properties.field.is_null())
    }
    unsafe fn property_next(&mut self, properties: &mut CoreProperties) -> RuntimeResult<()> {
        properties.field = unsafe { (*properties.field).next.cast() };
        unsafe { properties.seek(); }
        Ok(())
    }
    unsafe fn property_name_matches(&mut self, properties: &mut CoreProperties, name: *const u32) -> RuntimeResult<bool> {
        let index = i32::from_ne_bytes(unsafe { (*properties.field.cast::<UObject>()).name });
        self.access.write_name_text(index, &mut self.property_name)?;
        self.property_name.push(0);
        Ok(unsafe { hp2_core::ansi::stricmp(self.property_name.as_ptr().cast(), name.cast()) } == 0)
    }
    unsafe fn property_is_config(&mut self, properties: &mut CoreProperties) -> RuntimeResult<bool> {
        Ok(unsafe { (*properties.field.cast::<UProperty>()).property_flags } & hp2_core::object_config::CPF_CONFIG_FLAG != 0)
    }
    unsafe fn import_property(&mut self, properties: &mut CoreProperties, text: *const u32, actor: *mut c_void) -> RuntimeResult<()> {
        let property = properties.field.cast::<UProperty>();
        let value = unsafe { self.access.property_address(actor.cast(), property)? };
        unsafe { self.access.property_import_text_at(property, value, wide(text), ptr::null_mut(), 0)?; }
        Ok(())
    }
    unsafe fn level_info(&mut self, level: *mut c_void) -> RuntimeResult<*mut c_void> {
        unsafe { crate::package_map_level_owner::level_info(level.cast()).map(|info| info.cast()) }
    }
}
