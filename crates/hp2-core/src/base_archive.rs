//! Base FArchive policy over synchronous checked native leaves.
//! The owner and all potentially faulting memory access remain native. Callback
//! tables are copied; no archive reference or lock survives a callback.
use std::ffi::c_void;

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2BaseArchiveOp {
    SerializeBits = 0,
    AtEnd = 1,
    HostWchar = 2,
    BigEndian = 3,
}

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2BaseArchiveLeaf {
    Serialize = 0,
    Loading = 1,
    Tell = 2,
    TotalSize = 3,
    MaskByte = 4,
    ReadHostWchar = 5,
    WriteHostWchar = 6,
    SerializeWire = 7,
    Persistent = 8,
    SerializeByte = 9,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2BaseArchiveIo {
    pub data: *mut c_void,
    pub length: i32,
    pub value: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2BaseArchiveServices {
    pub context: *mut c_void,
    pub invoke:
        Option<unsafe extern "C" fn(*mut c_void, Hp2BaseArchiveLeaf, *mut Hp2BaseArchiveIo) -> i32>,
}

impl Hp2BaseArchiveServices {
    fn call(self, leaf: Hp2BaseArchiveLeaf, io: &mut Hp2BaseArchiveIo) -> Result<(), ()> {
        let invoke = self.invoke.ok_or(())?;
        if unsafe { invoke(self.context, leaf, io) } == 0 {
            Ok(())
        } else {
            Err(())
        }
    }
}

pub fn close(error: i32) -> i32 {
    i32::from(error == 0)
}
pub fn net_ver(version: i32) -> i32 {
    version & 0x7fffffff
}
pub fn is_net(version: i32) -> i32 {
    i32::from(version < 0)
}

/// Shared base-archive bit policy. Unlike FBitReader/FBitWriter this uses
/// whole-byte serialization, followed by the historical loading mask.
///
/// The callback must validate its memory operand, including the following
/// byte selected by aligned and zero-bit lengths.
pub fn serialize_bits_with<E>(
    data: *mut c_void,
    length: i32,
    mut invoke: impl FnMut(Hp2BaseArchiveLeaf, &mut Hp2BaseArchiveIo) -> Result<(), E>,
) -> Result<(), E> {
    let mut io = Hp2BaseArchiveIo {
        data,
        length: length.wrapping_add(7) / 8,
        value: 0,
    };
    invoke(Hp2BaseArchiveLeaf::Serialize, &mut io)?;
    invoke(Hp2BaseArchiveLeaf::Loading, &mut io)?;
    if io.value != 0 {
        io.length = length / 8;
        io.value = (1 << (length & 7)) - 1;
        invoke(Hp2BaseArchiveLeaf::MaskByte, &mut io)?;
    }
    Ok(())
}

/// # Safety
/// The native owner, callbacks and operand stay live throughout this invocation.
/// Native callbacks catch exceptions and signals before returning to Rust.
pub unsafe fn run(
    services: Hp2BaseArchiveServices,
    op: Hp2BaseArchiveOp,
    data: *mut c_void,
    length: i32,
) -> Result<i32, ()> {
    let mut io = Hp2BaseArchiveIo {
        data,
        length,
        value: 0,
    };
    match op {
        Hp2BaseArchiveOp::SerializeBits => {
            serialize_bits_with(data, length, |leaf, io| services.call(leaf, io))?;
            Ok(0)
        }
        Hp2BaseArchiveOp::AtEnd => {
            services.call(Hp2BaseArchiveLeaf::Tell, &mut io)?;
            let position = io.value;
            if position == -1 {
                return Ok(0);
            }
            services.call(Hp2BaseArchiveLeaf::TotalSize, &mut io)?;
            Ok(i32::from(position >= io.value))
        }
        Hp2BaseArchiveOp::HostWchar => {
            services.call(Hp2BaseArchiveLeaf::Loading, &mut io)?;
            let mut wire = 0u16;
            if io.value == 0 {
                services.call(Hp2BaseArchiveLeaf::ReadHostWchar, &mut io)?;
                wire = if (0..=0xffff).contains(&io.value) {
                    io.value as u16
                } else {
                    0xfffd
                };
            }
            io.data = (&raw mut wire).cast();
            services.call(Hp2BaseArchiveLeaf::SerializeWire, &mut io)?;
            services.call(Hp2BaseArchiveLeaf::Loading, &mut io)?;
            if io.value != 0 {
                io.data = data;
                io.value = i32::from(wire);
                services.call(Hp2BaseArchiveLeaf::WriteHostWchar, &mut io)?;
            }
            Ok(0)
        }
        Hp2BaseArchiveOp::BigEndian => {
            // Dormant on the supported little-endian host; retained without a
            // Rust round trip on that host's direct Serialize dispatch path.
            services.call(Hp2BaseArchiveLeaf::Persistent, &mut io)?;
            if io.value != 0 {
                let mut index = length.wrapping_sub(1);
                while index >= 0 {
                    io.length = index;
                    services.call(Hp2BaseArchiveLeaf::SerializeByte, &mut io)?;
                    index -= 1;
                }
            } else {
                services.call(Hp2BaseArchiveLeaf::Serialize, &mut io)?;
            }
            Ok(0)
        }
    }
}
