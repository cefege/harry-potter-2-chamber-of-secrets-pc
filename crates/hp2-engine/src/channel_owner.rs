//! Canonical metadata for Engine's abstract base network channel.

use hp2_abi::engine::generated::{FInBunch, FOutBunch, TArrayRaw, UChannel, UControlChannel, UNetConnection, UObject};
use hp2_core::array_storage::{self, Hp2ArraySlots};
use hp2_core::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use hp2_core::core_class_specs::{NativeClassSpec, NativeRecordKind};
use hp2_core::object_owner::ClassOps;
use std::{alloc::Layout, ffi::c_void, ptr, sync::OnceLock};

const OBJECT: ClassOps = NativeRecordKind::Object.operations();

const OBJECT_FLAGS: u32 = 4 | 0x0008_0000 | 0x0000_4000 | 0x0400_0000;
const CLASS_ABSTRACT: u32 = 0x0000_0001;

static LAN_PLAY: OnceLock<i32> = OnceLock::new();

fn cached_lan_play(access: CoreAccess<'_>) -> i32 {
    const LANPLAY: [i32; 8] = [108, 97, 110, 112, 108, 97, 121, 0];
    *LAN_PLAY.get_or_init(|| {
        i32::from(unsafe {
            hp2_core::search::parse_param(access.command_line().as_ptr().cast(), LANPLAY.as_ptr())
        })
    })
}
const CLASS_TRANSIENT: u32 = 0x0000_0008;

pub(crate) static NATIVE_CLASSES: &[NativeClassSpec] = &[
    NativeClassSpec {
        short_name: "Channel", package: "Engine", super_name: Some("Object"), within: "Object",
        flags: CLASS_ABSTRACT | CLASS_TRANSIENT, object_flags: OBJECT_FLAGS, config: "System",
        guid: [0; 4], kind: NativeRecordKind::Object, layout: Some(Layout::new::<UChannel>()),
        operations: Some(CHANNEL_OPERATIONS), properties: &[], defaults: &[],
    },
    NativeClassSpec {
        short_name: "ControlChannel", package: "Engine", super_name: Some("Channel"), within: "Object",
        flags: CLASS_TRANSIENT, object_flags: OBJECT_FLAGS, config: "System",
        guid: [0; 4], kind: NativeRecordKind::Object, layout: Some(Layout::new::<UControlChannel>()),
        operations: Some(CONTROL_OPERATIONS), properties: &[], defaults: &[],
    },
];

const CHANNEL_OPERATIONS: ClassOps = {
    let mut operations = OBJECT;
    operations.destroy = destroy_channel;
    operations
};

const CONTROL_OPERATIONS: ClassOps = {
    let mut operations = OBJECT;
    operations.destroy = destroy_channel;
    operations
};

/// Closes a live canonical channel, reliably sending a close bunch when its connection is open.
///
/// # Safety
/// `channel` must be a live canonical channel whose connection and indexed slot are valid.
pub unsafe fn close(access: CoreAccess<'_>, channel: *mut UChannel) -> RuntimeResult<()> {
    if channel.is_null() { return Err(RuntimeError::new("Null canonical UChannel")); }
    unsafe { close_with_ops(&CoreCloseOps { access }, channel) }
}

/// Initializes the base `UChannel` fields without publishing its connection slot.
///
/// # Safety
/// `channel` and `connection` are live canonical records.
pub unsafe fn init_channel(
    channel: *mut UChannel,
    connection: *mut UNetConnection,
    index: i32,
    opened_locally: i32,
) -> RuntimeResult<()> {
    if channel.is_null() || connection.is_null() {
        return Err(RuntimeError::new("Invalid canonical channel initialization"));
    }
    unsafe { crate::net_chan::channel_init(channel, connection, index, opened_locally); }
    Ok(())
}

/// Runs the dynamic channel destructor, then retires its UObject storage.
///
/// # Safety
/// `channel` is a live registered canonical channel.
pub unsafe fn delete_channel(access: CoreAccess<'_>, channel: *mut UChannel) -> RuntimeResult<()> {
    if channel.is_null() { return Err(RuntimeError::new("Null canonical UChannel")); }
    unsafe {
        access.conditional_destroy(channel.cast())?;
        access.delete_object(channel.cast())
    }
}

/// Creates and registers the canonical control channel at connection slot zero.
///
/// # Safety
/// `connection` must be a live canonical connection whose channel arrays are initialized.
pub unsafe fn create_control_channel(
    access: CoreAccess<'_>,
    connection: *mut UNetConnection,
) -> RuntimeResult<*mut UControlChannel> {
    if connection.is_null() {
        return Err(RuntimeError::new("Null canonical UNetConnection"));
    }
    if unsafe { !(*connection).channels[0].is_null() } {
        return Err(RuntimeError::new("Channels[ChIndex]==NULL"));
    }
    let class = access.native_class("Engine.ControlChannel")?;
    let channel = unsafe {
        access.construct_object(
            class,
            access.transient_package(),
            0,
            0,
            ptr::null_mut(),
        )?
    }.cast::<UControlChannel>();
    unsafe {
        (*channel.cast::<UChannel>()).ch_type = crate::level_net::CHTYPE_CONTROL;
        crate::net_chan::channel_init(channel.cast(), connection, 0, 1);
        (*connection).channels[0] = channel.cast();
    }
    let slots = Hp2ArraySlots {
        data: unsafe { ptr::addr_of_mut!((*connection).open_channels.data).cast() },
        num: unsafe { ptr::addr_of_mut!((*connection).open_channels.num) },
        max: unsafe { ptr::addr_of_mut!((*connection).open_channels.max) },
    };
    let index = unsafe {
        array_storage::add(
            slots,
            1,
            std::mem::size_of::<*mut c_void>() as i32,
            ptr::null(),
            array_storage::system_allocator(),
        )
    }.map_err(|_| RuntimeError::new("Core OpenChannels allocation failed"))?;
    unsafe {
        (*connection).open_channels.data.cast::<*mut c_void>()
            .add(index as usize).write(channel.cast());
    }
    Ok(channel)
}

/// Serializes and reliably sends one control-channel text message.
///
/// # Safety
/// `channel` must be a live canonical control channel; `text` must be readable
/// through its terminating NUL for the duration of the call.
pub unsafe fn serialize_control(
    access: CoreAccess<'_>,
    channel: *mut UControlChannel,
    text: *const u32,
) -> RuntimeResult<()> {
    if channel.is_null() || text.is_null() {
        return Err(RuntimeError::new("Invalid control-channel text arguments"));
    }
    let mut storage = std::mem::MaybeUninit::<FOutBunch>::uninit();
    let bunch = storage.as_mut_ptr();
    unsafe { net_bunch::construct_out(bunch) }
        .map_err(|_| RuntimeError::new("Core control bunch construction failed"))?;
    let operation = unsafe {
        net_bunch::initialize_out_core(bunch, channel.cast(), 0)
            .map_err(|_| RuntimeError::new("Core control bunch initialization failed"))
            .and_then(|()| {
                crate::net_chan::control_send_with_ops(
                    &CoreControlSendOps { access },
                    channel.cast(),
                    bunch,
                    text,
                )
            })
    };
    let destroy_bunch = unsafe { net_bunch::destruct_out(bunch) }
        .map_err(|_| RuntimeError::new("Core control bunch destruction failed"));
    operation.and(destroy_bunch)
}
struct CoreControlSendOps<'a> {
    access: CoreAccess<'a>,
}

impl crate::net_chan::ControlSendOps for CoreControlSendOps<'_> {
    type Error = RuntimeError;

    unsafe fn serialize_text(
        &self,
        bunch: *mut FOutBunch,
        text: *const u32,
    ) -> RuntimeResult<()> {
        if text.is_null() {
            return Err(RuntimeError::new("Invalid control-channel text"));
        }
        let mut length = 0usize;
        while unsafe { *text.add(length) } != 0 {
            length = length.checked_add(1)
                .ok_or_else(|| RuntimeError::new("Control-channel text length overflow"))?;
        }
        let text = unsafe { std::slice::from_raw_parts(text, length + 1) };
        let mut string = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
        unsafe { self.access.assign_fstring(&mut string, text)?; }
        let services = core_control_string_services();
        let status = unsafe {
            hp2_core::fstring_archive::serialize(
                bunch.cast(),
                (&mut string as *mut TArrayRaw).cast(),
                &services,
            )
        };
        let destroyed = unsafe { self.access.destroy_owned_fstring(&mut string) };
        destroyed?;
        if status != 0 {
            return Err(RuntimeError::new("Core control string serialization failed"));
        }
        Ok(())
    }

    unsafe fn bunch_is_error(&self, bunch: *mut FOutBunch) -> RuntimeResult<bool> {
        Ok(unsafe { (*bunch).base.ar_is_error != 0 })
    }

    fn log_overflow(&self) -> RuntimeResult<()> {
        let category = self.access.intern_str("DevNet", hp2_core::name_owner::FindName::Add)?;
        self.access.emit_log(
            category,
            &[67, 111, 110, 116, 114, 111, 108, 32, 99, 104, 97, 110, 110, 101, 108, 32,
                98, 117, 110, 99, 104, 32, 111, 118, 101, 114, 102, 108, 111, 119, 101, 100],
        )
    }

    unsafe fn send(
        &self,
        channel: *mut UChannel,
        bunch: *mut FOutBunch,
    ) -> RuntimeResult<()> {
        unsafe { crate::net_chan::channel_send_bunch_core(self.access, channel, bunch, 1)?; }
        Ok(())
    }
}


fn core_control_string_services() -> hp2_core::fstring_archive::Hp2FStringArchiveServices {
    use hp2_core::fstring_archive::Hp2FStringArchiveServices;
    Hp2FStringArchiveServices {
        context: ptr::null_mut(),
        archive_is_saving: control_archive_saving,
        archive_is_loading: control_archive_loading,
        archive_count_string: control_archive_count_string,
        archive_serialize_index: control_archive_serialize_index,
        archive_serialize_byte: control_archive_serialize_byte,
        archive_serialize_unicode: control_archive_serialize_unicode,
        string_num: control_string_num,
        string_data: control_string_data,
        string_resize_for_load: control_string_resize,
        string_set_num: control_string_set_num,
        string_empty: control_string_empty,
        wire_allocate: control_string_wire_allocate,
        tchar_to_ansi: control_tchar_to_ansi,
        ansi_to_tchar: control_ansi_to_tchar,
        string_length_overflow: control_string_overflow,
    }
}

unsafe extern "C" fn control_archive_saving(_: *mut c_void) -> i32 { 1 }
unsafe extern "C" fn control_archive_loading(_: *mut c_void) -> i32 { 0 }
unsafe extern "C" fn control_archive_count_string(_: *mut c_void, _: *mut c_void) -> i32 { 0 }
unsafe extern "C" fn control_archive_serialize_index(ar: *mut c_void, value: *mut i32) -> i32 {
    if value.is_null() { return 1; }
    let value_in = unsafe { *value };
    match hp2_core::compact_index::encode(value_in, |byte| {
        unsafe { hp2_core::bits::writer_serialize_bits(ar.cast(), &byte, 8); }
        Ok::<u8, ()>(byte)
    }) {
        Ok(encoded) => {
            unsafe { *value = encoded; }
            0
        }
        Err(()) => 1,
    }
}
unsafe extern "C" fn control_archive_serialize_byte(ar: *mut c_void, value: *mut u8) -> i32 {
    if value.is_null() { return 1; }
    unsafe { hp2_core::bits::writer_serialize_bits(ar.cast(), value, 8); }
    0
}
unsafe extern "C" fn control_archive_serialize_unicode(ar: *mut c_void, value: *mut u16) -> i32 {
    if value.is_null() { return 1; }
    unsafe { hp2_core::bits::writer_serialize_bits(ar.cast(), value.cast(), 16); }
    0
}
unsafe extern "C" fn control_string_num(string: *const c_void) -> i32 {
    unsafe { (*string.cast::<TArrayRaw>()).num }
}
unsafe extern "C" fn control_string_data(string: *mut c_void) -> *mut c_void {
    unsafe { (*string.cast::<TArrayRaw>()).data }
}
unsafe extern "C" fn control_string_resize(_: *mut c_void, _: i32) -> i32 { 1 }
unsafe extern "C" fn control_string_set_num(_: *mut c_void, _: i32) {}
unsafe extern "C" fn control_string_empty(_: *mut c_void) -> i32 { 0 }
unsafe extern "C" fn control_string_wire_allocate(
    _: *mut c_void,
    _: i32,
    _: *mut *mut u16,
) -> i32 { 1 }
unsafe extern "C" fn control_tchar_to_ansi(value: i32) -> u8 {
    if (0..=255).contains(&value) { value as u8 } else { b'?' }
}
unsafe extern "C" fn control_ansi_to_tchar(value: u8) -> i32 { i32::from(value) }
unsafe extern "C" fn control_string_overflow() -> i32 { 1 }

struct CoreCloseOps<'a> {
    access: CoreAccess<'a>,
}

impl CloseOps for CoreCloseOps<'_> {
    type Error = RuntimeError;

    unsafe fn channel_is_registered(&self, channel: *mut UChannel) -> Result<bool, Self::Error> {
        let connection = unsafe { (*channel).connection.cast::<UNetConnection>() };
        let index = unsafe { (*channel).ch_index };
        Ok(!connection.is_null() && (0..1023).contains(&index)
            && unsafe { (*connection).channels[index as usize] == channel.cast() })
    }

    unsafe fn connection_state(&self, channel: *mut UChannel) -> Result<i32, Self::Error> {
        Ok(unsafe { (*(*channel).connection.cast::<UNetConnection>()).state })
    }

    unsafe fn is_closing(&self, channel: *mut UChannel) -> bool {
        unsafe { (*channel).closing != 0 }
    }

    unsafe fn with_close_bunch<T>(
        &self,
        channel: *mut UChannel,
        body: impl FnOnce(*mut FOutBunch) -> Result<T, Self::Error>,
    ) -> Result<T, Self::Error> {
        let mut operation_error = None;
        let result = unsafe {
            net_bunch::with_out_core(channel, 1, |bunch| {
                body(bunch).map_err(|error| {
                    operation_error = Some(error);
                    crate::net_chan::Thrown
                })
            })
        };
        result.map_err(|_| operation_error.unwrap_or_else(|| RuntimeError::new("Core close bunch construction failed")))
    }

    unsafe fn bunch_is_error(&self, bunch: *mut FOutBunch) -> Result<bool, Self::Error> {
        Ok(unsafe { (*bunch).base.ar_is_error != 0 })
    }

    unsafe fn bunch_is_close(&self, bunch: *mut FOutBunch) -> bool {
        unsafe { (*bunch).b_close != 0 }
    }

    unsafe fn set_reliable(&self, bunch: *mut FOutBunch) {
        unsafe { (*bunch).b_reliable = 1; }
    }

    unsafe fn send_close_bunch(
        &self,
        channel: *mut UChannel,
        bunch: *mut FOutBunch,
    ) -> Result<(), Self::Error> {
        unsafe { crate::net_chan::channel_send_bunch_core(self.access, channel, bunch, 0) }
            .map(|_| ())
    }

    fn invalid_channel_slot(&self) -> Result<(), Self::Error> {
        Err(RuntimeError::new("Connection->Channels[ChIndex]==this"))
    }

    fn invalid_close_bunch(&self) -> Result<(), Self::Error> {
        Err(RuntimeError::new("!CloseBunch.IsError()"))
    }

    fn invalid_close_flag(&self) -> Result<(), Self::Error> {
        Err(RuntimeError::new("CloseBunch.bClose"))
    }
}

use crate::net_bunch;


unsafe fn destroy_channel(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    unsafe { destroy_channel_body(access, object.cast()) }
}


/// Operations used by the source-faithful close-bunch policy.
///
/// Implementations provide only data access and engine leaves; this module
/// owns the close-state branches and reliable close-bunch ordering.
pub(crate) trait CloseOps {
    type Error;

    /// Returns whether the channel occupies its connection's indexed slot.
    ///
    /// # Safety
    /// `channel` and its connection must be live.
    unsafe fn channel_is_registered(&self, channel: *mut UChannel) -> Result<bool, Self::Error>;

    /// Returns the native connection state value.
    ///
    /// # Safety
    /// `channel` and its connection must be live.
    unsafe fn connection_state(&self, channel: *mut UChannel) -> Result<i32, Self::Error>;

    /// Returns whether the channel has already entered its closing state.
    ///
    /// # Safety
    /// `channel` must be live.
    unsafe fn is_closing(&self, channel: *mut UChannel) -> bool;

    /// Runs `body` with a constructed close bunch and retires it afterward.
    ///
    /// # Safety
    /// `channel` must be live; `body` must not retain the bunch pointer.
    unsafe fn with_close_bunch<T>(
        &self,
        channel: *mut UChannel,
        body: impl FnOnce(*mut hp2_abi::engine::generated::FOutBunch) -> Result<T, Self::Error>,
    ) -> Result<T, Self::Error>;

    /// Checks the bunch archive error flag.
    ///
    /// # Safety
    /// `bunch` must be a live constructed bunch.
    unsafe fn bunch_is_error(
        &self,
        bunch: *mut hp2_abi::engine::generated::FOutBunch,
    ) -> Result<bool, Self::Error>;

    /// Checks the bunch close bit.
    ///
    /// # Safety
    /// `bunch` must be a live constructed bunch.
    unsafe fn bunch_is_close(
        &self,
        bunch: *mut hp2_abi::engine::generated::FOutBunch,
    ) -> bool;

    /// Marks the bunch reliable.
    ///
    /// # Safety
    /// `bunch` must be a live constructed bunch.
    unsafe fn set_reliable(&self, bunch: *mut hp2_abi::engine::generated::FOutBunch);

    /// Sends the bunch with merging disabled.
    ///
    /// # Safety
    /// `channel` and `bunch` must remain live through the send operation.
    unsafe fn send_close_bunch(
        &self,
        channel: *mut UChannel,
        bunch: *mut hp2_abi::engine::generated::FOutBunch,
    ) -> Result<(), Self::Error>;

    /// Reports a failed native channel-slot assertion.
    fn invalid_channel_slot(&self) -> Result<(), Self::Error>;

    /// Reports a failed native close-bunch archive assertion.
    fn invalid_close_bunch(&self) -> Result<(), Self::Error>;

    /// Reports a failed native close-bit assertion.
    fn invalid_close_flag(&self) -> Result<(), Self::Error>;
}
/// Drains queued records, unregisters the channel, clears its connection slot, then destroys UObject state.
///
/// # Safety
/// `channel` is a live canonical channel still registered in its live connection.
pub(crate) unsafe fn destroy_channel_body(access: CoreAccess<'_>, channel: *mut UChannel) -> RuntimeResult<()> {
    if channel.is_null() {
        return Err(RuntimeError::new("Null canonical UChannel"));
    }
    let connection = unsafe { (*channel).connection.cast::<UNetConnection>() };
    let index = unsafe { (*channel).ch_index };
    if connection.is_null() || !(0..1023).contains(&index)
        || unsafe { (*connection).channels[index as usize] != channel.cast() }
    {
        return Err(RuntimeError::new("Connection->Channels[ChIndex]==this"));
    }
    let mut output = unsafe { (*channel).out_rec.cast::<FOutBunch>() };
    let mut input = unsafe { (*channel).in_rec.cast::<FInBunch>() };
    unsafe {
        (*channel).out_rec = ptr::null_mut();
        (*channel).in_rec = ptr::null_mut();
        (*channel).num_out_rec = 0;
        (*channel).num_in_rec = 0;
    }
    let mut cleanup = Ok(());
    while !output.is_null() {
        let next = unsafe { (*output).next.cast::<FOutBunch>() };
        let result = unsafe { net_bunch::delete_out_core(output) }
            .map_err(|_| RuntimeError::new("Core outgoing bunch release failed"));
        cleanup = cleanup.and(result);
        output = next;
    }
    while !input.is_null() {
        let next = unsafe { (*input).next.cast::<FInBunch>() };
        let result = unsafe { net_bunch::delete_in_core(input) }
            .map_err(|_| RuntimeError::new("Core incoming bunch release failed"));
        cleanup = cleanup.and(result);
        input = next;
    }
    cleanup?;
    let slots = Hp2ArraySlots {
        data: unsafe { ptr::addr_of_mut!((*connection).open_channels.data).cast() },
        num: unsafe { ptr::addr_of_mut!((*connection).open_channels.num) },
        max: unsafe { ptr::addr_of_mut!((*connection).open_channels.max) },
    };
    let mut position = 0;
    let mut removed = 0;
    while position < unsafe { (*connection).open_channels.num } {
        let array = unsafe { (*connection).open_channels };
        let entry = unsafe {
            array.data.cast::<*mut c_void>().add(position as usize).read_unaligned()
        };
        if entry == channel.cast() {
            unsafe {
                array_storage::remove(
                    slots,
                    position,
                    1,
                    std::mem::size_of::<*mut c_void>() as i32,
                    ptr::null(),
                    array_storage::system_allocator(),
                )
            }.map_err(|_| RuntimeError::new("Core OpenChannels removal failed"))?;
            removed += 1;
        } else {
            position += 1;
        }
    }
    if removed != 1 {
        return Err(RuntimeError::new("Connection->OpenChannels.RemoveItem(this)==1"));
    }
    unsafe {
        (*connection).channels[index as usize] = ptr::null_mut();
        (*channel).connection = ptr::null_mut();
        (OBJECT.destroy)(access, channel.cast())?;
    }
    Ok(())
}

/// Constructor-only canonical channel selection; registration is left to the
/// shared `UNetConnection::CreateChannel` policy.
///
/// # Safety
/// `access` belongs to the engine thread and class construction is permitted.
pub(crate) unsafe fn construct_channel(
    access: CoreAccess<'_>,
    channel_type: i32,
) -> RuntimeResult<*mut UChannel> {
    match channel_type {
        crate::level_net::CHTYPE_CONTROL => {
            let class = access.native_class("Engine.ControlChannel")?;
            Ok(unsafe {
                access.construct_object(class, access.transient_package(), 0, 0, ptr::null_mut())?
            }.cast())
        }
        crate::level_net::CHTYPE_ACTOR => unsafe {
            crate::actor_channel_owner::construct(access).map(|channel| channel.cast())
        },
        crate::level_net::CHTYPE_FILE => unsafe {
            crate::file_channel_owner::construct(access).map(|channel| channel.cast())
        },
        _ => Err(RuntimeError::new("Unknown canonical channel type")),
    }
}

/// Applies the selected subtype's real initialization without publishing it.
///
/// # Safety
/// `channel` and `connection` are live canonical records.
pub(crate) unsafe fn initialize_channel(
    access: CoreAccess<'_>,
    channel_type: i32,
    channel: *mut UChannel,
    connection: *mut UNetConnection,
    index: i32,
    opened_locally: i32,
) -> RuntimeResult<()> {
    if channel.is_null() {
        return Err(RuntimeError::new("Null canonical UChannel"));
    }
    unsafe { (*channel).ch_type = channel_type; }
    match channel_type {
        crate::level_net::CHTYPE_CONTROL => unsafe {
            init_channel(channel, connection, index, opened_locally)
        },
        crate::level_net::CHTYPE_ACTOR => unsafe {
            crate::actor_channel_owner::initialize(
                access,
                channel.cast(),
                connection,
                index,
                opened_locally,
            )
        },
        crate::level_net::CHTYPE_FILE => unsafe {
            crate::file_channel_owner::initialize(
                access,
                channel.cast(),
                connection,
                index,
                opened_locally,
            )
        },
        _ => Err(RuntimeError::new("Unknown canonical channel type")),
    }
}

pub(crate) unsafe fn add_open_channel(
    connection: *mut UNetConnection,
    channel: *mut UChannel,
) -> RuntimeResult<()> {
    let slots = Hp2ArraySlots {
        data: unsafe { ptr::addr_of_mut!((*connection).open_channels.data).cast() },
        num: unsafe { ptr::addr_of_mut!((*connection).open_channels.num) },
        max: unsafe { ptr::addr_of_mut!((*connection).open_channels.max) },
    };
    let index = unsafe {
        array_storage::add(
            slots,
            1,
            std::mem::size_of::<*mut c_void>() as i32,
            ptr::null(),
            array_storage::system_allocator(),
        )
    }
    .map_err(|_| RuntimeError::new("Core OpenChannels allocation failed"))?;
    unsafe {
        (*connection).open_channels.data.cast::<*mut c_void>()
            .add(index as usize)
            .write(channel.cast());
    }
    Ok(())
}

struct CoreChannelReceiveOps<'a> {
    access: CoreAccess<'a>,
}

impl crate::net_chan::ChannelReceiveOps for CoreChannelReceiveOps<'_> {
    type Error = RuntimeError;

    unsafe fn check_fail(
        &self,
        expression: &'static std::ffi::CStr,
        _line: i32,
    ) -> RuntimeResult<()> {
        Err(RuntimeError::new(expression.to_str().unwrap_or("Native channel assertion failed")))
    }
    unsafe fn copy_in(&self, bunch: *const FInBunch) -> RuntimeResult<*mut FInBunch> {
        unsafe { net_bunch::copy_in_core(bunch) }
            .map_err(|_| RuntimeError::new("Core incoming bunch copy failed"))
    }
    unsafe fn delete_in(&self, bunch: *mut FInBunch) -> RuntimeResult<()> {
        unsafe { net_bunch::delete_in_core(bunch) }
            .map_err(|_| RuntimeError::new("Core incoming bunch cleanup failed"))
    }
    unsafe fn received_bunch(&self, channel: *mut UChannel, bunch: *mut FInBunch) -> RuntimeResult<()> {
        match unsafe { (*channel).ch_type } {
            crate::level_net::CHTYPE_CONTROL => unsafe {
                crate::net_chan::control_received_bunch_core(self.access, channel.cast(), bunch)
            },
            crate::level_net::CHTYPE_ACTOR => unsafe {
                crate::actor_channel_owner::received_bunch(self.access, channel.cast(), bunch)
            },
            crate::level_net::CHTYPE_FILE => unsafe {
                crate::file_channel_owner::received_bunch(self.access, channel.cast(), bunch)
            },
            _ => Err(RuntimeError::new("Unknown canonical channel type")),
        }
    }
    unsafe fn delete_channel(&self, channel: *mut UChannel) -> RuntimeResult<()> {
        unsafe { delete_channel(self.access, channel) }
    }
    unsafe fn delete_out(&self, bunch: *mut FOutBunch) -> RuntimeResult<()> {
        unsafe { net_bunch::delete_out_core(bunch) }
            .map_err(|_| RuntimeError::new("Core outgoing bunch cleanup failed"))
    }
    unsafe fn send_raw_bunch(&self, channel: *mut UChannel, bunch: *mut FOutBunch) -> RuntimeResult<()> {
        let connection = unsafe { (*channel).connection.cast::<UNetConnection>() };
        unsafe { crate::connection_owner::send_raw_bunch(self.access, connection, bunch, false) }
            .map(|_| ())
    }
}

/// Ticks one canonical channel through its source-defined inherited/override path.
///
/// ActorChannel::Tick and ControlChannel::Tick only execute the base channel
/// tick; FileChannel additionally transmits queued file data.
///
/// # Safety
/// `channel` is a live canonical channel with initialized connection/queue state.
pub unsafe fn tick(access: CoreAccess<'_>, channel: *mut UChannel) -> RuntimeResult<()> {
    if channel.is_null() {
        return Err(RuntimeError::new("Null canonical UChannel"));
    }
    match unsafe { (*channel).ch_type } {
        crate::level_net::CHTYPE_CONTROL | crate::level_net::CHTYPE_ACTOR => unsafe {
            crate::net_chan::channel_tick_core(access, channel)
        },
        crate::level_net::CHTYPE_FILE => unsafe {
            crate::file_channel_owner::tick(
                access,
                channel.cast(),
                cached_lan_play(access),
            )
        },
        _ => Err(RuntimeError::new("Unknown canonical channel type")),
    }
}

/// Handles one canonical unsequenced bunch through the shared reliable queue policy.
pub unsafe fn received_raw(
    access: CoreAccess<'_>,
    channel: *mut UChannel,
    bunch: *mut FInBunch,
) -> RuntimeResult<()> {
    unsafe { crate::net_chan::channel_received_raw_with(&CoreChannelReceiveOps { access }, channel, bunch) }
}

/// Applies one canonical channel acknowledgement through the shared queue policy.
pub unsafe fn received_acks(access: CoreAccess<'_>, channel: *mut UChannel) -> RuntimeResult<()> {
    unsafe { crate::net_chan::channel_received_acks_with(&CoreChannelReceiveOps { access }, channel) }
}

/// Applies the base channel resend policy for one NAKed packet.
pub unsafe fn received_nak(
    access: CoreAccess<'_>,
    channel: *mut UChannel,
    packet_id: i32,
) -> RuntimeResult<()> {
    unsafe {
        crate::net_chan::channel_received_nak_with(
            &CoreChannelReceiveOps { access },
            channel,
            packet_id,
        )
    }
}

/// Executes the native `UChannel::Close` branches using the provided leaves.
///
/// # Safety
/// `channel` is live and its connection/channel-slot invariants hold.
pub(crate) unsafe fn close_with_ops<O: CloseOps>(
    ops: &O,
    channel: *mut UChannel,
) -> Result<(), O::Error> {
    if !unsafe { ops.channel_is_registered(channel)? } {
        ops.invalid_channel_slot()?;
    }
    if !unsafe { ops.is_closing(channel) } {
        let state = unsafe { ops.connection_state(channel)? };
        if state == 3 || state == 2 {
            unsafe {
                ops.with_close_bunch(channel, |bunch| {
                    if ops.bunch_is_error(bunch)? {
                        ops.invalid_close_bunch()?;
                    }
                    if !ops.bunch_is_close(bunch) {
                        ops.invalid_close_flag()?;
                    }
                    ops.set_reliable(bunch);
                    ops.send_close_bunch(channel, bunch)
                })?;
            }
        }
    }
    Ok(())
}

const _: () = assert!(std::mem::size_of::<UChannel>() == 144);
