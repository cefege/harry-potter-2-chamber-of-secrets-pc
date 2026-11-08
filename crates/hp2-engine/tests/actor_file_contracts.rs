use hp2_engine::actor_files::{self, FileOperation, Hp2ActorFileServices};
use std::ffi::c_void;

#[derive(Default, Debug, PartialEq)]
struct SaveInfo {
    counts: [i32; 4],
    level: Vec<u8>,
}

#[derive(Default)]
struct Archive {
    bytes: Vec<u8>,
    position: usize,
    writing: bool,
    missing: bool,
    opened: usize,
    closed: usize,
    fail_field: Option<i32>,
}

unsafe extern "C" fn open(
    context: *mut c_void,
    _path: *const c_void,
    writing: i32,
    _save_slot: i32,
    out: *mut *mut c_void,
) -> i32 {
    let archive = unsafe { &mut *context.cast::<Archive>() };
    archive.opened += 1;
    archive.position = 0;
    archive.writing = writing != 0;
    unsafe {
        *out = if archive.missing {
            std::ptr::null_mut()
        } else {
            context
        }
    };
    0
}

unsafe extern "C" fn serialize_object(
    _context: *mut c_void,
    archive: *mut c_void,
    object: *mut c_void,
) -> i32 {
    let archive = unsafe { &mut *archive.cast::<Archive>() };
    let value = unsafe { &mut *object.cast::<i32>() };
    if archive.writing {
        archive.bytes.extend(value.to_le_bytes());
    } else {
        *value = i32::from_le_bytes(archive.bytes[..4].try_into().unwrap());
    }
    0
}

unsafe extern "C" fn serialize_info_field(
    _context: *mut c_void,
    archive: *mut c_void,
    object: *mut c_void,
    field: i32,
) -> i32 {
    let archive = unsafe { &mut *archive.cast::<Archive>() };
    if archive.fail_field == Some(field) {
        return 17;
    }
    let info = unsafe { &mut *object.cast::<SaveInfo>() };
    if archive.writing {
        if field < 4 {
            archive
                .bytes
                .extend(info.counts[field as usize].to_le_bytes());
        } else {
            archive.bytes.extend(&info.level);
        }
    } else if field < 4 {
        let end = archive.position + 4;
        info.counts[field as usize] =
            i32::from_le_bytes(archive.bytes[archive.position..end].try_into().unwrap());
        archive.position = end;
    } else {
        info.level = archive.bytes[archive.position..].to_vec();
    }
    0
}

unsafe extern "C" fn close(context: *mut c_void, _archive: *mut c_void) -> i32 {
    unsafe { (*context.cast::<Archive>()).closed += 1 };
    0
}

fn services(archive: &mut Archive) -> Hp2ActorFileServices {
    Hp2ActorFileServices {
        context: (archive as *mut Archive).cast(),
        open,
        serialize_object,
        serialize_info_field,
        close,
    }
}

#[test]
fn missing_archive_and_null_load_info_return_false_without_serializing() {
    let mut archive = Archive {
        missing: true,
        ..Archive::default()
    };
    let services = services(&mut archive);
    assert_eq!(
        unsafe {
            actor_files::transfer_file(
                &services,
                FileOperation::LoadInfo,
                std::ptr::null(),
                std::ptr::null_mut(),
            )
        },
        Ok(false)
    );
    assert_eq!(archive.opened, 0);
    let mut object = 42_i32;
    assert_eq!(
        unsafe {
            actor_files::transfer_file(
                &services,
                FileOperation::LoadObject,
                std::ptr::null(),
                (&mut object as *mut i32).cast(),
            )
        },
        Ok(false)
    );
    assert_eq!(object, 42);
    assert_eq!(archive.closed, 0);
}

#[test]
fn field_failure_propagates_without_serializing_later_fields_or_reporting_success() {
    let mut archive = Archive {
        fail_field: Some(2),
        ..Archive::default()
    };
    let services = services(&mut archive);
    let mut info = SaveInfo {
        counts: [7, 8, 9, 10],
        level: b"not written".to_vec(),
    };
    assert_eq!(
        unsafe {
            actor_files::transfer_file(
                &services,
                FileOperation::SaveInfo,
                std::ptr::null(),
                (&mut info as *mut SaveInfo).cast(),
            )
        },
        Err(17)
    );
    assert_eq!(archive.bytes, b"\x07\0\0\0\x08\0\0\0");
    assert_eq!(archive.closed, 0);
}
