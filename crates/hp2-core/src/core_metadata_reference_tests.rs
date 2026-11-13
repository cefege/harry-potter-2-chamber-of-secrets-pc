//! Reference-archive operand decisions outside an active script event.
//!
//! The counter observes the guard's write decision, not physical memory writes.
//! The parent module must explicitly register this file as a `cfg(test)` child.
use super::*;
use crate::core_access::{CoreOptions, CoreOwner};
use crate::expression_archive::{ExpressionOps, Hp2ExprOperand};
use crate::name_owner::FindName;
use crate::object_archive::{ArchiveFlags, ArchiveMapping};
use std::cell::Cell;

fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }

// This archive visitor only changes its own object slot and the fixture's
// allocated operand bytes. It never changes a script header or runs an event.
struct Visitor {
    live: *mut u8,
    expected: *mut UObject,
    desired: Option<*mut UObject>,
    live_index: Option<i32>,
    fail: bool,
    objects: Cell<usize>,
    names: Cell<usize>,
    observed_name: Cell<i32>,
}

impl ArchiveMapping for Visitor {
    fn load_object(&self, _: i32) -> RuntimeResult<*mut UObject> { Err(RuntimeError::new("unexpected load")) }
    fn save_object(&self, _: *mut UObject) -> RuntimeResult<i32> { Err(RuntimeError::new("unexpected save")) }
    fn load_name(&self, _: i32) -> RuntimeResult<i32> { Err(RuntimeError::new("unexpected load")) }
    fn save_name(&self, _: i32) -> RuntimeResult<i32> { Err(RuntimeError::new("unexpected save")) }
    fn visit_object(&self, object_slot: *mut *mut UObject) -> RuntimeResult<()> {
        self.objects.set(self.objects.get() + 1);
        // SAFETY: ObjectArchive supplies the live local pointer slot for this
        // synchronous call. The fixture's four operand bytes remain allocated.
        unsafe {
            assert_eq!(object_slot.read(), self.expected, "visitor input");
            if let Some(index) = self.live_index { self.live.cast::<i32>().write_unaligned(index); }
            if let Some(object) = self.desired { object_slot.write(object); }
        }
        if self.fail { Err(RuntimeError::new("fixture visitor failure")) } else { Ok(()) }
    }
    fn visit_name(&self, name: i32) -> RuntimeResult<()> {
        self.names.set(self.names.get() + 1);
        self.observed_name.set(name);
        Ok(())
    }
}

// Reset before shutdown, including during assertion unwinding: arbitrary operand
// bytes must not remain attached when Core traverses or destroys the structure.
struct ScriptCleanup<'a> {
    access: CoreAccess<'a>,
    structure: *mut UStruct,
}
impl Drop for ScriptCleanup<'_> {
    fn drop(&mut self) {
        // SAFETY: the owner and canonical structure outlive this guard, and no
        // archive visitor or context is active when the guard is dropped.
        unsafe { DirectMetadataOps { access: self.access }.reset_script(self.structure.cast(), 0) }
            .expect("detach fixture script before Core teardown");
    }
}

fn index(access: CoreAccess<'_>, object: *mut UObject) -> i32 {
    assert!(!object.is_null());
    // SAFETY: each input is a live canonical indexed Core record from the owner.
    let index = unsafe { ptr::addr_of!((*object).index).read_unaligned() };
    assert_eq!(access.object_at(index), object, "fixture object must be indexed");
    index
}

#[test]
fn reference_transfer_omits_only_original_desired_and_live_matches() {
    let root = std::path::PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
        .join(format!("reference-operands-{}", std::process::id()));
    std::fs::create_dir_all(root.join("System")).unwrap();
    std::fs::write(root.join("System/Default.ini"), "").unwrap();
    std::fs::write(root.join("System/DefUser.ini"), "").unwrap();
    let owner = CoreOwner::new(CoreOptions {
        data_root: root.clone(), system_ini: root.join("System/Default.ini"),
        user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(), output, log_file: None,
    }).unwrap();
    {
        let access = owner.access();
        // SAFETY: the owner provides canonical class and outer records and owns
        // every constructed record for the duration of this fixture.
        let (structure, property) = unsafe {
            let structure = access.construct_object(access.native_class("Core.Struct").unwrap(),
                access.transient_package(), 0, 0, ptr::null_mut()).unwrap().cast::<UStruct>();
            let property = access.construct_object(access.native_class("Core.IntProperty").unwrap(),
                structure.cast(), 0, 0, ptr::null_mut()).unwrap();
            (structure, property)
        };
        let cleanup = ScriptCleanup { access, structure };
        // SAFETY: the structure is live, and reset_script owns the script allocation.
        unsafe { DirectMetadataOps { access }.reset_script(structure.cast(), 8).unwrap(); }
        // SAFETY: reset_script initialized eight operand bytes and retains them
        // until cleanup. No active event or reference into this array exists.
        let live = unsafe { ptr::addr_of!((*structure).script.data).read_unaligned().cast::<u8>() };
        assert!(!live.is_null());
        let class = access.native_class("Core.Struct").unwrap().cast::<UObject>();
        let object = access.transient_package().cast::<UObject>();
        let struct_object = structure.cast::<UObject>();
        let property = property.cast::<UObject>();
        let object_index = index(access, object);
        let other = struct_object;
        let third = class;
        let other_index = index(access, other);
        let third_index = index(access, third);
        let name = access.intern_str("ReferenceOperandFixture", FindName::Add).unwrap();
        assert_ne!(name, 0);

        // Each case starts with a fresh initialized operand and counter. The
        // visitor may synchronously alter only the designated four-byte slot.
        let run = |kind: Hp2ExprOperand, input: &[u8], expected: *mut UObject,
                   desired: Option<*mut UObject>, live_index: Option<i32>, fail: bool,
                   expected_output: &[u8], expected_none: bool, decisions: usize,
                   object_visits: usize, name_visits: usize, expected_name: i32| {
            assert!(input.len() <= 8 && expected_output.len() == input.len());
            // SAFETY: input and the fixture allocation do not overlap; both
            // spans are valid for input.len() bytes and no event is active.
            unsafe { ptr::copy_nonoverlapping(input.as_ptr(), live, input.len()); }
            let visitor = Visitor { live, expected, desired, live_index, fail,
                objects: Cell::new(0), names: Cell::new(0), observed_name: Cell::new(-1) };
            let archive = ObjectArchive::reference(ArchiveFlags::new(79, 0, false), &visitor);
            let context = DirectExpressionContext { access, structure, archive: &archive,
                debug_lookahead: Cell::new(false) };
            let mut cursor = 0;
            REFERENCE_OPERAND_WRITE_DECISIONS.with(|count| count.set(0));
            // SAFETY: cursor and the allocated script operand remain live for
            // transfer and the visitor does not resize or replace the script.
            let result = unsafe { context.transfer(&mut cursor, kind) };
            if fail {
                let error = result.expect_err("visitor failure must propagate");
                assert_eq!(error.to_string(), "fixture visitor failure");
                assert_eq!(cursor, 0, "error must not advance the expression cursor");
            } else {
                assert_eq!(result.unwrap(), expected_none, "name-none result: {kind:?}");
                assert_eq!(cursor, input.len() as i32, "cursor width: {kind:?}");
            }
            assert_eq!(REFERENCE_OPERAND_WRITE_DECISIONS.with(Cell::get), decisions, "guard decision: {kind:?}");
            assert_eq!(visitor.objects.get(), object_visits, "object visits: {kind:?}");
            assert_eq!(visitor.names.get(), name_visits, "name visits: {kind:?}");
            if name_visits != 0 { assert_eq!(visitor.observed_name.get(), expected_name); }
            // SAFETY: only initialized fixture-owned operand bytes are read;
            // object operands here are serialized i32 indices, not pointers.
            let actual = unsafe { std::slice::from_raw_parts(live, input.len()) };
            assert_eq!(actual, expected_output, "operand result: {kind:?}");
        };

        use Hp2ExprOperand::*;
        for (kind, bytes) in [
            (ExprByte, vec![0xa5]),
            (ExprWord, 0xa55au16.to_ne_bytes().to_vec()),
            (ExprInt, (-1234567i32).to_ne_bytes().to_vec()),
            (ExprFloat, 0x7fc1_2345u32.to_ne_bytes().to_vec()), // quiet NaN payload
            (ExprFloat, 0x0000_0000u32.to_ne_bytes().to_vec()), // positive zero
            (ExprFloat, 0x8000_0000u32.to_ne_bytes().to_vec()), // negative zero
        ] {
            run(kind, &bytes, ptr::null_mut(), None, None, false, &bytes, false, 0, 0, 0, -1);
        }
        for value in [0, name] {
            let bytes = value.to_ne_bytes();
            run(ExprName, &bytes, ptr::null_mut(), None, None, false, &bytes, value == 0, 0, 0, 1, value);
        }
        for (kind, record) in [(ExprObject, object), (ExprProperty, property),
            (ExprStruct, struct_object), (ExprClass, class)] {
            let bytes = index(access, record).to_ne_bytes();
            run(kind, &bytes, record, None, None, false, &bytes, false, 0, 1, 0, -1);
        }
        let original = object_index.to_ne_bytes();
        // Desired only; live only; both changed to the same value (the crucial
        // counterexample to desired==live); both changed differently.
        for (desired, live_change, output) in [
            (Some(other), None, other_index),
            (None, Some(other_index), object_index),
            (Some(other), Some(other_index), other_index),
            (Some(other), Some(third_index), other_index),
        ] {
            run(ExprObject, &original, object, desired, live_change, false,
                &output.to_ne_bytes(), false, 1, 1, 0, -1);
        }
        // On error the visitor's live mutation remains, no guard is reached,
        // and neither the desired slot nor the cursor is committed.
        run(ExprObject, &original, object, Some(third), Some(other_index), true,
            &other_index.to_ne_bytes(), false, 0, 1, 0, -1);
        drop(cleanup);
    }
    owner.shutdown().unwrap();
}
