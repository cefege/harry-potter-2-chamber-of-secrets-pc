//! Rust policy for `Engine/Src/UnDemoPenLev.cpp`.
//!
//! The C++ shell owns Unreal object construction, virtual dispatch, native
//! `FString`/package-array storage, and exception capture. This module owns the
//! demo pending-level control flow and all command/data selection.

use core::{ffi::c_void, mem, ptr};

use hp2_abi::engine::generated::UDemoPlayPendingLevel;
use hp2_core::name::NAME_SIZE;

const DEVICE: &[u32] = &[
    'i' as u32, 'n' as u32, 'i' as u32, ':' as u32, 'E' as u32, 'n' as u32, 'g' as u32, 'i' as u32,
    'n' as u32, 'e' as u32, '.' as u32, 'E' as u32, 'n' as u32, 'g' as u32, 'i' as u32, 'n' as u32,
    'e' as u32, '.' as u32, 'D' as u32, 'e' as u32, 'm' as u32, 'o' as u32, 'R' as u32, 'e' as u32,
    'c' as u32, 'o' as u32, 'r' as u32, 'd' as u32, 'i' as u32, 'n' as u32, 'g' as u32, 'D' as u32,
    'e' as u32, 'v' as u32, 'i' as u32, 'c' as u32, 'e' as u32, 0,
];
const USES: &[u32] = &['U' as u32, 'S' as u32, 'E' as u32, 'S' as u32, 0];
const WELCOME: &[u32] = &[
    'W' as u32, 'E' as u32, 'L' as u32, 'C' as u32, 'O' as u32, 'M' as u32, 'E' as u32, 0,
];
const GUID: &[u32] = &[
    'G' as u32, 'U' as u32, 'I' as u32, 'D' as u32, '=' as u32, 0,
];
const GEN: &[u32] = &['G' as u32, 'E' as u32, 'N' as u32, '=' as u32, 0];
const SIZE: &[u32] = &[
    'S' as u32, 'I' as u32, 'Z' as u32, 'E' as u32, '=' as u32, 0,
];
const FLAGS: &[u32] = &[
    'F' as u32, 'L' as u32, 'A' as u32, 'G' as u32, 'S' as u32, '=' as u32, 0,
];
const PKG: &[u32] = &['P' as u32, 'K' as u32, 'G' as u32, '=' as u32, 0];
const LEVEL: &[u32] = &[
    'L' as u32, 'E' as u32, 'V' as u32, 'E' as u32, 'L' as u32, '=' as u32, 0,
];
const SERVER_DEMO: &[u32] = &[
    'S' as u32, 'E' as u32, 'R' as u32, 'V' as u32, 'E' as u32, 'R' as u32, 'D' as u32, 'E' as u32,
    'M' as u32, 'O' as u32, 0,
];

#[repr(C)]
pub struct Hp2DemoPendingServices {
    pub log_received_text: unsafe extern "C" fn(text: *const u32) -> i32,
    pub log_missing_package: unsafe extern "C" fn(name: *const u32) -> i32,
    pub static_load_driver_class:
        unsafe extern "C" fn(name: *const u32, result: *mut *mut c_void) -> i32,
    pub construct_driver: unsafe extern "C" fn(cls: *mut c_void, result: *mut *mut c_void) -> i32,
    pub driver_init_connect: unsafe extern "C" fn(
        driver: *mut c_void,
        notify: *mut c_void,
        url: *mut c_void,
        error: *mut c_void,
        result: *mut i32,
    ) -> i32,
    pub driver_delete: unsafe extern "C" fn(driver: *mut c_void) -> i32,
    pub connection_package_map:
        unsafe extern "C" fn(conn: *mut c_void, result: *mut *mut c_void) -> i32,
    pub package_map_append_info: unsafe extern "C" fn(
        map: *mut c_void,
        guid: *const u32,
        generation: i32,
        file_size: i32,
        flags: u32,
        package_name: *const u32,
    ) -> i32,
    pub package_map_list_num: unsafe extern "C" fn(map: *mut c_void, result: *mut i32) -> i32,
    pub package_info_parent_name:
        unsafe extern "C" fn(map: *mut c_void, index: i32, result: *mut *const u32) -> i32,
    pub package_info_guid:
        unsafe extern "C" fn(map: *mut c_void, index: i32, result: *mut *const u32) -> i32,
    pub app_find_package_file:
        unsafe extern "C" fn(name: *const u32, guid: *const u32, result: *mut i32) -> i32,
    pub parse_command:
        unsafe extern "C" fn(text: *mut *const u32, command: *const u32, result: *mut i32) -> i32,
    pub parse_guid: unsafe extern "C" fn(
        text: *const u32,
        key: *const u32,
        result: *mut u32,
        parsed: *mut i32,
    ) -> i32,
    pub parse_int: unsafe extern "C" fn(
        text: *const u32,
        key: *const u32,
        result: *mut i32,
        parsed: *mut i32,
    ) -> i32,
    pub parse_dword: unsafe extern "C" fn(
        text: *const u32,
        key: *const u32,
        result: *mut u32,
        parsed: *mut i32,
    ) -> i32,
    pub parse_text: unsafe extern "C" fn(
        text: *const u32,
        key: *const u32,
        result: *mut u32,
        max: i32,
        parsed: *mut i32,
    ) -> i32,
    pub parse_fstring:
        unsafe extern "C" fn(text: *const u32, key: *const u32, parsed: *mut i32) -> i32,
    pub driver_set_client_third_person: unsafe extern "C" fn(driver: *mut c_void) -> i32,
    pub driver_set_time_zero: unsafe extern "C" fn(driver: *mut c_void) -> i32,
    pub driver_server_connection:
        unsafe extern "C" fn(driver: *mut c_void, result: *mut *mut c_void) -> i32,
    pub driver_tick_dispatch: unsafe extern "C" fn(driver: *mut c_void, delta: f32) -> i32,
    pub driver_tick_flush: unsafe extern "C" fn(driver: *mut c_void) -> i32,
    pub check_failed: unsafe extern "C" fn(expr: *const i8, file: *const i8, line: i32) -> i32,
}

macro_rules! value {
    ($callback:expr $(, $arg:expr)* $(,)?) => {{
        let mut out = mem::MaybeUninit::uninit();
        if ($callback)($($arg,)* out.as_mut_ptr()) != 0 { return Err(()); }
        out.assume_init()
    }};
}
macro_rules! action {
    ($callback:expr $(, $arg:expr)* $(,)?) => {{
        if ($callback)($($arg),*) != 0 { return Err(()); }
    }};
}

type R<T> = core::result::Result<T, ()>;

unsafe fn driver_slot(this: *mut c_void) -> *mut *mut c_void {
    unsafe { ptr::addr_of_mut!((*this.cast::<UDemoPlayPendingLevel>()).base.demo_rec_driver) }
}
unsafe fn error_slot(this: *mut c_void) -> *mut c_void {
    unsafe { ptr::addr_of_mut!((*this.cast::<UDemoPlayPendingLevel>()).base.error).cast() }
}
unsafe fn success_slot(this: *mut c_void) -> *mut i32 {
    unsafe { ptr::addr_of_mut!((*this.cast::<UDemoPlayPendingLevel>()).base.success) }
}
unsafe fn driver(this: *mut c_void) -> *mut c_void {
    unsafe { ptr::read_unaligned(driver_slot(this)) }
}

pub(crate) trait PendingConstructorOps {
    type Error;
    unsafe fn load_recording_driver_class(&mut self) -> Result<*mut c_void, Self::Error>;
    unsafe fn construct_recording_driver(&mut self, class: *mut c_void) -> Result<*mut c_void, Self::Error>;
    unsafe fn init_connect(
        &mut self, driver: *mut c_void, pending: *mut c_void,
        url: *mut c_void, error: *mut c_void,
    ) -> Result<bool, Self::Error>;
    unsafe fn delete_driver(&mut self, driver: *mut c_void) -> Result<(), Self::Error>;
}

/// Shared `UDemoPlayPendingLevel` constructor sequence for Native and Core records.
///
/// # Safety
/// `this` is a live initialized DemoPendingLevel and `url` a live FURL.
pub(crate) unsafe fn constructor_with_ops<O: PendingConstructorOps>(
    ops: &mut O,
    this: *mut c_void,
    url: *mut c_void,
) -> Result<(), O::Error> {
    unsafe {
        let class = ops.load_recording_driver_class()?;
        let driver = ops.construct_recording_driver(class)?;
        ptr::write_unaligned(driver_slot(this), driver);
        if !ops.init_connect(driver, this, url, error_slot(this))? {
            ops.delete_driver(driver)?;
            ptr::write_unaligned(driver_slot(this), ptr::null_mut());
        }
    }
    Ok(())
}

struct NativePendingConstructorOps<'a>(&'a Hp2DemoPendingServices);
impl PendingConstructorOps for NativePendingConstructorOps<'_> {
    type Error = ();
    unsafe fn load_recording_driver_class(&mut self) -> R<*mut c_void> {
        let mut out = ptr::null_mut();
        if unsafe { (self.0.static_load_driver_class)(DEVICE.as_ptr(), &mut out) } != 0 { Err(()) } else { Ok(out) }
    }
    unsafe fn construct_recording_driver(&mut self, class: *mut c_void) -> R<*mut c_void> {
        let mut out = ptr::null_mut();
        if unsafe { (self.0.construct_driver)(class, &mut out) } != 0 { Err(()) } else { Ok(out) }
    }
    unsafe fn init_connect(
        &mut self, driver: *mut c_void, pending: *mut c_void, url: *mut c_void, error: *mut c_void,
    ) -> R<bool> {
        let mut connected = 0;
        if unsafe { (self.0.driver_init_connect)(driver, pending, url, error, &mut connected) } != 0 {
            Err(())
        } else {
            Ok(connected != 0)
        }
    }
    unsafe fn delete_driver(&mut self, driver: *mut c_void) -> R<()> {
        if unsafe { (self.0.driver_delete)(driver) } != 0 { Err(()) } else { Ok(()) }
    }
}

pub unsafe fn constructor(
    s: &Hp2DemoPendingServices,
    this: *mut c_void,
    url: *mut c_void,
) -> R<()> {
    unsafe { constructor_with_ops(&mut NativePendingConstructorOps(s), this, url) }
}

pub unsafe fn notify_get_level() -> *mut c_void {
    ptr::null_mut()
}

pub(crate) trait NotifyReceivedTextOps {
    type Error;
    fn log_received_text(&mut self, text: *const u32) -> Result<(), Self::Error>;
    fn parse_command(
        &mut self,
        text: &mut *const u32,
        command: &[u32],
    ) -> Result<bool, Self::Error>;
    fn parse_guid(
        &mut self,
        text: *const u32,
        key: &[u32],
        result: &mut [u32; 4],
    ) -> Result<(), Self::Error>;
    fn parse_int(
        &mut self,
        text: *const u32,
        key: &[u32],
        result: &mut i32,
    ) -> Result<(), Self::Error>;
    fn parse_dword(
        &mut self,
        text: *const u32,
        key: &[u32],
        result: &mut u32,
    ) -> Result<(), Self::Error>;
    fn parse_text(
        &mut self,
        text: *const u32,
        key: &[u32],
        result: &mut [u32; NAME_SIZE],
    ) -> Result<(), Self::Error>;
    fn append_package_info(
        &mut self,
        conn: *mut c_void,
        guid: &[u32; 4],
        generation: i32,
        file_size: i32,
        flags: u32,
        package_name: &[u32; NAME_SIZE],
    ) -> Result<(), Self::Error>;
    fn verify_packages(&mut self, conn: *mut c_void) -> Result<bool, Self::Error>;
    fn parse_fstring(&mut self, text: *const u32, key: &[u32]) -> Result<bool, Self::Error>;
    fn set_client_third_person(&mut self) -> Result<(), Self::Error>;
    fn set_time_zero(&mut self) -> Result<(), Self::Error>;
    fn set_success(&mut self);
}

pub(crate) fn notify_received_text_policy<O: NotifyReceivedTextOps>(
    ops: &mut O,
    conn: *mut c_void,
    text0: *const u32,
) -> Result<(), O::Error> {
    let mut text = text0;
    ops.log_received_text(text)?;
    if ops.parse_command(&mut text, USES)? {
        let mut guid = [0u32; 4];
        let mut generation = 0;
        let mut file_size = 0;
        let mut flags = 0u32;
        let mut package_name = [0u32; NAME_SIZE];
        ops.parse_guid(text, GUID, &mut guid)?;
        ops.parse_int(text, GEN, &mut generation)?;
        ops.parse_int(text, SIZE, &mut file_size)?;
        ops.parse_dword(text, FLAGS, &mut flags)?;
        ops.parse_text(text, PKG, &mut package_name)?;
        ops.append_package_info(conn, &guid, generation, file_size, flags, &package_name)?;
    } else if ops.parse_command(&mut text, WELCOME)? {
        // Preserve the original parse and its failure boundary although the value is unused.
        let mut level = [0u32; NAME_SIZE];
        ops.parse_text(text, LEVEL, &mut level)?;
        if !ops.verify_packages(conn)? {
            return Ok(());
        }
        if ops.parse_fstring(text, SERVER_DEMO)? {
            ops.set_client_third_person()?;
        }
        ops.set_time_zero()?;
        ops.set_success();
    }
    Ok(())
}

struct NativeNotifyReceivedTextOps<'a> {
    services: &'a Hp2DemoPendingServices,
    this: *mut c_void,
}

impl NotifyReceivedTextOps for NativeNotifyReceivedTextOps<'_> {
    type Error = ();

    fn log_received_text(&mut self, text: *const u32) -> R<()> {
        unsafe { action!(self.services.log_received_text, text); }
        Ok(())
    }

    fn parse_command(&mut self, text: &mut *const u32, command: &[u32]) -> R<bool> {
        unsafe { Ok(value!(self.services.parse_command, text, command.as_ptr()) != 0) }
    }

    fn parse_guid(&mut self, text: *const u32, key: &[u32], result: &mut [u32; 4]) -> R<()> {
        unsafe { value!(self.services.parse_guid, text, key.as_ptr(), result.as_mut_ptr()); }
        Ok(())
    }

    fn parse_int(&mut self, text: *const u32, key: &[u32], result: &mut i32) -> R<()> {
        unsafe { value!(self.services.parse_int, text, key.as_ptr(), result); }
        Ok(())
    }

    fn parse_dword(&mut self, text: *const u32, key: &[u32], result: &mut u32) -> R<()> {
        unsafe { value!(self.services.parse_dword, text, key.as_ptr(), result); }
        Ok(())
    }

    fn parse_text(
        &mut self,
        text: *const u32,
        key: &[u32],
        result: &mut [u32; NAME_SIZE],
    ) -> R<()> {
        unsafe {
            value!(
                self.services.parse_text,
                text,
                key.as_ptr(),
                result.as_mut_ptr(),
                NAME_SIZE as i32
            );
        }
        Ok(())
    }

    fn append_package_info(
        &mut self,
        conn: *mut c_void,
        guid: &[u32; 4],
        generation: i32,
        file_size: i32,
        flags: u32,
        package_name: &[u32; NAME_SIZE],
    ) -> R<()> {
        unsafe {
            let map = value!(self.services.connection_package_map, conn);
            action!(
                self.services.package_map_append_info,
                map,
                guid.as_ptr(),
                generation,
                file_size,
                flags,
                package_name.as_ptr()
            );
        }
        Ok(())
    }

    fn verify_packages(&mut self, conn: *mut c_void) -> R<bool> {
        unsafe {
            let map = value!(self.services.connection_package_map, conn);
            let count = value!(self.services.package_map_list_num, map);
            for index in 0..count {
                let name = value!(self.services.package_info_parent_name, map, index);
                let guid = value!(self.services.package_info_guid, map, index);
                if value!(self.services.app_find_package_file, name, guid) == 0 {
                    action!(self.services.log_missing_package, name);
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn parse_fstring(&mut self, text: *const u32, key: &[u32]) -> R<bool> {
        unsafe {
            let mut parsed = mem::MaybeUninit::uninit();
            if (self.services.parse_fstring)(text, key.as_ptr(), parsed.as_mut_ptr()) != 0 {
                return Err(());
            }
            Ok(parsed.assume_init() != 0)
        }
    }

    fn set_client_third_person(&mut self) -> R<()> {
        unsafe { action!(self.services.driver_set_client_third_person, driver(self.this)); }
        Ok(())
    }

    fn set_time_zero(&mut self) -> R<()> {
        unsafe { action!(self.services.driver_set_time_zero, driver(self.this)); }
        Ok(())
    }

    fn set_success(&mut self) {
        unsafe { ptr::write_unaligned(success_slot(self.this), 1) }
    }
}

pub unsafe fn notify_received_text(
    s: &Hp2DemoPendingServices,
    this: *mut c_void,
    conn: *mut c_void,
    text0: *const u32,
) -> R<()> {
    let mut ops = NativeNotifyReceivedTextOps { services: s, this };
    notify_received_text_policy(&mut ops, conn, text0)
}

pub unsafe fn tick(s: &Hp2DemoPendingServices, this: *mut c_void, delta: f32) -> R<()> {
    unsafe {
        let demo_driver = driver(this);
        if demo_driver.is_null() {
            action!(
                s.check_failed,
                c"DemoRecDriver".as_ptr(),
                c"UnDemoPenLev.cpp".as_ptr(),
                95
            );
            return Err(());
        }
        if value!(s.driver_server_connection, demo_driver).is_null() {
            action!(
                s.check_failed,
                c"DemoRecDriver->ServerConnection".as_ptr(),
                c"UnDemoPenLev.cpp".as_ptr(),
                96
            );
            return Err(());
        }
        action!(s.driver_tick_dispatch, demo_driver, delta);
        action!(s.driver_tick_flush, demo_driver);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    const SERVICE_COUNT: usize = 24;

    thread_local! {
        static INIT_RESULT: Cell<i32> = const { Cell::new(0) };
        static DELETES: Cell<i32> = const { Cell::new(0) };
    }

    unsafe extern "C" fn unused() {
        panic!("demo_pending test: unexpected native service")
    }
    unsafe extern "C" fn load(_name: *const u32, out: *mut *mut c_void) -> i32 {
        unsafe { *out = 1usize as *mut c_void };
        0
    }
    unsafe extern "C" fn construct(_class: *mut c_void, out: *mut *mut c_void) -> i32 {
        unsafe { *out = 2usize as *mut c_void };
        0
    }
    unsafe extern "C" fn init(
        _driver: *mut c_void,
        _notify: *mut c_void,
        _url: *mut c_void,
        _error: *mut c_void,
        out: *mut i32,
    ) -> i32 {
        unsafe { *out = INIT_RESULT.with(Cell::get) };
        0
    }
    unsafe extern "C" fn delete(_driver: *mut c_void) -> i32 {
        DELETES.with(|deletes| deletes.set(deletes.get() + 1));
        0
    }

    fn services() -> Hp2DemoPendingServices {
        let raw = [unused as usize; SERVICE_COUNT];
        let mut services: Hp2DemoPendingServices = unsafe { mem::transmute(raw) };
        services.static_load_driver_class = load;
        services.construct_driver = construct;
        services.driver_init_connect = init;
        services.driver_delete = delete;
        services
    }

    #[test]
    fn constructor_retains_only_a_connected_driver() {
        let services = services();
        let mut pending: UDemoPlayPendingLevel = unsafe { mem::zeroed() };

        INIT_RESULT.with(|result| result.set(0));
        DELETES.with(|deletes| deletes.set(0));
        unsafe {
            constructor(&services, (&raw mut pending).cast(), ptr::null_mut()).unwrap();
            assert!(driver((&raw mut pending).cast()).is_null());
        }
        assert_eq!(DELETES.with(Cell::get), 1);

        INIT_RESULT.with(|result| result.set(1));
        DELETES.with(|deletes| deletes.set(0));
        unsafe {
            constructor(&services, (&raw mut pending).cast(), ptr::null_mut()).unwrap();
            assert_eq!(driver((&raw mut pending).cast()), 2usize as *mut c_void);
        }
        assert_eq!(DELETES.with(Cell::get), 0);
    }
}
