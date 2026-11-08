//! Network bunch storage policy; Rust owns canonical archive state and arrays.
//! SDK bunches are four-byte packed; their archive bases and buffer headers
//! must not acquire the standalone types' eight-byte alignment assumptions.
use crate::net_chan::{Hp2NetChanServices, Thrown};
use hp2_abi::engine::generated::{FBitReader, FBitWriter, FInBunch, FOutBunch, TArrayRaw, UChannel, UNetConnection};
use hp2_core::{archive_state, array_storage as array, bits, string_storage, vm_string};
use hp2_core::array_storage::Hp2ArrayServices;
use std::{ffi::c_void, mem::MaybeUninit, ptr};

/// Placement lifetime leaves for native shipping archive identities.
#[repr(C)]
pub struct Hp2NetBunchServices {
    pub out_construct: unsafe extern "C" fn(*mut FOutBunch) -> i32,
    pub out_destruct: unsafe extern "C" fn(*mut FOutBunch) -> i32,
    pub in_construct: unsafe extern "C" fn(*mut FInBunch) -> i32,
    pub in_destruct: unsafe extern "C" fn(*mut FInBunch) -> i32,
    pub check_failed: unsafe extern "C" fn(*const std::ffi::c_char, *const std::ffi::c_char, i32) -> i32,
}

const TAG: [u32; 1] = [0];

fn status(code: i32) -> Result<(), Thrown> {
    if code == 0 { Ok(()) } else { Err(Thrown) }
}

unsafe fn allocator() -> Result<Hp2ArrayServices, Thrown> {
    unsafe { array::published_allocator().map_err(|_| Thrown) }
}

fn core_allocator() -> Hp2ArrayServices {
    array::system_allocator()
}

/// Resize an initialized native byte array through the canonical allocator.
///
/// # Safety
/// The header remains live during allocator reentry.
pub unsafe fn resize_buffer(buffer: *mut TArrayRaw, count: i32) -> Result<(), Thrown> {
    // SAFETY: the caller supplies a live native header.
    unsafe { array::set_exact(vm_string::slots(buffer), count, 1, TAG.as_ptr().cast(), allocator()?).map_err(|_| Thrown) }
}

/// Release an initialized native byte/string buffer.
///
/// # Safety
/// The header is exclusively retired and belongs to the published allocator.
pub unsafe fn release_buffer(buffer: *mut TArrayRaw) -> Result<(), Thrown> {
    unsafe { release_buffer_with(buffer, allocator()?) }
}

/// Release storage allocated through the Core system allocator.
///
/// # Safety
/// The header is exclusively retired and belongs to `system_allocator`.
pub unsafe fn release_core_buffer(buffer: *mut TArrayRaw) -> Result<(), Thrown> {
    unsafe { release_buffer_with(buffer, core_allocator()) }
}

unsafe fn release_buffer_with(
    buffer: *mut TArrayRaw,
    services: Hp2ArrayServices,
) -> Result<(), Thrown> {
    unsafe {
        let mut retired = buffer.read_unaligned();
        buffer.write_unaligned(TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 });
        array::release(vm_string::slots(&raw mut retired), services).map_err(|_| Thrown)
    }
}

/// Construct the Core-owned storage and scalar state of an outgoing bunch.
///
/// # Safety
/// `bunch` points to writable storage for a generated `FOutBunch` with no live
/// array allocation. The native archive vptr slot is zero; other opaque bytes are untouched.
pub unsafe fn construct_out(bunch: *mut FOutBunch) -> Result<(), Thrown> {
    unsafe {
        ptr::write_bytes(bunch.cast::<u8>(), 0, std::mem::size_of::<usize>());
        ptr::addr_of_mut!((*bunch.cast::<FBitWriter>()).buffer).write_unaligned(TArrayRaw {
            data: ptr::null_mut(),
            num: 0,
            max: 0,
        });
        archive_state::initialize_scalar_state(bunch.cast());
        if bits::writer_init(bunch.cast(), 0, core_allocator()).is_err() {
            let _ = release_core_buffer(&raw mut (*bunch.cast::<FBitWriter>()).buffer);
            return Err(Thrown);
        }
    }
    Ok(())
}

/// Destroy an outgoing bunch's Core-owned writer storage.
///
/// # Safety
/// `bunch` was initialized by [`construct_out`] and is no longer in use.
pub unsafe fn destruct_out(bunch: *mut FOutBunch) -> Result<(), Thrown> {
    unsafe { release_core_buffer(&raw mut (*bunch.cast::<FBitWriter>()).buffer) }
}

/// Populate an SDK writer through the published native allocator.
///
/// # Safety
/// The writer was constructed with a valid empty byte-array header allocated
/// from the published allocator.
pub unsafe fn initialize_writer(writer: *mut FBitWriter, capacity: i32) -> Result<(), Thrown> {
    unsafe { bits::writer_init(writer, capacity, allocator()?).map_err(|_| Thrown) }
}

unsafe fn initialize_core_writer(writer: *mut FBitWriter, capacity: i32) -> Result<(), Thrown> {
    unsafe { bits::writer_init(writer, capacity, core_allocator()).map_err(|_| Thrown) }
}

/// Populate an empty SDK reader, keeping the caller's input independent.
///
/// # Safety
/// The reader is constructed empty and input covers the requested bit count.
pub unsafe fn initialize_reader(reader: *mut FBitReader, data: *const u8, count: i32) -> Result<(), Thrown> {
    // SAFETY: the native identity and empty buffer already exist.
    unsafe { bits::reader_init(reader, data, count, allocator()?).map_err(|_| Thrown) }
}
/// Construct a Core-owned incoming bunch with reader state and a Core byte buffer.
///
/// # Safety
/// `bunch` points to writable FInBunch storage with no initialized array allocation.
pub unsafe fn construct_in_core(bunch: *mut FInBunch) -> Result<(), Thrown> {
    unsafe {
        ptr::write_bytes(bunch.cast::<u8>(), 0, std::mem::size_of::<usize>());
        ptr::addr_of_mut!((*bunch).base.buffer).write(TArrayRaw {
            data: ptr::null_mut(), num: 0, max: 0,
        });
        archive_state::initialize_scalar_state(bunch.cast());
        if bits::reader_init(bunch.cast(), ptr::null(), 0, core_allocator()).is_err() {
            let _ = release_core_buffer(&raw mut (*bunch).base.buffer);
            return Err(Thrown);
        }
        initialize_in(bunch, ptr::null_mut());
    }
    Ok(())
}

/// Retire a Core-owned incoming reader.
///
/// # Safety
/// `bunch` was initialized by [`construct_in_core`] and is no longer in use.
pub unsafe fn destruct_in_core(bunch: *mut FInBunch) -> Result<(), Thrown> {
    unsafe { release_core_buffer(&raw mut (*bunch).base.buffer) }
}


/// Initialize the scalar prefix and buffer of an SDK channel bunch.
///
/// # Safety
/// `b` is an empty, constructed SDK bunch and `channel` is live.
pub unsafe fn initialize_out(s: &Hp2NetBunchServices, b: *mut FOutBunch, channel: *mut UChannel, close: i32) -> Result<(), Thrown> {
    unsafe {
        let capacity = initialize_out_fields(b, channel, close);
        initialize_writer(b.cast(), capacity)?;
        validate_out(s, b)
    }
}

/// Initialize an outgoing bunch with Core-owned storage and no C++ identity calls.
///
/// # Safety
/// `b` is Core-constructed and `channel` is live; channel-slot and closing
/// preconditions have already been enforced by the caller.
pub unsafe fn initialize_out_core(b: *mut FOutBunch, channel: *mut UChannel, close: i32) -> Result<(), Thrown> {
    unsafe {
        let capacity = initialize_out_fields(b, channel, close);
        initialize_core_writer(b.cast(), capacity)?;
        validate_out_limit(b);
        Ok(())
    }
}

unsafe fn initialize_out_fields(b: *mut FOutBunch, channel: *mut UChannel, close: i32) -> i32 {
    unsafe {
        (*b).next = ptr::null_mut();
        (*b).channel = channel.cast();
        (*b).time = [0; 8];
        (*b).received_ack = 0;
        (*b).ch_index = (*channel).ch_index;
        (*b).ch_type = (*channel).ch_type;
        (*b).ch_sequence = 0;
        (*b).packet_id = 0;
        (*b).b_open = 0;
        (*b).b_close = close as u8;
        (*b).b_reliable = 0;
        let connection = (*channel).connection.cast::<UNetConnection>();
        out_bunch_capacity((*connection).max_packet)
    }
}

/// Initialize all named incoming metadata, including previously indeterminate fields.
///
/// # Safety
/// `b` is an empty, constructed SDK bunch.
pub unsafe fn initialize_in(b: *mut FInBunch, connection: *mut UNetConnection) {
    // SAFETY: this writes only named scalar members.
    unsafe {
        (*b).packet_id = 0;
        (*b).next = ptr::null_mut();
        (*b).connection = connection.cast();
        (*b).ch_index = 0;
        (*b).ch_type = 0;
        (*b).ch_sequence = 0;
        (*b).b_open = 0;
        (*b).b_close = 0;
        (*b).b_reliable = 0;
    }
}

unsafe fn copy_writer(
    dst: *mut FBitWriter,
    src: *const FBitWriter,
    constructing: bool,
    services: Hp2ArrayServices,
) -> Result<(), Thrown> {
    // initialized fields are read; buffers are resolved again after allocation.
    unsafe {
        ptr::addr_of_mut!((*dst).ar_ver).write_unaligned(ptr::addr_of!((*src).ar_ver).read_unaligned());
        ptr::addr_of_mut!((*dst).ar_net_ver).write_unaligned(ptr::addr_of!((*src).ar_net_ver).read_unaligned());
        ptr::addr_of_mut!((*dst).ar_licensee_ver).write_unaligned(ptr::addr_of!((*src).ar_licensee_ver).read_unaligned());
        ptr::addr_of_mut!((*dst).ar_is_loading).write_unaligned(ptr::addr_of!((*src).ar_is_loading).read_unaligned());
        ptr::addr_of_mut!((*dst).ar_is_saving).write_unaligned(ptr::addr_of!((*src).ar_is_saving).read_unaligned());
        ptr::addr_of_mut!((*dst).ar_is_trans).write_unaligned(ptr::addr_of!((*src).ar_is_trans).read_unaligned());
        ptr::addr_of_mut!((*dst).ar_is_persistent).write_unaligned(ptr::addr_of!((*src).ar_is_persistent).read_unaligned());
        ptr::addr_of_mut!((*dst).ar_for_edit).write_unaligned(ptr::addr_of!((*src).ar_for_edit).read_unaligned());
        ptr::addr_of_mut!((*dst).ar_for_client).write_unaligned(ptr::addr_of!((*src).ar_for_client).read_unaligned());
        ptr::addr_of_mut!((*dst).ar_for_server).write_unaligned(ptr::addr_of!((*src).ar_for_server).read_unaligned());
        ptr::addr_of_mut!((*dst).ar_is_error).write_unaligned(ptr::addr_of!((*src).ar_is_error).read_unaligned());
        let count = ptr::addr_of!((*src).buffer.num).read_unaligned();
        if constructing {
            // TArray<BYTE>'s copy constructor publishes the original count
            // before Realloc and memcpy uses that snapshot, not Other.Num().
            array::set_exact(vm_string::slots(&raw mut (*dst).buffer), count, 1, TAG.as_ptr().cast(), services).map_err(|_| Thrown)?;
            if count > 0 {
                ptr::copy_nonoverlapping(ptr::addr_of!((*src).buffer.data).read_unaligned().cast::<u8>(), ptr::addr_of!((*dst).buffer.data).read_unaligned().cast::<u8>(), count as usize);
            }
        } else {
            // Assignment reserves the old count, then its placement loop tests
            // the live source count. Batch only spans that cannot call out.
            array::empty(vm_string::slots(&raw mut (*dst).buffer), count, 1, TAG.as_ptr().cast(), services).map_err(|_| Thrown)?;
            let mut source_index = 0;
            while source_index < ptr::addr_of!((*src).buffer.num).read_unaligned() {
                let destination_index = ptr::addr_of!((*dst).buffer.num).read_unaligned();
                let available = ptr::addr_of!((*dst).buffer.max).read_unaligned().wrapping_sub(destination_index);
                if available > 0 {
                    let amount = (ptr::addr_of!((*src).buffer.num).read_unaligned() - source_index).min(available);
                    ptr::copy_nonoverlapping(
                        ptr::addr_of!((*src).buffer.data).read_unaligned().cast::<u8>().add(source_index as usize),
                        ptr::addr_of!((*dst).buffer.data).read_unaligned().cast::<u8>().add(destination_index as usize),
                        amount as usize,
                    );
                    ptr::addr_of!((*dst).buffer.num).cast_mut().write_unaligned(destination_index + amount);
                    source_index += amount;
                } else {
                    let index = array::add(vm_string::slots(&raw mut (*dst).buffer), 1, 1, TAG.as_ptr().cast(), services).map_err(|_| Thrown)?;
                    // SDK placement-new evaluates Other(i) after Add. A
                    // callback may relocate either buffer; reload both here.
                    ptr::copy_nonoverlapping(
                        ptr::addr_of!((*src).buffer.data).read_unaligned().cast::<u8>().add(source_index as usize),
                        ptr::addr_of!((*dst).buffer.data).read_unaligned().cast::<u8>().add(index as usize),
                        1,
                    );
                    source_index += 1;
                }
            }
        }
        ptr::addr_of_mut!((*dst).num).write_unaligned(ptr::addr_of!((*src).num).read_unaligned());
        ptr::addr_of_mut!((*dst).max).write_unaligned(ptr::addr_of!((*src).max).read_unaligned());
        Ok(())
    }
}

/// SDK bunch assignment with independent byte storage and untouched vtable identity.
///
/// # Safety
/// Both pointers address live initialized SDK bunches. Callbacks may change the
/// source count; an element whose placement Add is in flight must remain readable.
pub unsafe fn assign_out(dst: *mut FOutBunch, src: *const FOutBunch) -> Result<(), Thrown> {
    unsafe { copy_out_fields(dst, src, false, allocator()?) }
}

/// Core-owned outgoing assignment with independent system-allocated byte storage.
///
/// # Safety
/// Both bunches are live Core-owned records and source storage remains readable
/// across allocator reentry.
pub unsafe fn assign_out_core(dst: *mut FOutBunch, src: *const FOutBunch) -> Result<(), Thrown> {
    unsafe { copy_out_fields(dst, src, false, core_allocator()) }
}

/// SDK copy-construction policy after empty native archive construction.
///
/// # Safety
/// The source's initial byte prefix stays readable through allocator callbacks,
/// as required by the SDK TArray<BYTE> copy constructor.
pub unsafe fn copy_into_out(dst: *mut FOutBunch, src: *const FOutBunch) -> Result<(), Thrown> {
    unsafe { copy_out_fields(dst, src, true, allocator()?) }
}

/// Core-owned copy-construction policy for an outgoing bunch.
///
/// # Safety
/// `dst` was initialized by [`construct_out`], and the source remains readable
/// through allocator reentry.
pub unsafe fn copy_into_out_core(dst: *mut FOutBunch, src: *const FOutBunch) -> Result<(), Thrown> {
    unsafe { copy_out_fields(dst, src, true, core_allocator()) }
}

unsafe fn copy_out_fields(
    dst: *mut FOutBunch,
    src: *const FOutBunch,
    constructing: bool,
    services: Hp2ArrayServices,
) -> Result<(), Thrown> {
    if ptr::eq(dst, src) { return Ok(()); }
    unsafe {
        copy_writer(dst.cast(), src.cast(), constructing, services)?;
        (*dst).next = (*src).next;
        (*dst).channel = (*src).channel;
        (*dst).time = (*src).time;
        (*dst).received_ack = (*src).received_ack;
        (*dst).ch_index = (*src).ch_index;
        (*dst).ch_type = (*src).ch_type;
        (*dst).ch_sequence = (*src).ch_sequence;
        (*dst).packet_id = (*src).packet_id;
        (*dst).b_open = (*src).b_open;
        (*dst).b_close = (*src).b_close;
        (*dst).b_reliable = (*src).b_reliable;
        Ok(())
    }
}

/// SDK incoming assignment with independent reader storage.
///
/// # Safety
/// Both pointers address live bunches with initialized named metadata. Callbacks
/// may change the source count; a pending placement element stays readable.
pub unsafe fn assign_in(dst: *mut FInBunch, src: *const FInBunch) -> Result<(), Thrown> {
    unsafe { copy_in_fields(dst, src, false) }
}

/// SDK incoming copy-construction policy after empty native archive construction.
///
/// # Safety
/// The source's initial byte prefix stays readable through allocator callbacks,
/// as required by the SDK TArray<BYTE> copy constructor.
pub unsafe fn copy_into_in(dst: *mut FInBunch, src: *const FInBunch) -> Result<(), Thrown> {
    unsafe { copy_in_fields(dst, src, true) }
}

unsafe fn copy_in_fields(dst: *mut FInBunch, src: *const FInBunch, constructing: bool) -> Result<(), Thrown> {
    if ptr::eq(dst, src) { return Ok(()); }
    // SAFETY: the archive base is copied through its named reader-compatible
    // slots; reader Num/Pos remain in FBitReader::base.
    unsafe {
        copy_writer(dst.cast(), src.cast(), constructing, allocator()?)?;
        (*dst).packet_id = (*src).packet_id;
        (*dst).next = (*src).next;
        (*dst).connection = (*src).connection;
        (*dst).ch_index = (*src).ch_index;
        (*dst).ch_type = (*src).ch_type;
        (*dst).ch_sequence = (*src).ch_sequence;
        (*dst).base.num = (*src).base.num;
        (*dst).base.pos = (*src).base.pos;
        (*dst).b_close = (*src).b_close;
        (*dst).b_reliable = (*src).b_reliable;
        Ok(())
    }
}

/// Retire a native-constructed temporary after its body has completed.
unsafe fn destroy_out_native(s: &Hp2NetBunchServices, b: *mut FOutBunch) -> Result<(), Thrown> {
    unsafe { status((s.out_destruct)(b)) }
}

/// Allocate a reliable-list bunch using the published native lifecycle adapter.
///
/// # Safety
/// The source's initial byte prefix stays readable through allocator callbacks.
pub unsafe fn copy_out(s: &Hp2NetBunchServices, src: *const FOutBunch) -> Result<*mut FOutBunch, Thrown> {
    let mut storage = Box::<FOutBunch>::new_uninit();
    let b = storage.as_mut_ptr();
    unsafe {
        status((s.out_construct)(b))?;
        let operation = copy_into_out(b, src);
        if operation.is_err() {
            let _ = destroy_out_native(s, b);
            return operation.map(|()| b);
        }
        Ok(Box::into_raw(storage).cast())
    }
}

/// Allocate a Core-owned reliable-list bunch.
///
/// # Safety
/// The source remains readable through allocator reentry.
pub unsafe fn copy_out_core(src: *const FOutBunch) -> Result<*mut FOutBunch, Thrown> {
    let mut storage = Box::<FOutBunch>::new_uninit();
    let b = storage.as_mut_ptr();
    unsafe {
        construct_out(b)?;
        let operation = copy_into_out_core(b, src);
        if operation.is_err() {
            let _ = destruct_out(b);
            return operation.map(|()| b);
        }
        Ok(Box::into_raw(storage).cast())
    }
}

/// Allocate a queued incoming bunch with independent reader storage.
///
/// # Safety
/// Services establish SDK identity. The source's initial byte prefix remains
/// readable through callbacks, matching TArray<BYTE>'s copy-constructor contract.
pub unsafe fn copy_in(s: &Hp2NetBunchServices, src: *const FInBunch) -> Result<*mut FInBunch, Thrown> {
    let mut storage = Box::<FInBunch>::new_uninit();
    let b = storage.as_mut_ptr();
    // SAFETY: the reader archive fields live in FBitReader::base; no opaque
    // bytes are copied.
    unsafe {
        status((s.in_construct)(b))?;
        let operation = copy_into_in(b, src);
        if operation.is_err() {
            let _ = release_buffer(&raw mut (*b.cast::<FBitReader>()).buffer);
            let _ = (s.in_destruct)(b);
            return operation.map(|()| b);
        }
        Ok(Box::into_raw(storage).cast())
    }
}
/// Assigns a Core-owned incoming bunch while preserving Core allocator pairing.
///
/// # Safety
/// Both records are live Core-owned bunches and the source remains readable through reentry.
pub unsafe fn assign_in_core(dst: *mut FInBunch, src: *const FInBunch) -> Result<(), Thrown> {
    unsafe { core_copy_in_fields(dst, src, false, core_allocator()) }
}
/// Allocate a Core-owned queued incoming bunch with independently owned storage.
///
/// # Safety
/// The source remains readable through system allocator reentry.
pub unsafe fn copy_in_core(src: *const FInBunch) -> Result<*mut FInBunch, Thrown> {
    let mut storage = Box::<FInBunch>::new_uninit();
    let bunch = storage.as_mut_ptr();
    // SAFETY: the reader is separately initialized with Core-owned storage.
    unsafe {
        construct_in_core(bunch)?;
        let result = core_copy_in_fields(bunch, src, true, core_allocator());
        if result.is_err() {
            let _ = release_core_buffer(&raw mut (*bunch.cast::<FBitReader>()).buffer);
            return result.map(|()| bunch);
        }
        Ok(Box::into_raw(storage).cast())
    }
}

unsafe fn core_copy_in_fields(
    dst: *mut FInBunch,
    src: *const FInBunch,
    constructing: bool,
    services: Hp2ArrayServices,
) -> Result<(), Thrown> {
    if ptr::eq(dst, src) { return Ok(()); }
    unsafe {
        copy_writer(dst.cast(), src.cast(), constructing, services)?;
        (*dst).packet_id = (*src).packet_id;
        (*dst).next = (*src).next;
        (*dst).connection = (*src).connection;
        (*dst).ch_index = (*src).ch_index;
        (*dst).ch_type = (*src).ch_type;
        (*dst).base.num = (*src).base.num;
        (*dst).base.pos = (*src).base.pos;
        (*dst).b_open = (*src).b_open;
        (*dst).b_close = (*src).b_close;
        (*dst).b_reliable = (*src).b_reliable;
        Ok(())
    }
}

/// Releases a Core-owned incoming queue record.
///
/// # Safety
/// `bunch` was returned by [`copy_in_core`] and is unlinked and no longer in use.
pub unsafe fn delete_in_core(bunch: *mut FInBunch) -> Result<(), Thrown> {
    unsafe {
        let release = destruct_in_core(bunch);
        drop(Box::from_raw(bunch.cast::<MaybeUninit<FInBunch>>()));
        release
    }
}

/// Retire a native-adapter reliable-list bunch.
///
/// # Safety
/// `b` was returned by [`copy_out`] and is no longer linked or in use.
pub unsafe fn delete_out(s: &Hp2NetBunchServices, b: *mut FOutBunch) -> Result<(), Thrown> {
    unsafe {
        let destroy = status((s.out_destruct)(b));
        drop(Box::from_raw(b.cast::<MaybeUninit<FOutBunch>>()));
        destroy
    }
}

/// Retire a Core-owned reliable-list bunch.
///
/// # Safety
/// `b` was returned by [`copy_out_core`] and is no longer linked or in use.
pub unsafe fn delete_out_core(b: *mut FOutBunch) -> Result<(), Thrown> {
    unsafe {
        let destroy = destruct_out(b);
        drop(Box::from_raw(b.cast::<MaybeUninit<FOutBunch>>()));
        destroy
    }
}

/// Retire a Rust-allocated incoming-list bunch.
///
/// # Safety
/// `b` was returned by `copy_in` and is no longer linked or in use.
pub unsafe fn delete_in(s: &Hp2NetBunchServices, b: *mut FInBunch) -> Result<(), Thrown> {
    // Do not introduce an owning Box reference while callbacks use the raw header.
    unsafe {
        let release = release_buffer(&raw mut (*b.cast::<FBitReader>()).buffer);
        let destroy = status((s.in_destruct)(b));
        drop(Box::from_raw(b.cast::<MaybeUninit<FInBunch>>()));
        release.and(destroy)
    }
}

/// Run a scoped native-adapter bunch and destroy its SDK writer on every exit.
///
/// # Safety
/// The channel and callbacks remain live for this invocation.
pub unsafe fn with_out<T>(
    s: &Hp2NetBunchServices,
    ch: *mut UChannel,
    close: i32,
    body: impl FnOnce(*mut FOutBunch) -> Result<T, Thrown>,
) -> Result<T, Thrown> {
    let mut storage = MaybeUninit::<FOutBunch>::uninit();
    let bunch = storage.as_mut_ptr();
    unsafe {
        status((s.out_construct)(bunch))?;
        let operation = initialize_out(s, bunch, ch, close).and_then(|()| body(bunch));
        let destroy = destroy_out_native(s, bunch);
        operation.and_then(|value| destroy.map(|()| value))
    }
}

/// Run one scoped Core-owned bunch and release its system-allocated writer.
///
/// # Safety
/// `ch` remains live for this invocation and its channel-slot invariants hold.
pub unsafe fn with_out_core<T>(
    ch: *mut UChannel,
    close: i32,
    body: impl FnOnce(*mut FOutBunch) -> Result<T, Thrown>,
) -> Result<T, Thrown> {
    let mut storage = MaybeUninit::<FOutBunch>::uninit();
    let bunch = storage.as_mut_ptr();
    unsafe {
        construct_out(bunch)?;
        let operation = initialize_out_core(bunch, ch, close).and_then(|()| body(bunch));
        let destroy = destruct_out(bunch);
        operation.and_then(|value| destroy.map(|()| value))
    }
}

/// Run a scoped canonical FString, releasing storage even after a captured error.
///
/// # Safety
/// Callback uses the pointer only until it returns.
pub unsafe fn with_string<T>(body: impl FnOnce(*mut c_void) -> Result<T, Thrown>) -> Result<T, Thrown> {
    let mut value = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
    let value = &raw mut value;
    let operation = body(value.cast());
    // SAFETY: a native FString is its canonical TArray header.
    let release = unsafe { release_buffer(value) };
    operation.and_then(|result| release.map(|()| result))
}

/// Assign a terminated string using the canonical FString storage policy.
///
/// # Safety
/// Destination is an initialized FString header and text remains readable.
pub unsafe fn assign_string(value: *mut c_void, text: *const u32) -> Result<(), Thrown> {
    // SAFETY: the native FString header is addressed only through raw slots.
    unsafe { string_storage::assign_with_allocator(vm_string::slots(value.cast()), None, text.cast(), false, allocator()?, TAG.as_ptr().cast()).map_err(|_| Thrown) }
}

/// FInBunch object serialization delegates to PackageMap and propagates its
/// `Hp2FfiCall` status without allowing C++ unwinding through Rust.
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn in_serialize_object(
    s: &Hp2NetChanServices,
    b: *mut FInBunch,
    slot: *mut *mut c_void,
) -> Result<(), Thrown> {
    unsafe {
        let c = (*b).connection.cast::<UNetConnection>();
        if (s.chan_pm_serialize_object)((*c).package_map, b.cast(), slot) != 0 {
            return Err(Thrown);
        }
        Ok(())
    }
}
/// FInBunch name serialization delegates to PackageMap.
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn in_serialize_name(
    s: &Hp2NetChanServices,
    b: *mut FInBunch,
    name: *mut c_void,
) -> Result<(), Thrown> {
    unsafe {
        let c = (*b).connection.cast::<UNetConnection>();
        if (s.chan_pm_serialize_name)((*c).package_map, b.cast(), name) != 0 {
            return Err(Thrown);
        }
        Ok(())
    }
}
/// FOutBunch object serialization delegates to the owning channel PackageMap.
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn out_serialize_object(
    s: &Hp2NetChanServices,
    b: *mut FOutBunch,
    slot: *mut *mut c_void,
) -> Result<(), Thrown> {
    unsafe {
        let ch = (*b).channel.cast::<UChannel>();
        let c = (*ch).connection.cast::<UNetConnection>();
        if (s.chan_pm_serialize_object)((*c).package_map, b.cast(), slot) != 0 {
            return Err(Thrown);
        }
        Ok(())
    }
}
/// FOutBunch name serialization delegates to the owning channel PackageMap.
///
/// # Safety
/// Pointer arguments must be valid for the accesses the C++ original performs, and any
/// `s` must be the initialized `GHp2NetChanServices`. Null is undefined behavior, as in C++.
pub unsafe fn out_serialize_name(
    s: &Hp2NetChanServices,
    b: *mut FOutBunch,
    name: *mut c_void,
) -> Result<(), Thrown> {
    unsafe {
        let ch = (*b).channel.cast::<UChannel>();
        let c = (*ch).connection.cast::<UNetConnection>();
        if (s.chan_pm_serialize_name)((*c).package_map, b.cast(), name) != 0 {
            return Err(Thrown);
        }
        Ok(())
    }
}

/// `FOutBunch`'s `FBitWriter` capacity, preserving signed 32-bit arithmetic.
pub const fn out_bunch_capacity(max_packet: i32) -> i32 {
    max_packet
        .wrapping_mul(8)
        .wrapping_sub(64)
        .wrapping_sub(1)
        .wrapping_sub(16)
}

unsafe fn validate_out(s: &Hp2NetBunchServices, b: *mut FOutBunch) -> Result<(), Thrown> {
    unsafe {
        let ch = (*b).channel.cast::<UChannel>();
        let c = (*ch).connection.cast::<UNetConnection>();
        if (*ch).closing != 0
            && (s.check_failed)(c"!Channel->Closing".as_ptr(), c"UnBunch.cpp".as_ptr(), 59)
                != 0
        {
            return Err(Thrown);
        }
        if (*c).channels[(*ch).ch_index as usize] != ch.cast()
            && (s.check_failed)(
                c"Channel->Connection->Channels[Channel->ChIndex]==Channel".as_ptr(),
                c"UnBunch.cpp".as_ptr(),
                60,
            ) != 0
        {
            return Err(Thrown);
        }
        validate_out_limit(b);
        Ok(())
    }
}

unsafe fn validate_out_limit(b: *mut FOutBunch) {
    unsafe {
        let channel = (*b).channel.cast::<UChannel>();
        if (*channel).num_out_rec >= 127 + (*b).b_close as i32 {
            bits::writer_set_overflowed(b.cast());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::out_bunch_capacity;

    #[test]
    fn out_bunch_capacity_preserves_protocol_deductions_and_wrap() {
        assert_eq!(out_bunch_capacity(1024), 8111);
        assert_eq!(out_bunch_capacity(0), -81);
        assert_eq!(out_bunch_capacity(i32::MAX), -89);
    }
}
