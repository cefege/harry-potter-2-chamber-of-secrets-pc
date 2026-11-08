//! Built-in download codecs over checked, live native archive leaves.
//!
//! The archive and notification owners stay native. No owner reference or lock
//! crosses a callback: each operation resolves current native state again.
use std::ffi::c_void;

pub type CodecResult<T> = Result<T, ()>;

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hp2CodecKind {
    Bwt = 0,
    Rle = 1,
    Huffman = 2,
    Mtf = 3,
}

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hp2CodecLeaf {
    Read = 0,
    Write = 1,
    Tell = 2,
    TotalSize = 3,
    AtEnd = 4,
    Seek = 5,
    Progress = 6,
    Check = 7,
    BitWriterBegin = 8,
    BitReaderBegin = 9,
}

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hp2CodecFailure {
    BwtBlockLimit = 0,
    BwtRemaining = 1,
    RleRunCount = 2,
    HuffmanWriterError = 3,
    HuffmanWriterBits = 4,
    HuffmanReaderEnd = 5,
    MtfIndex = 6,
    ArrayIndex = 7,
    ArrayLength = 8,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2CodecIo {
    pub data: *mut c_void,
    pub length: i32,
    pub value: i32,
    pub progress: f32,
}

impl Default for Hp2CodecIo {
    fn default() -> Self {
        Self {
            data: std::ptr::null_mut(),
            length: 0,
            value: 0,
            progress: 0.0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2CodecServices {
    pub context: *mut c_void,
    pub invoke: unsafe extern "C" fn(*mut c_void, Hp2CodecLeaf, *mut Hp2CodecIo) -> i32,
    pub notify_present: i32,
}

/// An immutable callback table, not a borrowed archive or codec owner.
pub struct CodecIo<'a> {
    services: &'a Hp2CodecServices,
}

impl<'a> CodecIo<'a> {
    /// # Safety
    /// The native service context and callbacks remain valid for this invocation.
    pub unsafe fn new(services: &'a Hp2CodecServices) -> Self {
        Self { services }
    }

    fn call(&self, leaf: Hp2CodecLeaf, io: &mut Hp2CodecIo) -> CodecResult<()> {
        // SAFETY: established by new; this local record survives the callback.
        let status = unsafe { (self.services.invoke)(self.services.context, leaf, io) };
        if status == 0 { Ok(()) } else { Err(()) }
    }
    pub fn has_notify(&self) -> bool {
        self.services.notify_present != 0
    }

    /// # Safety
    /// The buffer meets the actual native archive's Serialize contract. Native
    /// callbacks may mutate it; do not borrow it exclusively across this call.
    pub unsafe fn read(&self, data: *mut u8, length: i32) -> CodecResult<()> {
        self.call(
            Hp2CodecLeaf::Read,
            &mut Hp2CodecIo {
                data: data.cast(),
                length,
                ..Default::default()
            },
        )
    }

    /// # Safety
    /// As read. Native Serialize accepts mutable data even on output; preserve
    /// mutations of the original local or buffer instead of serializing a copy.
    pub unsafe fn write(&self, data: *mut u8, length: i32) -> CodecResult<()> {
        self.call(
            Hp2CodecLeaf::Write,
            &mut Hp2CodecIo {
                data: data.cast(),
                length,
                ..Default::default()
            },
        )
    }

    fn integer(&self, leaf: Hp2CodecLeaf) -> CodecResult<i32> {
        let mut io = Hp2CodecIo::default();
        self.call(leaf, &mut io)?;
        Ok(io.value)
    }

    pub fn tell(&self) -> CodecResult<i32> {
        self.integer(Hp2CodecLeaf::Tell)
    }
    pub fn total_size(&self) -> CodecResult<i32> {
        self.integer(Hp2CodecLeaf::TotalSize)
    }
    pub fn at_end(&self) -> CodecResult<bool> {
        Ok(self.integer(Hp2CodecLeaf::AtEnd)? != 0)
    }

    pub fn seek(&self, position: i32) -> CodecResult<()> {
        self.call(
            Hp2CodecLeaf::Seek,
            &mut Hp2CodecIo {
                value: position,
                ..Default::default()
            },
        )
    }

    /// Native notification presence is resolved on every call. A null notifier
    /// returns true without synthesizing a notification.
    pub fn progress(&self, progress: f32) -> CodecResult<bool> {
        let mut io = Hp2CodecIo {
            progress,
            ..Default::default()
        };
        self.call(Hp2CodecLeaf::Progress, &mut io)?;
        Ok(io.value != 0)
    }

    pub fn check(&self, failure: Hp2CodecFailure, condition: bool) -> CodecResult<()> {
        self.check_value(failure, condition, 0)
    }

    pub fn check_value(
        &self,
        failure: Hp2CodecFailure,
        condition: bool,
        value: i32,
    ) -> CodecResult<()> {
        if condition {
            return Ok(());
        }
        self.call(
            Hp2CodecLeaf::Check,
            &mut Hp2CodecIo {
                length: failure as i32,
                value,
                ..Default::default()
            },
        )
    }

    /// Constructs a genuine typed native FBitWriter in the native invocation's
    /// stack scope; that scope destroys it after Rust returns or records a fault.
    pub fn bit_writer_begin(
        &self,
        bits: i32,
    ) -> CodecResult<*mut hp2_abi::engine::generated::FBitWriter> {
        let mut io = Hp2CodecIo {
            length: bits,
            ..Default::default()
        };
        self.call(Hp2CodecLeaf::BitWriterBegin, &mut io)?;
        Ok(io.data.cast())
    }

    /// # Safety
    /// Source satisfies the genuine native FBitReader constructor's contract.
    /// The returned pointer is live until the native invocation scope ends.
    pub unsafe fn bit_reader_begin(
        &self,
        data: *mut u8,
        bits: i32,
    ) -> CodecResult<*mut hp2_abi::engine::generated::FBitReader> {
        let mut io = Hp2CodecIo {
            data: data.cast(),
            length: bits,
            ..Default::default()
        };
        self.call(Hp2CodecLeaf::BitReaderBegin, &mut io)?;
        Ok(io.data.cast())
    }
}
