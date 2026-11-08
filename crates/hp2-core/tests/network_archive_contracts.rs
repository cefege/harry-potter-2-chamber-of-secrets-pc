use hp2_abi::engine::generated::{FBitReader, FBitWriter, UBoolProperty, UClass, UObject, UProperty};
use hp2_core::{
    bits,
    core_access::{CoreAccess, CoreOptions, CoreOwner, RuntimeError, RuntimeResult},
    object_archive::{ArchiveFlags, ArchiveMapping, NetworkReader, NetworkWriter, ObjectArchive},
};
use std::{cell::UnsafeCell, mem::MaybeUninit, path::PathBuf, ptr, slice};

struct Packet<'a> {
    reader: UnsafeCell<FBitReader>,
    _storage: &'a [u8],
}
impl<'a> Packet<'a> {
    fn new(bytes: &'a [u8], bit_count: i32) -> Self {
        // SAFETY: compiler-probed archive views contain integers and raw pointers.
        let mut reader: FBitReader = unsafe { std::mem::zeroed() };
        reader.ar_is_loading = 1;
        reader.buffer.data = bytes.as_ptr().cast_mut().cast();
        reader.buffer.num = bytes.len() as i32;
        reader.buffer.max = bytes.len() as i32;
        reader.num = bit_count;
        Self { reader: UnsafeCell::new(reader), _storage: bytes }
    }
}
/// Finite-capacity real `FBitWriter` packet; no mocked byte echo.
struct WriterPacket {
    writer: UnsafeCell<FBitWriter>,
}
impl WriterPacket {
    fn new(max_bits: i32) -> Self {
        // SAFETY: compiler-probed archive storage is zero-initialized then
        // initialized by the canonical FBitWriter constructor kernel.
        let mut writer: FBitWriter = unsafe { std::mem::zeroed() };
        // SAFETY: the local writer and system allocator remain live.
        unsafe {
            bits::writer_init(&mut writer, max_bits, hp2_core::array_storage::system_allocator())
                .expect("initialize real FBitWriter");
        }
        Self { writer: UnsafeCell::new(writer) }
    }
    fn ptr(&self) -> *mut FBitWriter { self.writer.get() }
    fn written(&self) -> Vec<u8> {
        // SAFETY: the initialized writer owns its buffer for this borrow.
        unsafe {
            let writer = &*self.writer.get();
            slice::from_raw_parts(writer.buffer.data.cast::<u8>(), ((writer.num + 7) >> 3) as usize).to_vec()
        }
    }
}
impl Drop for WriterPacket {
    fn drop(&mut self) {
        // SAFETY: the writer was initialized with the system allocator and is
        // no longer borrowed after this owner is dropped.
        unsafe {
            let writer = &mut *self.writer.get();
            let slots = hp2_core::array_storage::Hp2ArraySlots {
                data: ptr::addr_of_mut!(writer.buffer.data).cast(),
                num: ptr::addr_of_mut!(writer.buffer.num),
                max: ptr::addr_of_mut!(writer.buffer.max),
            };
            let _ = hp2_core::array_storage::release(slots, hp2_core::array_storage::system_allocator());
        }
    }
}
impl ArchiveMapping for WriterPacket {
    fn load_object(&self, _: i32) -> RuntimeResult<*mut UObject> { Err(unexpected_mapping()) }
    fn save_object(&self, _: *mut UObject) -> RuntimeResult<i32> { Err(unexpected_mapping()) }
    fn load_name(&self, _: i32) -> RuntimeResult<i32> { Err(unexpected_mapping()) }
    fn save_name(&self, _: i32) -> RuntimeResult<i32> { Err(unexpected_mapping()) }
    fn visit_object(&self, _: *mut *mut UObject) -> RuntimeResult<()> { Err(unexpected_mapping()) }
    fn visit_name(&self, _: i32) -> RuntimeResult<()> { Err(unexpected_mapping()) }
}
impl NetworkWriter for WriterPacket {
    unsafe fn write_raw(&self, input: *const u8, count: usize) -> RuntimeResult<()> {
        let count = i32::try_from(count).map_err(|_| RuntimeError::new("test writer length exceeds FBitWriter"))?;
        // SAFETY: the writer and the caller's input span remain live.
        unsafe { bits::writer_serialize(self.ptr(), input, count); }
        Ok(())
    }
    fn write_bounded(&self, value: u32, maximum: u32) -> RuntimeResult<()> {
        // SAFETY: the real writer publishes finite-capacity overflow in ar_is_error.
        unsafe {
            bits::writer_write_int_with_checker(
                self.ptr(), value, maximum,
                hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.slow_checks,
                |expression, line| {
                    let expression = std::ffi::CStr::from_bytes_with_nul(expression)
                        .map_err(|_| RuntimeError::new("invalid test writer assertion"))?;
                    Err(RuntimeError::new(&format!("{} (UnBits.cpp:{line})", expression.to_string_lossy())))
                },
            )
        }
    }
    unsafe fn write_bits(&self, input: *const u8, bit_count: u32) -> RuntimeResult<()> {
        let count = i32::try_from(bit_count).map_err(|_| RuntimeError::new("test writer bit count exceeds FBitWriter"))?;
        // SAFETY: the real FBitWriter consumes exactly the requested bits.
        unsafe { bits::writer_serialize_bits(self.ptr(), input, count); }
        Ok(())
    }
    fn position_bits(&self) -> usize { unsafe { (*self.ptr()).num.max(0) as usize } }
    fn total_bits(&self) -> usize { unsafe { (*self.ptr()).max.max(0) as usize } }
    unsafe fn write_object(&self, _: *mut *mut UObject, _: *mut UClass) -> RuntimeResult<i32> {
        Err(unexpected_mapping())
    }
    unsafe fn write_name(&self, _: *mut i32) -> RuntimeResult<()> { Err(unexpected_mapping()) }
}

fn unexpected_mapping() -> RuntimeError {
    RuntimeError::new("This scalar packet contains no mapped object or name references")
}
impl ArchiveMapping for Packet<'_> {
    fn load_object(&self, _: i32) -> RuntimeResult<*mut UObject> { Err(unexpected_mapping()) }
    fn save_object(&self, _: *mut UObject) -> RuntimeResult<i32> { Err(unexpected_mapping()) }
    fn load_name(&self, _: i32) -> RuntimeResult<i32> { Err(unexpected_mapping()) }
    fn save_name(&self, _: i32) -> RuntimeResult<i32> { Err(unexpected_mapping()) }
    fn visit_object(&self, _: *mut *mut UObject) -> RuntimeResult<()> { Err(unexpected_mapping()) }
    fn visit_name(&self, _: i32) -> RuntimeResult<()> { Err(unexpected_mapping()) }
}
impl NetworkReader for Packet<'_> {
    unsafe fn read_raw(&self, output: *mut u8, count: usize) -> RuntimeResult<()> {
        // SAFETY: the reader borrows its packet and output is the caller's span.
        unsafe { bits::reader_serialize(self.reader.get(), output, count as i32); }
        Ok(())
    }
    fn read_bounded(&self, value: &mut u32, maximum: u32) -> RuntimeResult<()> {
        // SAFETY: no mutable archive borrow survives this direct kernel call.
        unsafe { bits::reader_serialize_int(self.reader.get(), value, maximum); }
        Ok(())
    }
    unsafe fn read_bits(&self, output: *mut u8, count: u32) -> RuntimeResult<()> {
        unsafe { bits::reader_serialize_bits(self.reader.get(), output, count as i32); }
        Ok(())
    }
    fn position_bits(&self) -> usize { unsafe { (*self.reader.get()).pos as usize } }
    fn total_bits(&self) -> usize { unsafe { (*self.reader.get()).num as usize } }
    unsafe fn read_object(&self, _: *mut *mut UObject, _: *mut UClass) -> RuntimeResult<i32> {
        Err(unexpected_mapping())
    }
    unsafe fn read_name(&self, _: *mut i32) -> RuntimeResult<()> { Err(unexpected_mapping()) }
}
fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }
unsafe fn property(access: CoreAccess<'_>, name: &str) -> *mut UProperty {
    unsafe {
        access.construct_object(
            access.native_class(name).expect("canonical Core property class"),
            access.native_class("Core.Object").expect("property's Struct-compatible outer").cast(),
            0, 0, ptr::null_mut(),
        )
    }.expect("construct actual canonical property").cast()
}

// Also called by the standalone runtime consumer, rather than relying on a test
// runner alone to prove the canonical property/archive execution path.
pub fn exercise_unaligned_network_archive() {
    let artifacts = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"));
    assert!(artifacts.is_dir());
    let root = artifacts.join(format!("network-archive-{}", std::process::id()));
    std::fs::create_dir_all(root.join("System")).unwrap();
    std::fs::write(root.join("System/Default.ini"), "").unwrap();
    std::fs::write(root.join("System/DefUser.ini"), "").unwrap();
    let owner = CoreOwner::new(CoreOptions {
        data_root: root.clone(), system_ini: root.join("System/Default.ini"),
        user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(), output, log_file: None,
    }).expect("start real canonical Core without game data");
    let access = owner.access();
    // Independently packed, little-bit-endian wire: prefix=5 (3 bits),
    // int=0x12345678, bool=1, float=1.5, raw=ab cd ef, bounded=5 (max=7).
    let wire = [0xc5, 0xb3, 0xa2, 0x91, 0x08, 0x00, 0x00, 0xfc, 0xb3, 0xda, 0xfc, 0x5e];
    for bit_count in [95, 94] {
        let packet = Packet::new(&wire, bit_count);
        let archive = ObjectArchive::network_loading(&packet, ArchiveFlags::new(79, 0, false), &packet);
        let mut prefix = [0];
        archive.serialize_bits(&mut prefix, 3).unwrap();
        assert_eq!(prefix, [5]);
        unsafe {
            let integer = property(access, "IntProperty");
            let mut value = 0i32;
            access.property_net_serialize(integer, ptr::null_mut(), &archive, ptr::from_mut(&mut value).cast(), ptr::null_mut(), 0).unwrap();
            assert_eq!(value, 0x12345678);
            let boolean = property(access, "BoolProperty").cast::<UBoolProperty>();
            (*boolean).bit_mask = 8;
            let mut flags = 0x20u32;
            access.property_net_serialize(boolean.cast(), ptr::null_mut(), &archive, ptr::from_mut(&mut flags).cast(), ptr::null_mut(), 0).unwrap();
            assert_eq!(flags, 0x28, "one net bit changes only the property's mask");
            let float = property(access, "FloatProperty");
            let mut value = 0.0f32;
            access.property_net_serialize(float, ptr::null_mut(), &archive, ptr::from_mut(&mut value).cast(), ptr::null_mut(), 0).unwrap();
            assert_eq!(value.to_bits(), 1.5f32.to_bits());
            let mut raw = MaybeUninit::<[u8; 3]>::uninit();
            archive.read_raw(raw.as_mut_ptr().cast(), 3).unwrap();
            assert_eq!(raw.assume_init(), [0xab, 0xcd, 0xef]);
        }
        let mut bounded = 0;
        archive.serialize_bounded(&mut bounded, 7).unwrap();
        if bit_count == 95 {
            assert_eq!(bounded, 5);
            assert_eq!(packet.position_bits(), 95);
            assert_eq!(unsafe { (*packet.reader.get()).ar_is_error }, 0);
        } else {
            assert_eq!(bounded, 1, "bounded overflow retains its decoded partial value");
            assert_eq!(packet.position_bits(), 94);
            assert_eq!(unsafe { (*packet.reader.get()).ar_is_error }, 1);
        }
    }
    // Real FBitWriter twin of the independently packed 95-bit wire. The same
    // canonical Int/Bool/Float property policies and unaligned prefix/raw/
    // bounded operations must reproduce those exact bytes.
    let packet = WriterPacket::new(96);
    let archive = ObjectArchive::network_saving(&packet, ArchiveFlags::new(79, 0, false), &packet);
    let mut prefix = [5u8];
    archive.serialize_bits(&mut prefix, 3).unwrap();
    unsafe {
        let integer = property(access, "IntProperty");
        let mut value = 0x12345678i32;
        access.property_net_serialize(integer, ptr::null_mut(), &archive, ptr::from_mut(&mut value).cast(), ptr::null_mut(), 0).unwrap();
        let boolean = property(access, "BoolProperty").cast::<UBoolProperty>();
        (*boolean).bit_mask = 8;
        let mut flags = 0x28u32;
        access.property_net_serialize(boolean.cast(), ptr::null_mut(), &archive, ptr::from_mut(&mut flags).cast(), ptr::null_mut(), 0).unwrap();
        let float = property(access, "FloatProperty");
        let mut value = 1.5f32;
        access.property_net_serialize(float, ptr::null_mut(), &archive, ptr::from_mut(&mut value).cast(), ptr::null_mut(), 0).unwrap();
        archive.write(&[0xabu8, 0xcd, 0xef]).unwrap();
    }
    let mut bounded = 5u32;
    archive.serialize_bounded(&mut bounded, 7).unwrap();
    assert_eq!(packet.position_bits(), 95);
    assert_eq!(packet.written(), wire, "real FBitWriter must reproduce the canonical wire");
    assert_eq!(unsafe { (*packet.ptr()).ar_is_error }, 0);
    drop(archive);

    // Finite capacity: the real writer publishes overflow in ar_is_error and
    // leaves the cursor unchanged rather than silently truncating.
    let overflow = WriterPacket::new(8);
    let archive = ObjectArchive::network_saving(&overflow, ArchiveFlags::new(79, 0, false), &overflow);
    archive.write(&[0xffu8, 0xff]).unwrap();
    assert_eq!(overflow.position_bits(), 0);
    assert_eq!(unsafe { (*overflow.ptr()).ar_is_error }, 1, "capacity overflow must be published");
    drop(archive);

    owner.shutdown().expect("release canonical properties and Core state");
    std::fs::write(root.join("report.txt"), "data_profile=data-none\nunaligned_canonical_int_bool_float_properties=ok\nraw_uninitialized_byte_read=ok\nbounded_partial_overflow_and_cursor=ok\ncanonical_writer_unaligned_95bit=ok\nwriter_capacity_overflow_error=ok\nshutdown=ok\n").unwrap();
}

#[test]
fn unaligned_network_properties_and_truncated_bounded_values() {
    exercise_unaligned_network_archive();
}
