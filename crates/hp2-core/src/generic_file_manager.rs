//! Generic file-manager policy. Native leaves retain virtual dispatch, lexical
//! FString/TArray lifetimes, locale operations and the original guard scopes.
use crate::ansi::Tchar;
use std::ffi::c_void;
use std::mem::MaybeUninit;
use std::ptr::{null, null_mut};

/// Checked operations on one invocation-local native context.
///
/// cbindgen:prefix-with-name
#[repr(u32)]
#[derive(Clone, Copy)]
pub enum Hp2GenericFileLeaf {
    CreateReader,
    SourceSize,
    DestroySource,
    Progress,
    CreateWriter,
    ReadSource,
    SourceError,
    WriteDestination,
    DestinationError,
    CloseDestination,
    DestroyDestination,
    DeleteDestination,
    CloseSource,
    IsDrive,
    MakeDirectory,
    RunDeleteScope,
    ListCount,
    DeleteListedFile,
    RefreshDirectories,
    DeleteListedDirectory,
    DeleteDirectory,
    CopyForMove,
    DeleteMoveSource,
    StatTime,
    StatModified,
    CompareEmpty,
    CompareBackslash,
    CompareDoubleBackslash,
    UpperFirst,
    LowerFirst,
}

/// Native callback ABI. `call` writes its output only on success and returns
/// zero or the first nonzero failure status. Each native callback contains its
/// own exception/signal boundary. Rust stops immediately on failure, without
/// cleanup calls; native lexical scopes unwind after the Rust phase returns.
/// The table is copied before callbacks, and the opaque native owner is never
/// borrowed by Rust. Reentry creates an independent invocation context.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2GenericFileServices {
    pub context: *mut c_void,
    pub call:
        unsafe extern "C" fn(*mut c_void, Hp2GenericFileLeaf, *const c_void, *mut c_void) -> i32,
    pub filewrite_no_replace: u32,
    pub filewrite_even_if_read_only: u32,
}

/// A synchronous Serialize call; the native archive must not retain `data`.
#[repr(C)]
pub struct Hp2GenericFileTransfer {
    pub data: *mut u8,
    pub count: i32,
}

impl Hp2GenericFileServices {
    unsafe fn value<T>(&self, leaf: Hp2GenericFileLeaf, input: *const c_void) -> Result<T, i32> {
        let mut output = MaybeUninit::<T>::uninit();
        let status = unsafe { (self.call)(self.context, leaf, input, output.as_mut_ptr().cast()) };
        if status == 0 {
            Ok(unsafe { output.assume_init() })
        } else {
            Err(status)
        }
    }

    unsafe fn int(&self, leaf: Hp2GenericFileLeaf) -> Result<i32, i32> {
        unsafe { self.value(leaf, null()) }
    }

    unsafe fn action(&self, leaf: Hp2GenericFileLeaf, input: *const c_void) -> Result<(), i32> {
        let status = unsafe { (self.call)(self.context, leaf, input, null_mut()) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
}

/// # Safety
/// The checked services own live native paths and virtual archive receivers.
pub unsafe fn file_size(services: Hp2GenericFileServices) -> Result<i32, i32> {
    use Hp2GenericFileLeaf as L;
    unsafe {
        if services.int(L::CreateReader)? == 0 {
            return Ok(-1);
        }
        let size = services.int(L::SourceSize)?;
        services.action(L::DestroySource, null())?;
        Ok(size)
    }
}

/// Preserve the original copier's success overwrite, short-circuit source
/// Close, and progress measured BEFORE incrementing the 4096-byte cursor.
/// `attributes` has deliberately never affected this generic implementation.
///
/// # Safety
/// Services contain checked virtual/archive/progress leaves and stable paths.
pub unsafe fn copy(
    services: Hp2GenericFileServices,
    replace_existing: i32,
    even_if_read_only: i32,
    has_progress: i32,
) -> Result<i32, i32> {
    use Hp2GenericFileLeaf as L;
    unsafe {
        let mut success = 0;
        if has_progress != 0 {
            services.action(L::Progress, (&0.0_f32 as *const f32).cast())?;
        }
        if services.int(L::CreateReader)? != 0 {
            let size = services.int(L::SourceSize)?;
            let flags = if replace_existing != 0 {
                0
            } else {
                services.filewrite_no_replace
            } | if even_if_read_only != 0 {
                services.filewrite_even_if_read_only
            } else {
                0
            };
            if services.value::<i32>(L::CreateWriter, (&flags as *const u32).cast())? != 0 {
                let mut buffer = MaybeUninit::<[u8; 4096]>::uninit();
                let mut total = 0_i32;
                while total < size {
                    let transfer = Hp2GenericFileTransfer {
                        data: buffer.as_mut_ptr().cast(),
                        count: (size - total).min(4096),
                    };
                    services.action(
                        L::ReadSource,
                        (&transfer as *const Hp2GenericFileTransfer).cast(),
                    )?;
                    if services.int(L::SourceError)? != 0 {
                        break;
                    }
                    services.action(
                        L::WriteDestination,
                        (&transfer as *const Hp2GenericFileTransfer).cast(),
                    )?;
                    if services.int(L::DestinationError)? != 0 {
                        break;
                    }
                    if has_progress != 0 {
                        let fraction = total as f32 / size as f32;
                        services.action(L::Progress, (&fraction as *const f32).cast())?;
                    }
                    // The native += sizeof(Buffer) performs unsigned addition
                    // before conversion back to INT, including its last step.
                    total = total.wrapping_add(4096);
                }
                success = services.int(L::CloseDestination)?;
                services.action(L::DestroyDestination, null())?;
                if success == 0 {
                    services.action(L::DeleteDestination, null())?;
                }
            }
            success = i32::from(success != 0 && services.int(L::CloseSource)? != 0);
            services.action(L::DestroySource, null())?;
        }
        if has_progress != 0 {
            services.action(L::Progress, (&1.0_f32 as *const f32).cast())?;
        }
        Ok(success)
    }
}

/// Traverse the original native zero-initialized TCHAR Full[256]. No new path
/// validation is imposed; as in the native implementation the caller provides
/// a path whose traversal and terminators fit the original fixed buffer.
///
/// # Safety
/// `path` is a live NUL-terminated native string. `full` is the original native
/// zero-filled 256-TCHAR local, and all writes made by this traversal fit it.
pub unsafe fn make_directory(
    services: Hp2GenericFileServices,
    mut path: *const Tchar,
    full: *mut Tchar,
    separator: Tchar,
) -> Result<i32, i32> {
    use Hp2GenericFileLeaf as L;
    unsafe {
        let mut cursor = full;
        let mut slash_count = 0_i32;
        let mut create_count = 0_i32;
        loop {
            if *path == separator || *path == 0 {
                let previous = slash_count;
                slash_count += 1;
                if previous > 0 && services.value::<i32>(L::IsDrive, full.cast())? == 0 {
                    *cursor = 0;
                    if services.value::<i32>(L::MakeDirectory, full.cast())? == 0 {
                        return Ok(0);
                    }
                    create_count += 1;
                }
            }
            if *path == 0 {
                break;
            }
            *cursor = *path;
            cursor = cursor.add(1);
            path = path.add(1);
        }
        Ok(i32::from(create_count != 0))
    }
}

/// # Safety
/// `path` is a valid native string and the services enter a synchronous native
/// scope containing the original directly constructed FString and TArray.
pub unsafe fn delete_directory(
    services: Hp2GenericFileServices,
    path: *const Tchar,
) -> Result<i32, i32> {
    unsafe {
        if crate::unix_text::tchar_length(path) == 0 {
            return Ok(0);
        }
        services.int(Hp2GenericFileLeaf::RunDeleteScope)
    }
}

/// Both traversal phases run inside one native scope. RefreshDirectories
/// assigns the second FindFiles result to the SAME TArray, preserving the first
/// snapshot until that native assignment and keeping Spec alive throughout.
///
/// # Safety
/// Services expose the native Spec/List locals for this synchronous phase only.
pub unsafe fn delete_directory_scope(services: Hp2GenericFileServices) -> Result<i32, i32> {
    use Hp2GenericFileLeaf as L;
    unsafe {
        let mut index = 0_i32;
        while index < services.int(L::ListCount)? {
            if services.value::<i32>(L::DeleteListedFile, (&index as *const i32).cast())? == 0 {
                return Ok(0);
            }
            index += 1;
        }
        services.action(L::RefreshDirectories, null())?;
        index = 0;
        while index < services.int(L::ListCount)? {
            if services.value::<i32>(L::DeleteListedDirectory, (&index as *const i32).cast())? == 0
            {
                return Ok(0);
            }
            index += 1;
        }
        services.int(L::DeleteDirectory)
    }
}

/// # Safety
/// Services dispatch to the original virtual Copy and Delete methods.
pub unsafe fn move_file(services: Hp2GenericFileServices) -> Result<i32, i32> {
    unsafe {
        if services.int(Hp2GenericFileLeaf::CopyForMove)? == 0 {
            return Ok(0);
        }
        services.action(Hp2GenericFileLeaf::DeleteMoveSource, null())?;
        Ok(1)
    }
}

/// # Safety
/// StatTime invokes actual native stat(appToAnsi(Filename), &buffer). The
/// original native buffer remains live until StatModified reads st_mtime.
pub unsafe fn get_global_time(services: Hp2GenericFileServices) -> Result<i64, i32> {
    unsafe {
        if services.int(Hp2GenericFileLeaf::StatTime)? != 0 {
            return Ok(0);
        }
        services.value(Hp2GenericFileLeaf::StatModified, null())
    }
}

/// The actual original unspecified setter is a no-op returning zero.
pub fn set_global_time() -> i32 {
    0
}

/// # Safety
/// `path` is a NUL-terminated native TCHAR string; locale comparison leaves use
/// this same string and run within the original native IsDrive guard.
pub unsafe fn is_drive(services: Hp2GenericFileServices, path: *const Tchar) -> Result<i32, i32> {
    use Hp2GenericFileLeaf as L;
    unsafe {
        if services.int(L::CompareEmpty)? == 0 {
            return Ok(1);
        }
        let upper = services.value::<Tchar>(L::UpperFirst, null())?;
        let lower = services.value::<Tchar>(L::LowerFirst, null())?;
        if upper != lower && *path.add(1) == b':' as Tchar && *path.add(2) == 0 {
            return Ok(1);
        }
        if services.int(L::CompareBackslash)? == 0 || services.int(L::CompareDoubleBackslash)? == 0
        {
            return Ok(1);
        }
        if *path == b'\\' as Tchar && *path.add(1) == b'\\' as Tchar {
            let mut cursor = path.add(2);
            let mut slashes = 0;
            while *cursor != 0 {
                if *cursor == b'\\' as Tchar {
                    slashes += 1;
                    if slashes == 2 {
                        return Ok(0);
                    }
                }
                cursor = cursor.add(1);
            }
            return Ok(1);
        }
        Ok(0)
    }
}
