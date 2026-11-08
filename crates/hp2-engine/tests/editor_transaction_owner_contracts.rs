use hp2_abi::engine::generated::{TArrayRaw, UObject, UObjectProperty, UTextBuffer};
use hp2_core::{
    core_access::{CoreAccess, CoreOptions, RuntimeError, RuntimeResult},
    object_archive::{ArchiveFlags, ArchiveMapping, ObjectArchive},
};
use hp2_engine::{EngineOwner, editor_transaction_owner as transactions};
use std::{cell::RefCell, fs, path::PathBuf, ptr};
fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> {
    Ok(())
}
fn wide(text: &str) -> Vec<u32> {
    text.chars().map(u32::from).chain([0]).collect()
}
unsafe fn construct(access: CoreAccess<'_>, class: &str) -> *mut UObject {
    unsafe {
        access.construct_object(
            access.native_class(class).unwrap(),
            access.transient_package(),
            0,
            0,
            ptr::null_mut(),
        )
    }
    .unwrap()
}
unsafe fn text(value: *const TArrayRaw) -> String {
    unsafe { hp2_core::native_text::code_units(hp2_core::native_text::fstring_units(value)) }
        .iter()
        .map(|&c| char::from_u32(c).unwrap())
        .collect()
}
#[derive(Default)]
struct References {
    objects: RefCell<Vec<*mut UObject>>,
    fail_on: Option<*mut UObject>,
}
impl ArchiveMapping for References {
    fn load_object(&self, _: i32) -> RuntimeResult<*mut UObject> {
        Err(RuntimeError::new(
            "Reference collector has no package indices",
        ))
    }
    fn save_object(&self, _: *mut UObject) -> RuntimeResult<i32> {
        Err(RuntimeError::new(
            "Reference collector has no package indices",
        ))
    }
    fn load_name(&self, _: i32) -> RuntimeResult<i32> {
        Err(RuntimeError::new(
            "Reference collector has no package names",
        ))
    }
    fn save_name(&self, _: i32) -> RuntimeResult<i32> {
        Err(RuntimeError::new(
            "Reference collector has no package names",
        ))
    }
    fn visit_name(&self, _: i32) -> RuntimeResult<()> {
        Ok(())
    }
    fn visit_object(&self, slot: *mut *mut UObject) -> RuntimeResult<()> {
        let object = unsafe { slot.read_unaligned() };
        if self.fail_on == Some(object) {
            return Err(RuntimeError::new("Reference visitor rejected object"));
        }
        self.objects.borrow_mut().push(object);
        Ok(())
    }
}
fn flags() -> ArchiveFlags {
    let c = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS;
    ArchiveFlags::new(c.version as u16, c.licensee as u16, false)
}
#[test]
fn real_snapshots_restore_owned_strings_visit_saved_references_and_keep_pseudo_objects_unpublished()
{
    let root = PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("artifacts"))
        .join(format!("canonical-transactions-{}", std::process::id()));
    fs::create_dir_all(root.join("System")).unwrap();
    fs::write(root.join("System/Default.ini"), "").unwrap();
    fs::write(root.join("System/DefUser.ini"), "").unwrap();
    let owner = EngineOwner::new(CoreOptions {
        data_root: root.clone(),
        system_ini: root.join("System/Default.ini"),
        user_ini: root.join("System/DefUser.ini"),
        command_line: Vec::new(),
        output,
        log_file: Some(fs::File::create(root.join("runtime.log")).unwrap()),
    })
    .unwrap();
    let access = owner.access();
    unsafe {
        let text_buffer = construct(access, "Core.TextBuffer").cast::<UTextBuffer>();
        access
            .assign_fstring(
                ptr::addr_of_mut!((*text_buffer).text),
                &wide("saved Ω transaction text"),
            )
            .unwrap();
        (*text_buffer).pos = 7;
        (*text_buffer).top = 3;
        let saved_class = access
            .native_class("Engine.Actor")
            .unwrap()
            .cast::<UObject>();
        let property = access.construct_object(
            access.native_class("Core.ObjectProperty").unwrap(),
            saved_class,
            0,
            0,
            ptr::null_mut(),
        ).unwrap().cast::<UObjectProperty>();
        let current_class = access
            .native_class("Engine.Level")
            .unwrap()
            .cast::<UObject>();
        (*property).property_class = saved_class.cast();
        let transaction = transactions::create_internal_transaction().unwrap();
        transaction.save_object(access, text_buffer.cast()).unwrap();
        transaction.save_object(access, property.cast()).unwrap();
        access
            .assign_fstring(ptr::addr_of_mut!((*text_buffer).text), &wide("changed"))
            .unwrap();
        (*text_buffer).pos = 90;
        (*text_buffer).top = 91;
        (*property).property_class = current_class.cast();
        let references = References::default();
        let archive = ObjectArchive::reference(flags(), &references);
        let count = access.object_count();
        transaction.serialize(access, &archive).unwrap();
        assert_eq!(
            access.object_count(),
            count,
            "temporary snapshot objects never enter TObjectIterator's object array"
        );
        assert!(
            references.objects.borrow().contains(&saved_class),
            "reference GC retains the class in the saved snapshot"
        );
        assert!(
            !references.objects.borrow().contains(&current_class),
            "reference GC does not substitute the object's changed live value"
        );
        assert_eq!(
            text(ptr::addr_of!((*text_buffer).text)),
            "changed",
            "reference traversal does not restore the live object"
        );
        transaction.apply(access).unwrap();
        assert_eq!(
            text(ptr::addr_of!((*text_buffer).text)),
            "saved Ω transaction text"
        );
        assert_eq!(((*text_buffer).pos, (*text_buffer).top), (7, 3));
        assert_eq!((*property).property_class, saved_class.cast());

        let failing = References {
            objects: RefCell::new(Vec::new()),
            fail_on: Some(saved_class),
        };
        let failing_archive = ObjectArchive::reference(flags(), &failing);
        assert!(transaction.serialize(access, &failing_archive).is_err());
        assert_eq!(
            access.object_count(),
            count,
            "failure also destroys pseudo-object lifetimes without publication"
        );
        // A second traversal after failure exercises the scratch-mark unwind.
        transaction.serialize(access, &archive).unwrap();

        let buffer = transactions::create_buffer(access, 1024 * 1024).unwrap();
        assert!(!transactions::can_undo(access, buffer, ptr::null_mut()).unwrap());
        assert!(!transactions::can_redo(access, buffer, ptr::null_mut()).unwrap());
        transactions::begin(access, buffer, &wide("canonical object modification")).unwrap();
        (*text_buffer.cast::<UObject>()).object_flags |= 1;
        access.modify_object(text_buffer.cast()).unwrap();
        access
            .assign_fstring(
                ptr::addr_of_mut!((*text_buffer).text),
                &wide("history value"),
            )
            .unwrap();
        transactions::end(access, buffer).unwrap();
        assert!(transactions::can_undo(access, buffer, ptr::null_mut()).unwrap());
        let history_refs = References::default();
        let history_archive = ObjectArchive::reference(flags(), &history_refs);
        access.serialize_object(buffer, &history_archive).unwrap();
        assert!(
            history_refs.objects.borrow().contains(&text_buffer.cast()),
            "real GUndo adapter captures canonical modified objects"
        );
        transactions::reset(access, buffer, &wide("test reset")).unwrap();
        assert!(!transactions::can_undo(access, buffer, ptr::null_mut()).unwrap());
        access.delete_object(buffer).unwrap();
    }
    owner.shutdown().unwrap();
    fs::write(root.join("contract.txt"),"data_profile=data-none\nsnapshots=real-Core-objects\nreferences=saved-not-live\npseudo_objects=unregistered-GMem\nstrings=owned-roundtrip\nhistory=real-GUndo-adapter\neditor_ui=not-created\n").unwrap();
}
