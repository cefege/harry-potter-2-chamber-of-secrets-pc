//! Pure raw-storage policy contracts, not fabricated live C++ objects. The SDK
//! consumer separately covers actual UClass construction and typed destruction.
use hp2_abi::engine::generated::{FDependency, TArrayRaw, UClass, UField, UObject, UState, UStruct, UTextBuffer};
use hp2_core::{
    array_storage::Hp2ArrayServices,
    class_lifecycle::{self as lifecycle, Hp2ClassLifecycleServices},
    class_residue,
};
use hp2_core::core_records::ClassRecord;
use std::{
    cell::Cell,
    ffi::c_void,
    mem::{MaybeUninit, offset_of, size_of},
    ptr,
};

type P = *mut c_void;
thread_local! { static ACTIVE: Cell<*mut Host> = const { Cell::new(ptr::null_mut()) }; }

// Keep callback addresses stable without moving or reborrowing Box pointees.
struct Stable<T: ?Sized>(*mut T);
impl<T: ?Sized> Stable<T> {
    fn from_box(value: Box<T>) -> Self { Self(Box::into_raw(value)) }
    fn pointer(&self) -> *mut T { self.0 }
}
impl<T: ?Sized> std::ops::Deref for Stable<T> {
    type Target = T;
    fn deref(&self) -> &T { unsafe { &*self.0 } }
}
impl<T: ?Sized> std::ops::DerefMut for Stable<T> {
    fn deref_mut(&mut self) -> &mut T { unsafe { &mut *self.0 } }
}
impl<T: ?Sized> Drop for Stable<T> {
    fn drop(&mut self) { unsafe { drop(Box::from_raw(self.0)); } }
}
struct Host {
    class: *mut UClass,
    parent: *mut UClass,
    blocks: Vec<Stable<[u8]>>,
    allocations: usize,
    fail_allocation: usize,
    mutate_allocation: usize,
    mutate: Option<unsafe fn(*mut Host)>,
    prefixes: Vec<(i32, i32, Vec<u8>)>,
    widths: Vec<i32>,
    bind_error: i32,
    exit_error: i32,
    super_error: i32,
    bound: Vec<u8>,
    exited: Vec<u8>,
    super_dead: bool,
    exit_mutates_defaults: bool,
}
struct Fixture {
    class: Stable<UClass>,
    parent: Stable<UClass>,
    host: Stable<Host>,
    previous: *mut Host,
}
impl Fixture {
    fn new() -> Self {
        // Only raw initialized Rust fields are accessed by these tests.
        let class = Stable::from_box(Box::new(unsafe { std::mem::zeroed::<UClass>() }));
        let parent = Stable::from_box(Box::new(unsafe { std::mem::zeroed::<UClass>() }));
        let text = Stable::from_box(vec![b'x', 0, 0, 0, 0, 0, 0, 0].into_boxed_slice());
        unsafe {
            (*class.pointer().cast::<ClassRecord>()).default_prop_text = TArrayRaw {
                data: text.pointer().cast(), num: 2, max: 2,
            };
        }
        let host = Stable::from_box(Box::new(Host {
            class: class.pointer(),
            parent: parent.pointer(),
            blocks: vec![text],
            allocations: 0,
            fail_allocation: 0,
            mutate_allocation: 0,
            mutate: None,
            prefixes: vec![],
            widths: vec![],
            bind_error: 0,
            exit_error: 0,
            super_error: 0,
            bound: vec![],
            exited: vec![],
            super_dead: false,
            exit_mutates_defaults: false,
        }));
        let previous = ACTIVE.with(|active| active.replace(host.pointer()));
        Self {
            class,
            parent,
            host,
            previous,
        }
    }
    fn default_text(&self) -> TArrayRaw {
        unsafe { (*self.class.pointer().cast::<ClassRecord>()).default_prop_text }
    }
    fn arrays(&mut self) -> Hp2ArrayServices {
        Hp2ArrayServices {
            context: self.host.pointer().cast(),
            realloc,
            checked_int_size,
            free,
        }
    }
    fn seed(&mut self, parent: bool, bytes: &[u8]) {
        let block = Stable::from_box(bytes.to_vec().into_boxed_slice());
        let array = TArrayRaw {
            data: block.pointer().cast(),
            num: bytes.len() as i32,
            max: bytes.len() as i32,
        };
        self.host.blocks.push(block);
        if parent {
            self.parent.defaults = array;
        } else {
            self.class.defaults = array;
        }
    }
    fn inherit(&mut self) -> Result<(), i32> {
        let arrays = self.arrays();
        unsafe { lifecycle::inherit_defaults(self.class.pointer(), arrays, LEAVES) }
    }
    fn reset(&mut self) -> Result<(), i32> {
        let arrays = self.arrays();
        unsafe { lifecycle::reset_defaults(self.class.pointer(), arrays) }
    }
    fn destroy(&mut self) -> Result<(), i32> {
        let arrays = self.arrays();
        unsafe { lifecycle::destroy_class(self.class.pointer(), arrays, LEAVES) }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        ACTIVE.with(|active| active.set(self.previous));
    }
}
unsafe fn bytes(array: TArrayRaw) -> Vec<u8> {
    if array.num == 0 {
        vec![]
    } else {
        unsafe { std::slice::from_raw_parts(array.data.cast::<u8>(), array.num as usize).to_vec() }
    }
}
unsafe extern "C" fn realloc(
    context: P,
    data: P,
    size: usize,
    _: *const i32,
    width: i32,
    tag: *const c_void,
    out: *mut P,
) -> i32 {
    unsafe {
        let h = context.cast::<Host>();
        assert_eq!(*tag.cast::<u32>(), 0);
        (*h).allocations += 1;
        (*h).widths.push(width);
        let a = (*(*h).class).defaults;
        // Failed growth may have Num > allocated capacity: only inspect the
        // actual old backing prefix, not the not-yet-constructed placement.
        let old_len = (*h)
            .blocks
            .iter()
            .find(|b| b.pointer().cast::<c_void>() == a.data)
            .map_or(0, |b| b.pointer().len());
        let committed = TArrayRaw {
            num: a.num.min(old_len as i32),
            ..a
        };
        (*h).prefixes.push((a.num, a.max, bytes(committed)));
        if (*h).allocations == (*h).mutate_allocation {
            if let Some(mutate) = (*h).mutate.take() {
                mutate(h);
            }
        }
        if (*h).allocations == (*h).fail_allocation {
            return 81;
        }
        if size == 0 {
            out.write(ptr::null_mut());
            return 0;
        }
        let block = Stable::from_box(vec![0xa5; size].into_boxed_slice());
        if let Some(old) = (*h)
            .blocks
            .iter()
            .find(|b| b.pointer().cast::<c_void>() == data)
        {
            let n = old.pointer().len().min(size);
            ptr::copy_nonoverlapping(old.pointer().cast::<u8>(), block.pointer().cast::<u8>(), n);
        }
        out.write(block.pointer().cast());
        (*h).blocks.push(block);
        0
    }
}
unsafe extern "C" fn checked_int_size(_: P, n: usize, out: *mut i32) -> i32 {
    unsafe {
        out.write(n as i32);
    }
    0
}
unsafe extern "C" fn free(_: P, _: P) -> i32 {
    0
}
unsafe extern "C" fn bind(class: P) -> i32 {
    ACTIVE.with(|active| unsafe {
        let h = active.get();
        (*h).bound = bytes((*class.cast::<UClass>()).defaults);
        (*h).bind_error
    })
}
unsafe extern "C" fn exit_properties(data: P, class: P) -> i32 {
    ACTIVE.with(|active| unsafe {
        let h = active.get();
        let c = class.cast::<UClass>();
        assert!((*c).net_fields.data.is_null());
        assert!((*c).dependencies.data.is_null());
        assert!((*c).package_imports.data.is_null());
        assert_eq!(data, (*c).defaults.data);
        (*h).exited = bytes((*c).defaults);
        if (*h).exit_mutates_defaults {
            (*c).properties_size = 3;
            let arrays = Hp2ArrayServices {
                context: h.cast(),
                realloc,
                checked_int_size,
                free,
            };
            lifecycle::reset_defaults(c, arrays).unwrap();
            (*c).defaults.data.cast::<u8>().write(29);
        }
        (*h).exit_error
    })
}
unsafe extern "C" fn super_destroy(class: P) -> i32 {
    ACTIVE.with(|active| unsafe {
        let h = active.get();
        assert!((*class.cast::<UClass>()).defaults.data.is_null());
        assert!((*class.cast::<ClassRecord>()).default_prop_text.data.is_null());
        if (*h).super_error != 0 {
            return (*h).super_error;
        }
        (*h).super_dead = true;
        0
    })
}
const LEAVES: Hp2ClassLifecycleServices = Hp2ClassLifecycleServices {
    bind,
    exit_properties,
    super_destroy,
};

#[test]
fn instance_initialization_changes_only_within_and_crc() {
    let mut f = Fixture::new();
    f.seed(false, &[7, 8]);
    f.class.properties_size = 37;
    f.class.class_crc = 0xffff;
    f.class.super_field = (f.parent.pointer()).cast();
    let data = f.class.defaults.data;
    let root = (f.parent.pointer()).cast();
    unsafe {
        lifecycle::initialize_class_instance_data(f.class.pointer(), root);
    }
    assert_eq!(f.class.class_within, root);
    assert_eq!(f.class.class_crc, 0);
    assert_eq!(f.class.properties_size, 37);
    assert_eq!(f.class.super_field, root);
    assert_eq!(f.class.defaults.data, data);
    assert_eq!(unsafe { bytes(f.class.defaults) }, [7, 8]);
    assert_eq!(f.host.allocations, 0);
}

#[test]
fn reset_reloads_size_after_reentry_and_does_not_initialize_header() {
    unsafe fn mutate(h: *mut Host) {
        unsafe {
            (*(*h).class).properties_size = 6;
        }
    }
    let mut f = Fixture::new();
    f.class.properties_size = 2;
    f.class.class_crc = 0x1234;
    f.host.mutate_allocation = 1;
    f.host.mutate = Some(mutate);
    f.reset().unwrap();
    assert_eq!(f.class.defaults.num, 6);
    assert_eq!(f.class.defaults.max, 40);
    assert_eq!(unsafe { bytes(f.class.defaults) }, [0xa5; 6]);
    assert_eq!(f.class.class_crc, 0x1234);
    assert_eq!(
        &f.host.prefixes[..2],
        &[(0, 2, vec![]), (6, 40, vec![0xa5; 2])]
    );
}

#[test]
fn reset_failure_preserves_published_counts_and_can_recover() {
    for allocation in [1, 2] {
        unsafe fn grow(h: *mut Host) {
            unsafe {
                (*(*h).class).properties_size = 6;
            }
        }
        let mut f = Fixture::new();
        f.seed(false, &[9, 8]);
        let original = f.class.defaults.data;
        f.class.properties_size = 2;
        f.host.mutate_allocation = 1;
        f.host.mutate = Some(grow);
        f.host.fail_allocation = allocation;
        assert_eq!(f.reset(), Err(81));
        assert_eq!(f.class.defaults.num, if allocation == 1 { 0 } else { 6 });
        assert_eq!(f.class.defaults.max, if allocation == 1 { 2 } else { 40 });
        if allocation == 1 {
            assert_eq!(f.class.defaults.data, original);
        }
        f.host.fail_allocation = 0;
        f.reset().unwrap();
        assert_eq!(f.class.defaults.num, 6);
        assert_eq!(f.class.defaults.max, 6);
    }
}

#[test]
fn inheritance_bulk_copies_live_parent_and_retains_storage_on_bind_error() {
    let mut f = Fixture::new();
    let source: Vec<_> = (0..4096).map(|i| (i % 251) as u8).collect();
    f.seed(true, &source);
    f.seed(false, &[99]);
    f.parent.class_within = (f.parent.pointer()).cast();
    f.class.super_field = (f.parent.pointer()).cast();
    f.host.bind_error = 82;
    assert_eq!(f.inherit(), Err(82));
    assert_eq!(f.class.class_within, f.parent.class_within);
    assert_eq!(unsafe { bytes(f.class.defaults) }, source);
    assert_eq!(f.host.bound, source);
    assert_eq!(f.host.allocations, 1);
    assert_ne!(f.class.defaults.data, f.parent.defaults.data);
    unsafe {
        f.class.defaults.data.cast::<u8>().write(255);
    }
    assert_eq!(unsafe { *f.parent.defaults.data.cast::<u8>() }, 0);
    f.host.bind_error = 0;
    f.inherit().unwrap();
    assert_eq!(unsafe { bytes(f.class.defaults) }, source);
}

#[test]
fn inheritance_reloads_source_after_growth_and_preserves_committed_prefix() {
    unsafe fn grow_source(h: *mut Host) {
        unsafe {
            let block = Stable::from_box(vec![10, 20, 30, 40, 50].into_boxed_slice());
            (*(*h).parent).defaults = TArrayRaw {
                data: block.pointer().cast(),
                num: 5,
                max: 5,
            };
            (*h).blocks.push(block);
            // The assignment's source identity is already selected.
            (*(*h).class).super_field = ptr::null_mut();
        }
    }
    let mut f = Fixture::new();
    f.seed(true, &[1, 2]);
    f.class.super_field = (f.parent.pointer()).cast();
    f.host.mutate_allocation = 1;
    f.host.mutate = Some(grow_source);
    f.inherit().unwrap();
    assert_eq!(unsafe { bytes(f.class.defaults) }, [10, 20, 30, 40, 50]);
    assert_eq!(f.host.prefixes[1], (3, 36, vec![10, 20]));
    assert_eq!(f.host.bound, [10, 20, 30, 40, 50]);
}

#[test]
fn inheritance_growth_failure_leaves_exact_constructed_prefix_and_recovers() {
    unsafe fn grow(h: *mut Host) {
        unsafe {
            (*(*h).parent).defaults.num = 5;
        }
    }
    let mut f = Fixture::new();
    f.seed(true, &[1, 2, 3, 4, 5]);
    f.parent.defaults.num = 2;
    f.class.super_field = (f.parent.pointer()).cast();
    f.host.mutate_allocation = 1;
    f.host.mutate = Some(grow);
    f.host.fail_allocation = 2;
    assert_eq!(f.inherit(), Err(81));
    assert_eq!(f.class.defaults.num, 3);
    assert_eq!(f.class.defaults.max, 36);
    assert_eq!(
        unsafe { std::slice::from_raw_parts(f.class.defaults.data.cast::<u8>(), 2) },
        [1, 2]
    );
    assert!(f.host.bound.is_empty());
    f.host.fail_allocation = 0;
    f.inherit().unwrap();
    assert_eq!(unsafe { bytes(f.class.defaults) }, [1, 2, 3, 4, 5]);
}

#[test]
fn inheritance_reloads_both_buffers_and_live_counts_after_placement_allocation() {
    unsafe fn mutate(h: *mut Host) {
        unsafe {
            let block = Stable::from_box(vec![91, 92, 93, 94, 95].into_boxed_slice());
            (*(*h).parent).defaults.data = block.pointer().cast();
            (*(*h).parent).defaults.num = 3;
            (*h).blocks.push(block);
            (*(*h).class).defaults.num = 1;
        }
    }
    let mut f = Fixture::new();
    f.seed(true, &[1, 2, 3, 4, 5]);
    f.class.super_field = (f.parent.pointer()).cast();
    // Empty reserves five slots, then reentry reduces the live capacity to two.
    unsafe fn reserve_two(h: *mut Host) {
        unsafe {
            (*(*h).class).defaults.max = 2;
            (*h).mutate_allocation = 2;
            (*h).mutate = Some(mutate);
        }
    }
    f.host.mutate_allocation = 1;
    f.host.mutate = Some(reserve_two);
    f.inherit().unwrap();
    assert_eq!(f.class.defaults.num, 1);
    assert_eq!(f.host.bound, [1]);
    assert_eq!(
        unsafe { std::slice::from_raw_parts(f.class.defaults.data.cast::<u8>(), 3) },
        [1, 2, 93]
    );
    assert_eq!(unsafe { bytes(f.parent.defaults) }, [91, 92, 93]);
}

#[test]
fn inheritance_observes_source_shrink_and_self_assignment() {
    unsafe fn shrink(h: *mut Host) {
        unsafe {
            (*(*h).parent).defaults.num = 1;
        }
    }
    let mut f = Fixture::new();
    f.seed(true, &[4, 5, 6]);
    f.class.super_field = (f.parent.pointer()).cast();
    f.host.mutate_allocation = 1;
    f.host.mutate = Some(shrink);
    f.inherit().unwrap();
    assert_eq!(unsafe { bytes(f.class.defaults) }, [4]);
    assert_eq!(f.class.defaults.max, 3);
    let data = f.class.defaults.data;
    f.class.super_field = (f.class.pointer()).cast();
    f.inherit().unwrap();
    assert_eq!(f.class.defaults.data, data);
    assert_eq!(f.host.allocations, 1);
    f.class.super_field = ptr::null_mut();
    f.host.bind_error = 83;
    f.inherit().unwrap();
}

#[test]
fn destroy_preserves_error_prefixes_and_allows_cleanup_recovery() {
    for step in 1..=7 {
        let mut f = Fixture::new();
        f.seed(false, &[11, 12, 13]);
        // Independent initialized raw buffers with each element's actual width.
        let class = &mut *f.class;
        for (array, width) in [
            (&mut class.net_fields, std::mem::size_of::<P>()),
            (
                &mut class.dependencies,
                std::mem::size_of::<hp2_abi::engine::generated::FDependency>(),
            ),
            (&mut class.package_imports, std::mem::size_of::<i32>()),
        ] {
            let block = Stable::from_box(vec![0; width].into_boxed_slice());
            *array = TArrayRaw {
                data: block.pointer().cast(),
                num: 1,
                max: 1,
            };
            f.host.blocks.push(block);
        }
        let original = f.class.defaults.data;
        match step {
            1..=3 => f.host.fail_allocation = step,
            4 => f.host.exit_error = 84,
            5 => f.host.fail_allocation = 4,
            6 => f.host.fail_allocation = 5,
            7 => f.host.super_error = 87,
            _ => unreachable!(),
        }
        assert_eq!(
            f.destroy(),
            Err(match step {
                4 => 84,
                7 => 87,
                _ => 81,
            })
        );
        assert_eq!(f.class.net_fields.num, 0);
        assert_eq!(f.class.net_fields.data.is_null(), step > 1);
        assert_eq!(f.class.dependencies.data.is_null(), step > 2);
        assert_eq!(f.class.package_imports.data.is_null(), step > 3);
        if step <= 5 {
            assert_eq!(f.class.defaults.data, original);
        } else {
            assert!(f.class.defaults.data.is_null());
        }
        assert_eq!(f.default_text().data.is_null(), step == 7);
        assert!(!f.host.super_dead);
        f.host.fail_allocation = 0;
        f.host.exit_error = 0;
        f.host.super_error = 0;
        f.destroy().unwrap();
        assert!(f.class.defaults.data.is_null());
        assert!(f.default_text().data.is_null());
        assert!(f.host.super_dead);
    }
}

#[test]
fn destroy_uses_live_defaults_after_recursive_property_exit() {
    let mut f = Fixture::new();
    f.seed(false, &[11, 12]);
    f.host.exit_mutates_defaults = true;
    f.destroy().unwrap();
    assert_eq!(f.host.exited, [11, 12]);
    assert!(f.class.defaults.data.is_null());
    assert_eq!(f.class.defaults.num, 0);
    assert!(f.host.super_dead);
}

#[test]
fn recursive_exit_failure_retains_replacement_defaults_until_recovery() {
    let mut f = Fixture::new();
    f.seed(false, &[11, 12]);
    f.host.exit_mutates_defaults = true;
    f.host.exit_error = 84;
    assert_eq!(f.destroy(), Err(84));
    assert_eq!(f.host.exited, [11, 12]);
    assert_eq!(unsafe { bytes(f.class.defaults) }, [29, 12, 0xa5]);
    assert!(!f.default_text().data.is_null());
    assert!(!f.host.super_dead);
    f.host.exit_mutates_defaults = false;
    f.host.exit_error = 0;
    f.destroy().unwrap();
    assert_eq!(f.host.exited, [29, 12, 0xa5]);
    assert!(f.class.defaults.data.is_null());
    assert!(f.host.super_dead);
}

// Allocate actual Rust raw storage and initialize every byte in place, including
// padding. Never construct or borrow a native object, or move a typed value whose
// padding could become uninitialized before comparing preservation.
struct RawMetadata<T>(Box<MaybeUninit<T>>);
impl<T> RawMetadata<T> {
    fn new() -> Self {
        let mut storage = Box::new(MaybeUninit::<T>::uninit());
        unsafe {
            storage.as_mut_ptr().cast::<u8>().write_bytes(0xa7, size_of::<T>());
        }
        Self(storage)
    }

    fn pointer(&mut self) -> *mut T {
        self.0.as_mut_ptr()
    }

    fn snapshot(&self) -> Vec<u8> {
        unsafe {
            std::slice::from_raw_parts(self.0.as_ptr().cast::<u8>(), size_of::<T>()).to_vec()
        }
    }
}

fn assert_preserved(before: &[u8], after: &[u8], written: &[(usize, usize)]) {
    assert_eq!(before.len(), after.len());
    for (index, (&old, &new)) in before.iter().zip(after).enumerate() {
        if !written.iter().any(|&(offset, size)| (offset..offset + size).contains(&index)) {
            assert_eq!(old, new, "unexpected write at byte {index}");
        }
    }
}

#[test]
fn field_forms_preserve_distinct_superclass_and_link_ownership() {
    for native in [false, true] {
        let mut storage = RawMetadata::<UField>::new();
        let field = storage.pointer();
        let mut parent = RawMetadata::<UField>::new();
        let parent = parent.pointer().cast();
        unsafe {
            ptr::addr_of_mut!((*field).super_field).write_unaligned(parent);
            let before = storage.snapshot();
            lifecycle::initialize_field_registration_data(field, field.cast(), native);
            assert_eq!(
                ptr::addr_of!((*field).super_field).read_unaligned(),
                if native { field.cast() } else { parent },
            );
            assert!(ptr::addr_of!((*field).next).read_unaligned().is_null());
            assert!(ptr::addr_of!((*field).hash_next).read_unaligned().is_null());
            let mut written = vec![
                (offset_of!(UField, next), size_of::<P>()),
                (offset_of!(UField, hash_next), size_of::<P>()),
            ];
            if native {
                written.push((offset_of!(UField, super_field), size_of::<P>()));
            }
            assert_preserved(&before, &storage.snapshot(), &written);
        }
    }

    let mut storage = RawMetadata::<UField>::new();
    let field = storage.pointer();
    let before = storage.snapshot();
    unsafe {
        lifecycle::initialize_field_instance_data(field, field.cast());
        assert_eq!(ptr::addr_of!((*field).super_field).read_unaligned(), field.cast());
    }
    assert_preserved(
        &before,
        &storage.snapshot(),
        &[(offset_of!(UField, super_field), size_of::<P>())],
    );
}

#[test]
fn struct_registration_preserves_friendly_name_script_vptr_and_padding() {
    let mut storage = RawMetadata::<UStruct>::new();
    let structure = storage.pointer();
    let before = storage.snapshot();
    unsafe {
        lifecycle::initialize_struct_registration_data(structure, 381, 16);
        assert_eq!(ptr::addr_of!((*structure).properties_size).read_unaligned(), 381);
        assert_eq!(ptr::addr_of!((*structure).properties_align).read_unaligned(), 16);
        assert_eq!(ptr::addr_of!((*structure).text_pos).read_unaligned(), 0);
        assert_eq!(ptr::addr_of!((*structure).line).read_unaligned(), 0);
        for link in [
            ptr::addr_of!((*structure).script_text).read_unaligned(),
            ptr::addr_of!((*structure).children).read_unaligned(),
            ptr::addr_of!((*structure).ref_link).read_unaligned(),
            ptr::addr_of!((*structure).struct_link).read_unaligned(),
            ptr::addr_of!((*structure).property_link).read_unaligned(),
            ptr::addr_of!((*structure).config_link).read_unaligned(),
            ptr::addr_of!((*structure).constructor_link).read_unaligned(),
        ] {
            assert!(link.is_null());
        }
    }
    assert_preserved(
        &before,
        &storage.snapshot(),
        &[
            (offset_of!(UStruct, script_text), size_of::<P>()),
            (offset_of!(UStruct, children), size_of::<P>()),
            (offset_of!(UStruct, properties_size), size_of::<i32>()),
            (offset_of!(UStruct, properties_align), size_of::<i32>()),
            (offset_of!(UStruct, text_pos), size_of::<i32>()),
            (offset_of!(UStruct, line), size_of::<i32>()),
            (offset_of!(UStruct, ref_link), size_of::<P>()),
            (offset_of!(UStruct, struct_link), size_of::<P>()),
            (offset_of!(UStruct, property_link), size_of::<P>()),
            (offset_of!(UStruct, config_link), size_of::<P>()),
            (offset_of!(UStruct, constructor_link), size_of::<P>()),
        ],
    );
}

#[test]
fn struct_instance_inherits_layout_but_uses_own_root_name_including_self_alias() {
    for parent_kind in 0..3 {
        let mut storage = RawMetadata::<UStruct>::new();
        let structure = storage.pointer();
        let mut parent_storage = RawMetadata::<UStruct>::new();
        let parent = parent_storage.pointer();
        unsafe {
            ptr::addr_of_mut!((*structure.cast::<UObject>()).name)
                .write_unaligned([0x12, 0x34, 0x56, 0x78]);
            ptr::addr_of_mut!((*parent.cast::<UObject>()).name)
                .write_unaligned([0xff, 0xee, 0xdd, 0xcc]);
            ptr::addr_of_mut!((*structure).properties_size).write_unaligned(53);
            ptr::addr_of_mut!((*structure).properties_align).write_unaligned(32);
            ptr::addr_of_mut!((*parent).properties_size).write_unaligned(719);
            ptr::addr_of_mut!((*parent).properties_align).write_unaligned(64);
            let (superclass, size, alignment) = match parent_kind {
                0 => (ptr::null_mut(), 0, 8),
                1 => (parent, 719, 64),
                _ => (structure, 53, 32),
            };
            lifecycle::initialize_field_instance_data(structure.cast(), superclass.cast());
            let before = storage.snapshot();
            let parent_before = parent_storage.snapshot();
            lifecycle::initialize_struct_instance_data(structure, superclass, 8);
            assert_eq!(ptr::addr_of!((*structure).properties_size).read_unaligned(), size);
            assert_eq!(ptr::addr_of!((*structure).properties_align).read_unaligned(), alignment);
            assert_eq!(
                ptr::addr_of!((*structure).friendly_name).read_unaligned(),
                [0x12, 0x34, 0x56, 0x78],
            );
            assert_preserved(
                &before,
                &storage.snapshot(),
                &[
                    (offset_of!(UStruct, properties_size), size_of::<i32>()),
                    (offset_of!(UStruct, properties_align), size_of::<i32>()),
                    (offset_of!(UStruct, friendly_name), 4),
                ],
            );
            assert_eq!(parent_before, parent_storage.snapshot());
        }
    }
}

#[test]
fn state_registration_preserves_hash_storage_and_inherited_bytes() {
    let mut storage = RawMetadata::<UState>::new();
    let state = storage.pointer();
    let before = storage.snapshot();
    unsafe {
        lifecycle::initialize_state_registration_data(state);
        assert_eq!(ptr::addr_of!((*state).probe_mask).read_unaligned(), 0);
        assert_eq!(ptr::addr_of!((*state).ignore_mask).read_unaligned(), 0);
        assert_eq!(ptr::addr_of!((*state).state_flags).read_unaligned(), 0);
        assert_eq!(ptr::addr_of!((*state).label_table_offset).read_unaligned(), 0);
    }
    assert_preserved(
        &before,
        &storage.snapshot(),
        &[
            (offset_of!(UState, probe_mask), size_of::<u64>()),
            (offset_of!(UState, ignore_mask), size_of::<u64>()),
            (offset_of!(UState, state_flags), size_of::<u32>()),
            (offset_of!(UState, label_table_offset), size_of::<u16>()),
        ],
    );
}

unsafe extern "C" fn registration_constructor(object: P) -> i32 {
    unsafe {
        ptr::addr_of_mut!((*object.cast::<UObject>()).object_flags).write_unaligned(0x04000000);
    }
    -73
}

#[test]
fn class_registration_preserves_static_within_and_roundtrips_full_config_pointer() {
    let config_text = [b'S' as u32, b'y' as u32, b's' as u32, 0];
    for native in [false, true] {
        for config in [
            config_text.as_ptr(),
            ptr::without_provenance::<u32>(usize::MAX - 0x1234),
            ptr::null(),
        ] {
            let mut storage = RawMetadata::<UClass>::new();
            let class = storage.pointer();
            let mut within_storage = RawMetadata::<UClass>::new();
            let old_within = within_storage.pointer().cast();
            let guid = [0x12345678, 0x90abcdef, 0xfedcba09, 0x87654321];
            let callback: Option<unsafe extern "C" fn(P) -> i32> =
                Some(registration_constructor);
            let (constructor, static_constructor) =
                if native { (callback, None) } else { (None, callback) };
            unsafe {
                ptr::addr_of_mut!((*class).class_within).write_unaligned(old_within);
                ptr::addr_of_mut!((*class).class_guid).write_unaligned(guid);
                let before = storage.snapshot();
                lifecycle::initialize_class_registration_data(
                    class,
                    native,
                    0x40000001,
                    0x30,
                    class.cast(),
                    ptr::addr_of!((*class).class_guid).cast(),
                    config,
                    constructor,
                    static_constructor,
                );
                assert_eq!(ptr::addr_of!((*class).class_flags).read_unaligned(), 0x40000031);
                assert_eq!(ptr::addr_of!((*class).class_unique).read_unaligned(), 0);
                assert_eq!(ptr::addr_of!((*class).class_guid).read_unaligned(), guid);
                assert_eq!(
                    ptr::addr_of!((*class).class_within).read_unaligned(),
                    if native { class.cast() } else { old_within },
                );
                let ordinary = ptr::addr_of!((*class).class_constructor).read_unaligned();
                let static_slot = ptr::addr_of!((*class).class_static_constructor).read_unaligned();
                let (selected, inactive) = if native { (ordinary, static_slot) } else { (static_slot, ordinary) };
                assert!(inactive.is_null());
                assert!(!selected.is_null());
                // Exercise dispatch, not separately reified function addresses:
                // Rust permits distinct addresses for the same function.
                let construct: unsafe extern "C" fn(P) -> i32 = std::mem::transmute(selected);
                let mut object: UObject = std::mem::zeroed();
                assert_eq!(construct((&raw mut object).cast()), -73);
                assert_eq!(object.object_flags, 0x04000000);
                assert_eq!(
                    ptr::addr_of!((*class).class_config_name)
                        .cast::<*const u32>()
                        .read_unaligned(),
                    config,
                );
                if size_of::<P>() == 8 {
                    assert_eq!(
                        offset_of!(UClass, class_crc),
                        offset_of!(UClass, class_config_name) + 4,
                    );
                    let pointer_bytes = (config as usize).to_ne_bytes();
                    assert_eq!(
                        ptr::addr_of!((*class).class_crc).read_unaligned().to_ne_bytes(),
                        pointer_bytes[4..8],
                    );
                }
                let mut written = vec![
                    (offset_of!(UClass, class_flags), size_of::<u32>()),
                    (offset_of!(UClass, class_unique), size_of::<i32>()),
                    (offset_of!(UClass, class_guid), size_of::<[u32; 4]>()),
                    (offset_of!(UClass, class_config_name), size_of::<P>()),
                    (offset_of!(UClass, class_constructor), size_of::<P>()),
                    (offset_of!(UClass, class_static_constructor), size_of::<P>()),
                ];
                if native {
                    written.push((offset_of!(UClass, class_within), size_of::<P>()));
                    written.push((offset_of!(UClass, class_crc), size_of::<u32>()));
                }
                assert_preserved(&before, &storage.snapshot(), &written);
            }
        }
    }
}

#[test]
fn dependency_updates_all_duplicates_then_appends_and_keeps_nonmatching_entries() {
    let mut f = Fixture::new();
    f.parent.class_crc = 0x1234;
    let dependency_class = (f.parent.pointer()).cast();
    let block = Stable::from_box(vec![0; 3 * size_of::<FDependency>()].into_boxed_slice());
    let data = block.pointer().cast::<FDependency>();
    // SAFETY: initialized independent raw records backed by the owned block.
    unsafe {
        for (i, class) in [dependency_class, ptr::null_mut(), dependency_class].into_iter().enumerate() {
            data.add(i).write_unaligned(FDependency { class, deep: 7, script_text_crc: 99 });
        }
    }
    f.class.dependencies = TArrayRaw { data: data.cast(), num: 3, max: 3 };
    f.host.blocks.push(block);
    let arrays = f.arrays();
    // SAFETY: the fixture owns canonical classes and live dependency storage.
    unsafe {
        class_residue::add_dependency(f.class.pointer(), dependency_class, -3, arrays).unwrap();
        assert_eq!(f.class.dependencies.num, 4);
        let data = f.class.dependencies.data.cast::<FDependency>();
        for i in [0, 2, 3] {
            let entry = data.add(i).read_unaligned();
            assert_eq!(entry.class, dependency_class);
            assert_eq!(entry.deep, -3);
            assert_eq!(entry.script_text_crc, 0x1234);
        }
        let unchanged = data.add(1).read_unaligned();
        assert!(unchanged.class.is_null());
        assert_eq!((unchanged.deep, unchanged.script_text_crc), (7, 99));
    }
}

#[test]
fn dependency_append_failure_retains_duplicate_updates_and_native_growth_header() {
    let mut f = Fixture::new();
    f.parent.class_crc = 42;
    let dependency_class = (f.parent.pointer()).cast();
    let block = Stable::from_box(vec![0; size_of::<FDependency>()].into_boxed_slice());
    let original = block.pointer().cast::<FDependency>();
    // SAFETY: one initialized raw dependency in its own allocation.
    unsafe { original.write_unaligned(FDependency { class: dependency_class, deep: 0, script_text_crc: 9 }); }
    f.class.dependencies = TArrayRaw { data: original.cast(), num: 1, max: 1 };
    f.host.blocks.push(block);
    f.host.fail_allocation = 1;
    let arrays = f.arrays();
    // SAFETY: failed allocation keeps the original readable record; the failed
    // append publishes Num/Max but does not initialize its new element.
    unsafe {
        assert_eq!(class_residue::add_dependency(&mut *f.class, dependency_class, 1, arrays), Err(81));
        assert_eq!(f.class.dependencies.data, original.cast());
        assert_eq!((f.class.dependencies.num, f.class.dependencies.max), (2, 34));
        let updated = original.read_unaligned();
        assert_eq!((updated.deep, updated.script_text_crc), (1, 42));
    }
}

#[test]
fn dependency_append_evaluates_crc_after_allocator_reentry_and_null_class_uses_zero() {
    unsafe fn change_crc(host: *mut Host) {
        // SAFETY: the host owns its parent class and the allocator invokes once.
        unsafe { (*(*host).parent).class_crc = 0x4321; }
    }
    let mut f = Fixture::new();
    f.parent.class_crc = 0x1234;
    f.host.mutate_allocation = 1;
    f.host.mutate = Some(change_crc);
    let dependency_class = (f.parent.pointer()).cast();
    let arrays = f.arrays();
    // SAFETY: canonical storage stays live across the synchronous allocation.
    unsafe {
        class_residue::add_dependency(f.class.pointer(), dependency_class, 1, arrays).unwrap();
        let entry = f.class.dependencies.data.cast::<FDependency>().read_unaligned();
        assert_eq!(entry.script_text_crc, 0x4321);
        class_residue::add_dependency(f.class.pointer(), ptr::null_mut(), 0, arrays).unwrap();
        let entry = f.class.dependencies.data.cast::<FDependency>().add(1).read_unaligned();
        assert!(entry.class.is_null());
        assert_eq!(entry.script_text_crc, 0);
    }
}

#[test]
fn class_crc_uses_canonical_text_caches_nonzero_and_xors_live_superclass_crc() {
    let f = Fixture::new();
    // SAFETY: mirrors contain scalar/pointer fields; text is a live terminated
    // TCHAR buffer, not a fabricated C++ object or an alternate text owner.
    let mut text: UTextBuffer = unsafe { std::mem::zeroed() };
    let mut chars = [b'A' as hp2_core::ansi::Tchar, 0x1234, 0];
    let chars_ptr = chars.as_mut_ptr();
    text.text = TArrayRaw { data: chars_ptr.cast(), num: 3, max: 3 };
    let class = f.class.pointer();
    let parent = f.parent.pointer();
    // SAFETY: UClass contains the canonical UStruct prefix and superclass slot.
    unsafe {
        ptr::addr_of_mut!((*class.cast::<UStruct>()).script_text).write_unaligned((&raw mut text).cast());
        (*class).super_field = parent.cast();
        (*parent).class_crc = 0x55aa;
        let script_crc = hp2_core::crc::string_crc(chars_ptr, false);
        assert_ne!(script_crc, 0);
        assert_eq!(class_residue::get_class_crc(class), script_crc ^ 0x55aa);
        assert_eq!((*class).class_crc, script_crc);
        chars_ptr.write(b'B' as hp2_core::ansi::Tchar);
        (*parent).class_crc = 0xaa55;
        assert_eq!(class_residue::get_class_crc(class), script_crc ^ 0xaa55);
        (*class).class_crc = 0;
        let changed_crc = hp2_core::crc::string_crc(chars_ptr, false);
        assert_ne!(changed_crc, script_crc);
        assert_eq!(class_residue::get_class_crc(class), changed_crc ^ 0xaa55);
        let mut dependency: FDependency = std::mem::zeroed();
        class_residue::initialize_dependency(&raw mut dependency, class.cast(), 1);
        assert!(class_residue::dependency_is_up_to_date(&raw const dependency));
        (*parent).class_crc ^= 1;
        assert!(!class_residue::dependency_is_up_to_date(&raw const dependency));
    }
}

#[test]
fn zero_class_crc_recomputes_empty_or_previously_missing_script_text() {
    let f = Fixture::new();
    let class = f.class.pointer();
    // SAFETY: all canonical prefixes and terminated TCHAR storage remain live.
    unsafe {
        assert_eq!(class_residue::get_class_crc(class), 0);
        let mut text: UTextBuffer = std::mem::zeroed();
        let text_ptr = &raw mut text;
        ptr::addr_of_mut!((*class.cast::<UStruct>()).script_text).write_unaligned(text_ptr.cast());
        assert_eq!(class_residue::get_class_crc(class), 0);
        let mut chars = [b'X' as hp2_core::ansi::Tchar, 0];
        let chars_ptr = chars.as_mut_ptr();
        ptr::addr_of_mut!((*text_ptr).text).write_unaligned(TArrayRaw { data: chars_ptr.cast(), num: 2, max: 2 });
        let expected = hp2_core::crc::string_crc(chars_ptr, false);
        assert_eq!(class_residue::get_class_crc(class), expected);
        assert_ne!(expected, 0);
    }
}
