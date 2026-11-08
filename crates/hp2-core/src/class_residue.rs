//! `UnClass.cpp` residue: field-chain cleanup, dependency bookkeeping, linker
//! comparison, owner-class lookup, class CRC caching and `TFieldIterator`'s
//! walk, reused by [`cleanup_destroyed`] and [`get_return_property`]. Native
//! leaves retain virtual dispatch (`GetInheritanceSuper`, `IsPendingKill`,
//! `Modify`) and the lazily constructed `StaticClass()` singletons; every
//! other step is a plain `SuperField`/`Children`/`Next`/`RefLink`/`StructLink`
//! chain walk over fields the ABI already mirrors
//! (`hp2_abi::engine::generated`).

use crate::array_storage::{self, Hp2ArrayServices, Hp2ArraySlots};
use crate::object_model::is_a;
use hp2_abi::engine::generated::{
    FDependency, UClass, UField, UObject, UObjectProperty, UProperty, UStruct, UStructProperty, UTextBuffer,
};
use std::ffi::c_void;
use std::mem::size_of;
use std::ptr;

type Object = *mut c_void;
type Result<T = ()> = std::result::Result<T, i32>;

/// `UnObj.h` `CPF_ReturnParm`.
const CPF_RETURN_PARM: u32 = 0x400;

/// Lazily constructed `UClass`-family `StaticClass()` singletons `UStruct::CleanupDestroyed`,
/// `UField::GetOwnerClass` and `UFunction::GetReturnProperty` compare against.
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2ClassResidueStaticClass {
    FunctionClass,
    ObjectPropertyClass,
    StructPropertyClass,
    PropertyClass,
    ClassClass,
}

/// Native leaves for the `UnClass.cpp` residue. `is_editor` is `GIsEditor`;
/// `static_class` resolves one of [`Hp2ClassResidueStaticClass`]'s lazily
/// constructed singletons; `is_pending_kill`/`modify` are virtual `UObject` calls.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2ClassResidueServices {
    pub is_editor: unsafe extern "C" fn(*mut i32) -> i32,
    pub static_class: unsafe extern "C" fn(Hp2ClassResidueStaticClass, *mut Object) -> i32,
    pub is_pending_kill: unsafe extern "C" fn(Object, *mut i32) -> i32,
    pub modify: unsafe extern "C" fn(Object) -> i32,
}

fn status(code: i32) -> Result {
    if code == 0 { Ok(()) } else { Err(code) }
}

unsafe fn static_class(s: &Hp2ClassResidueServices, which: Hp2ClassResidueStaticClass) -> Result<Object> {
    unsafe {
        let mut value = ptr::null_mut();
        status((s.static_class)(which, &mut value))?;
        Ok(value)
    }
}

/// `UStruct::GetInheritanceSuper()` virtual dispatch (`UnClass.h:222`, overridden
/// by `UFunction` at `UnClass.h:300` to always return `NULL`; `UState`/`UClass`'s
/// overrides return `GetSuperState()`/`GetSuperClass()`, each the identical raw
/// `SuperField` pointer value as the base `UStruct` behavior). `struct_` must
/// already be non-null.
unsafe fn inheritance_super(struct_: Object, s: &Hp2ClassResidueServices) -> Result<Object> {
    unsafe {
        let function_class = static_class(s, Hp2ClassResidueStaticClass::FunctionClass)?;
        if is_a(struct_, function_class) {
            return Ok(ptr::null_mut());
        }
        Ok((*struct_.cast::<UField>()).super_field)
    }
}

/// `TFieldIterator<T>`: walks `struct_`'s `Children`/`Next` chain, then
/// `GetInheritanceSuper()` into ancestors, stopping on the first field whose
/// `IsA(target)`. Reused by [`cleanup_destroyed`]'s editor path (`T=UProperty`)
/// and [`get_return_property`] (`T=UProperty`, restricted to a single
/// `UFunction`'s own children since its `GetInheritanceSuper()` is always null).
pub(crate) struct FieldIter<M, S> {
    struct_: Object,
    field: Object,
    matches: M,
    inheritance_super: S,
}

impl<M, S> FieldIter<M, S>
where
    M: FnMut(Object) -> Result<bool>,
    S: FnMut(Object) -> Result<Object>,
{
    /// # Safety
    /// Struct/field pointers remain live, compiler-probed UStruct/UField objects.
    /// Callbacks return live handles and do not retain Rust references.
    pub(crate) unsafe fn new(struct_: Object, matches: M, inheritance_super: S) -> Result<Self> {
        unsafe {
            let field = if struct_.is_null() {
                ptr::null_mut()
            } else {
                (*struct_.cast::<UStruct>()).children
            };
            let mut it = FieldIter {
                struct_,
                field,
                matches,
                inheritance_super,
            };
            it.iterate_to_next()?;
            Ok(it)
        }
    }
    pub(crate) fn field(&self) -> Object {
        self.field
    }
    unsafe fn iterate_to_next(&mut self) -> Result {
        unsafe {
            while !self.struct_.is_null() {
                while !self.field.is_null() {
                    if (self.matches)(self.field)? {
                        return Ok(());
                    }
                    self.field = (*self.field.cast::<UField>()).next;
                }
                self.struct_ = (self.inheritance_super)(self.struct_)?;
                self.field = if self.struct_.is_null() {
                    ptr::null_mut()
                } else {
                    (*self.struct_.cast::<UStruct>()).children
                };
            }
            Ok(())
        }
    }
    /// # Safety
    /// A current field exists and the constructor's pointer contract still holds.
    pub(crate) unsafe fn advance(&mut self) -> Result {
        unsafe {
            self.field = (*self.field.cast::<UField>()).next;
            self.iterate_to_next()
        }
    }
}

/// `UStruct::CleanupDestroyed(BYTE* Data)` (`UnClass.cpp:930`). `GIsEditor`
/// selects between the slow `TFieldIterator<UProperty>` walk (which also
/// `Modify()`s cleared references) and the fast `RefLink`/`StructLink` walk.
///
/// # Safety
/// `this_struct` must be a live `UStruct`-family pointer; `data` must be the
/// matching instance's property base, valid for `this_struct`'s
/// `PropertiesSize`. Not virtually overridden anywhere in this codebase, so
/// nested-struct recursion never re-enters native code.
pub unsafe fn cleanup_destroyed(
    this_struct: *mut UStruct,
    data: *mut u8,
    s: &Hp2ClassResidueServices,
) -> Result {
    unsafe {
        let mut editor = 0;
        status((s.is_editor)(&mut editor))?;
        if editor != 0 {
            let property_class = static_class(s, Hp2ClassResidueStaticClass::PropertyClass)?;
            let object_property_class =
                static_class(s, Hp2ClassResidueStaticClass::ObjectPropertyClass)?;
            let struct_property_class =
                static_class(s, Hp2ClassResidueStaticClass::StructPropertyClass)?;
            let mut it = FieldIter::new(
                this_struct.cast(),
                |field| Ok(is_a(field, property_class)),
                |struct_| inheritance_super(struct_, s),
            )?;
            while !it.field.is_null() {
                let property = it.field;
                if is_a(property, object_property_class) {
                    cleanup_object_reference(data, property.cast(), s)?;
                } else if (*property.cast::<UObject>()).class == struct_property_class {
                    cleanup_substructure(data, property.cast(), s)?;
                }
                it.advance()?;
            }
        } else {
            let mut ref_link = (*this_struct).ref_link.cast::<UObjectProperty>();
            while !ref_link.is_null() {
                let property = ref_link.cast::<UProperty>();
                let offset = (*property).offset;
                let array_dim = (*property).array_dim;
                let linked = data.add(offset as usize).cast::<Object>();
                for k in 0..array_dim {
                    let slot = linked.add(k as usize);
                    let obj = slot.read_unaligned();
                    if !obj.is_null() {
                        let mut pending = 0;
                        status((s.is_pending_kill)(obj, &mut pending))?;
                        if pending != 0 {
                            slot.write_unaligned(ptr::null_mut());
                        }
                    }
                }
                ref_link = (*ref_link).next_reference.cast();
            }
            let mut struct_link = (*this_struct).struct_link.cast::<UStructProperty>();
            while !struct_link.is_null() {
                let property = struct_link.cast::<UProperty>();
                let offset = (*property).offset;
                let element_size = (*property).element_size;
                let array_dim = (*property).array_dim;
                let nested = (*struct_link).r#struct.cast::<UStruct>();
                for k in 0..array_dim {
                    cleanup_destroyed(
                        nested,
                        data.add((offset + k * element_size) as usize),
                        s,
                    )?;
                }
                struct_link = (*struct_link).next_struct.cast();
            }
        }
        Ok(())
    }
}

unsafe fn cleanup_object_reference(
    data: *mut u8,
    property: *mut UProperty,
    s: &Hp2ClassResidueServices,
) -> Result {
    unsafe {
        let offset = (*property).offset;
        let array_dim = (*property).array_dim;
        let linked = data.add(offset as usize).cast::<Object>();
        for k in 0..array_dim {
            let slot = linked.add(k as usize);
            let obj = slot.read_unaligned();
            if !obj.is_null() {
                let mut pending = 0;
                status((s.is_pending_kill)(obj, &mut pending))?;
                if pending != 0 {
                    status((s.modify)(obj))?;
                    slot.write_unaligned(ptr::null_mut());
                }
            }
        }
        Ok(())
    }
}

unsafe fn cleanup_substructure(
    data: *mut u8,
    property: *mut UProperty,
    s: &Hp2ClassResidueServices,
) -> Result {
    unsafe {
        let offset = (*property).offset;
        let element_size = (*property).element_size;
        let array_dim = (*property).array_dim;
        let nested = (*property.cast::<UStructProperty>()).r#struct.cast::<UStruct>();
        for k in 0..array_dim {
            cleanup_destroyed(nested, data.add((offset + k * element_size) as usize), s)?;
        }
        Ok(())
    }
}

/// Cache script text CRC when zero, then XOR each superclass's CRC.
///
/// # Safety
/// `class_` must be a live class with valid canonical text and superclass links.
pub unsafe fn get_class_crc(class_: *mut UClass) -> u32 {
    // SAFETY: the canonical headers are live; CRC has no reentrant callbacks.
    unsafe {
        if (*class_).class_crc == 0 {
            let script_text = ptr::addr_of!((*class_.cast::<UStruct>()).script_text)
                .read_unaligned().cast::<UTextBuffer>();
            if !script_text.is_null() {
                let text = ptr::addr_of!((*script_text).text.data).read_unaligned();
                // Empty FString dereference supplies the empty string.
                (*class_).class_crc = if (*script_text).text.num == 0 {
                    0
                } else {
                    crate::crc::string_crc(text.cast(), false)
                };
            }
        }
        let crc = (*class_).class_crc;
        let super_class = ptr::addr_of!((*class_).super_field).read_unaligned();
        if !super_class.is_null() {
            crc ^ get_class_crc(super_class.cast())
        } else {
            crc
        }
    }
}

/// Construct one dependency, using the class's cached/inherited script CRC.
///
/// # Safety
/// `dependency` must be writable canonical storage; nonnull `class` must be live.
pub unsafe fn initialize_dependency(dependency: *mut FDependency, class: Object, deep: i32) {
    // SAFETY: all stores target canonical fields; no reference crosses a callback.
    unsafe {
        ptr::addr_of_mut!((*dependency).class).write_unaligned(class);
        ptr::addr_of_mut!((*dependency).deep).write_unaligned(deep);
        let crc = if class.is_null() { 0 } else { get_class_crc(class.cast()) };
        ptr::addr_of_mut!((*dependency).script_text_crc).write_unaligned(crc);
    }
}

/// Compare a dependency's recorded CRC to the live class's cached/inherited CRC.
///
/// # Safety
/// `dependency` must be readable and its class must be nonnull and live.
pub unsafe fn dependency_is_up_to_date(dependency: *const FDependency) -> bool {
    // SAFETY: the caller retains the native nonnull-class assertion.
    unsafe {
        get_class_crc(ptr::addr_of!((*dependency).class).read_unaligned().cast())
            == (*dependency).script_text_crc
    }
}

/// `UClass::AddDependency(UClass* InClass, UBOOL InDeep)` (`UnClass.cpp:1208`).
/// The original loop never `break`s on a match, so it both overwrites every
/// matching entry in place *and* (since the loop always completes with
/// `i==Dependencies.Num()`) unconditionally appends a new entry; preserved
/// here exactly rather than treated as a bug to fix.
///
/// # Safety
/// `class_` must be a live `UClass*` with a valid `Dependencies` array;
/// `arrays` must be the live array allocator resource.
pub unsafe fn add_dependency(
    class_: *mut UClass,
    in_class: Object,
    in_deep: i32,
    arrays: Hp2ArrayServices,
) -> Result {
    unsafe {
        let slots = dependency_slots(class_);
        let mut i = 0;
        while i < slots.num.read() {
            let entry = slots.data.read_unaligned().cast::<FDependency>().add(i as usize);
            if ptr::addr_of!((*entry).class).read_unaligned() == in_class {
                initialize_dependency(entry, in_class, in_deep);
            }
            i += 1;
        }
        // Native placement new allocates before evaluating the constructor.
        // In particular an append failure leaves all duplicate updates intact,
        // and allocator reentry can change the CRC used by the new record.
        let index = array_storage::add(slots, 1, size_of::<FDependency>() as i32, DEPENDENCY_TAG.as_ptr().cast(), arrays)?;
        let entry = slots.data.read_unaligned().cast::<FDependency>().add(index as usize);
        initialize_dependency(entry, in_class, in_deep);
        Ok(())
    }
}

const DEPENDENCY_TAG: [u32; 1] = [0];

unsafe fn dependency_slots(class_: *mut UClass) -> Hp2ArraySlots {
    unsafe {
        let array = ptr::addr_of_mut!((*class_).dependencies);
        Hp2ArraySlots {
            data: ptr::addr_of_mut!((*array).data),
            num: ptr::addr_of_mut!((*array).num),
            max: ptr::addr_of_mut!((*array).max),
        }
    }
}

/// `Compare(UField* A, UField* B)` (`UnClass.cpp:1160`). `ENGINE_VERSION` is
/// 433 (`UnObjVer.h:14`), so the `#if ENGINE_VERSION<230` guid-comparison
/// branch never compiles; only the linker-index comparison is live.
/// `GetLinker()`/`GetLinkerIndex()` are non-virtual raw-field reads.
///
/// # Safety
/// `a` and `b` must be live `UField*` (or more precisely, live `UObject*`).
pub unsafe fn compare(a: Object, b: Object) -> i32 {
    unsafe {
        let a = a.cast::<UObject>();
        let b = b.cast::<UObject>();
        if (*a)._linker.is_null() || (*b)._linker.is_null() {
            return 0;
        }
        (*a)._linker_index - (*b)._linker_index
    }
}

/// `UField::GetOwnerClass()` (`UnClass.cpp:203`): walks the `Outer` chain
/// until an ancestor whose own `GetClass()` is exactly `UClass::StaticClass()`
/// (i.e. the nearest `UClass` ancestor). No null check, matching the original
/// (always terminates given a well-formed object graph).
///
/// # Safety
/// `field` must be a live `UObject*` with an `Outer` chain terminating in a
/// `UClass` instance.
pub unsafe fn get_owner_class(field: Object, s: &Hp2ClassResidueServices) -> Result<Object> {
    unsafe {
        let class_class = static_class(s, Hp2ClassResidueStaticClass::ClassClass)?;
        let mut obj = field;
        while (*obj.cast::<UObject>()).class != class_class {
            obj = (*obj.cast::<UObject>()).outer;
        }
        Ok(obj)
    }
}

/// `UFunction::GetReturnProperty()` (`UnClass.cpp:1511`): the first
/// `CPF_ReturnParm` property among this function's own `TFieldIterator<UProperty>`
/// walk (its `GetInheritanceSuper()` is always null, so the walk never leaves
/// `function`'s own `Children`).
///
/// # Safety
/// `function` must be a live `UFunction*`.
pub unsafe fn get_return_property(function: Object, s: &Hp2ClassResidueServices) -> Result<Object> {
    unsafe {
        let property_class = static_class(s, Hp2ClassResidueStaticClass::PropertyClass)?;
        let mut it = FieldIter::new(
            function,
            |field| Ok(is_a(field, property_class)),
            |struct_| inheritance_super(struct_, s),
        )?;
        while !it.field.is_null() {
            if (*it.field.cast::<UProperty>()).property_flags & CPF_RETURN_PARM != 0 {
                return Ok(it.field);
            }
            it.advance()?;
        }
        Ok(ptr::null_mut())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct FakeField {
        pad0: [u8; 72],
        super_field: Object,
        next: Object,
        pad_rest: [u8; 16],
    }
    impl Default for FakeField {
        fn default() -> Self {
            FakeField {
                pad0: [0; 72],
                super_field: ptr::null_mut(),
                next: ptr::null_mut(),
                pad_rest: [0; 16],
            }
        }
    }

    fn null_obj() -> Object {
        ptr::null_mut()
    }

    #[test]
    fn compare_returns_zero_when_either_linker_missing() {
        #[repr(C)]
        struct Fake {
            pad0: [u8; 32],
            linker: Object,
            linker_index: i32,
            pad: [u8; 28],
        }
        let mut a = Fake {
            pad0: [0; 32],
            linker: null_obj(),
            linker_index: 5,
            pad: [0; 28],
        };
        let mut b = Fake {
            pad0: [0; 32],
            linker: 1 as Object,
            linker_index: 2,
            pad: [0; 28],
        };
        unsafe {
            assert_eq!(
                compare(
                    (&raw mut a).cast::<c_void>(),
                    (&raw mut b).cast::<c_void>()
                ),
                0
            );
        }
    }

    #[test]
    fn compare_subtracts_linker_index_when_both_linked() {
        #[repr(C)]
        struct Fake {
            pad0: [u8; 32],
            linker: Object,
            linker_index: i32,
            pad: [u8; 28],
        }
        let mut a = Fake {
            pad0: [0; 32],
            linker: 1 as Object,
            linker_index: 7,
            pad: [0; 28],
        };
        let mut b = Fake {
            pad0: [0; 32],
            linker: 1 as Object,
            linker_index: 2,
            pad: [0; 28],
        };
        unsafe {
            assert_eq!(
                compare(
                    (&raw mut a).cast::<c_void>(),
                    (&raw mut b).cast::<c_void>()
                ),
                5
            );
        }
    }

    thread_local! {
        static STATIC_CLASSES: RefCell<[Object; 5]> = const { RefCell::new([ptr::null_mut(); 5]) };
    }

    unsafe extern "C" fn fixture_static_class(
        which: Hp2ClassResidueStaticClass,
        out: *mut Object,
    ) -> i32 {
        let value = STATIC_CLASSES.with(|c| c.borrow()[which as usize]);
        unsafe { out.write(value) };
        0
    }
    unsafe extern "C" fn fixture_is_editor(out: *mut i32) -> i32 {
        unsafe { out.write(0) };
        0
    }
    unsafe extern "C" fn fixture_is_pending_kill(_obj: Object, out: *mut i32) -> i32 {
        unsafe { out.write(0) };
        0
    }
    unsafe extern "C" fn fixture_modify(_obj: Object) -> i32 {
        0
    }

    fn fixture_services() -> Hp2ClassResidueServices {
        Hp2ClassResidueServices {
            is_editor: fixture_is_editor,
            static_class: fixture_static_class,
            is_pending_kill: fixture_is_pending_kill,
            modify: fixture_modify,
        }
    }

    #[test]
    fn get_owner_class_walks_outer_chain_to_class_instance() {
        #[repr(C)]
        struct FakeObject {
            pad0: [u8; 48],
            outer: Object,
            object_flags: u32,
            name: [u8; 4],
            class: Object,
        }
        let class_marker: i32 = 0;
        let class_marker_ptr = (&raw const class_marker).cast_mut().cast::<c_void>();
        STATIC_CLASSES.with(|c| c.borrow_mut()[Hp2ClassResidueStaticClass::ClassClass as usize] = class_marker_ptr);

        let mut owner_class = FakeObject {
            pad0: [0; 48],
            outer: null_obj(),
            object_flags: 0,
            name: [0; 4],
            class: class_marker_ptr,
        };
        let mut package = FakeObject {
            pad0: [0; 48],
            outer: null_obj(),
            object_flags: 0,
            name: [0; 4],
            class: 2 as Object,
        };
        package.outer = (&raw mut owner_class).cast();
        let mut leaf = FakeObject {
            pad0: [0; 48],
            outer: (&raw mut package).cast(),
            object_flags: 0,
            name: [0; 4],
            class: 3 as Object,
        };

        let s = fixture_services();
        unsafe {
            let result = get_owner_class((&raw mut leaf).cast(), &s).unwrap();
            assert_eq!(result, (&raw mut owner_class).cast());
        }
    }

    #[test]
    fn field_iter_walks_children_then_next_chain() {
        // Three children of a struct with no super; the iterator's target
        // class matches every field (is_a with a null target always matches).
        let mut c1 = FakeField::default();
        let mut c2 = FakeField::default();
        let mut c3 = FakeField::default();
        c1.next = (&raw mut c2).cast();
        c2.next = (&raw mut c3).cast();

        #[repr(C)]
        struct FakeStruct {
            pad0: [u8; 96],
            script_text: Object,
            children: Object,
            pad_rest: [u8; 88],
        }
        let mut st = FakeStruct {
            pad0: [0; 96],
            script_text: null_obj(),
            children: (&raw mut c1).cast(),
            pad_rest: [0; 88],
        };
        // Super chain terminates immediately (super_field null on c1's parent
        // struct, mirrored through the UField prefix at offset 72).
        let s = fixture_services();
        unsafe {
            let mut it = FieldIter::new(
                (&raw mut st).cast(),
                |field| Ok(is_a(field, null_obj())),
                |struct_| inheritance_super(struct_, &s),
            )
            .unwrap();
            let mut seen = vec![];
            while !it.field.is_null() {
                seen.push(it.field);
                it.advance().unwrap();
            }
            assert_eq!(
                seen,
                vec![
                    (&raw mut c1).cast::<c_void>(),
                    (&raw mut c2).cast::<c_void>(),
                    (&raw mut c3).cast::<c_void>(),
                ]
            );
        }
    }

    thread_local! {
        static MODIFY_CALLS: Cell<usize> = const { Cell::new(0) };
        static MODIFY_SLOT: Cell<*mut Object> = const { Cell::new(ptr::null_mut()) };
    }

    unsafe extern "C" fn fixture_is_editor_enabled(out: *mut i32) -> i32 {
        unsafe { out.write(1) };
        0
    }
    unsafe extern "C" fn fixture_pending_kill(_object: Object, out: *mut i32) -> i32 {
        unsafe { out.write(1) };
        0
    }
    unsafe extern "C" fn fixture_modify_counted(object: Object) -> i32 {
        MODIFY_SLOT.with(|slot| {
            let slot = slot.get();
            assert!(!slot.is_null());
            assert_eq!(unsafe { slot.read() }, object, "Modify must precede clearing the reference");
        });
        MODIFY_CALLS.with(|calls| calls.set(calls.get() + 1));
        0
    }

    #[test]
    fn editor_cleanup_modifies_then_clears_pending_object_references() {
        let mut property_class = std::mem::MaybeUninit::<UClass>::zeroed();
        let mut object_property_class = std::mem::MaybeUninit::<UClass>::zeroed();
        let mut function_class = std::mem::MaybeUninit::<UClass>::zeroed();
        let mut struct_property_class = std::mem::MaybeUninit::<UClass>::zeroed();
        let property_class = property_class.as_mut_ptr();
        let object_property_class = object_property_class.as_mut_ptr();
        let function_class = function_class.as_mut_ptr();
        let struct_property_class = struct_property_class.as_mut_ptr();
        unsafe {
            ptr::addr_of_mut!((*object_property_class).super_field)
                .write(property_class.cast());
        }
        STATIC_CLASSES.with(|classes| {
            let mut classes = classes.borrow_mut();
            classes[Hp2ClassResidueStaticClass::FunctionClass as usize] = function_class.cast();
            classes[Hp2ClassResidueStaticClass::ObjectPropertyClass as usize] =
                object_property_class.cast();
            classes[Hp2ClassResidueStaticClass::StructPropertyClass as usize] =
                struct_property_class.cast();
            classes[Hp2ClassResidueStaticClass::PropertyClass as usize] = property_class.cast();
        });

        let mut object_property = std::mem::MaybeUninit::<UObjectProperty>::zeroed();
        let property = object_property.as_mut_ptr();
        let property_base = property.cast::<UProperty>();
        unsafe {
            ptr::addr_of_mut!((*property_base).array_dim).write(1);
            ptr::addr_of_mut!((*property_base).offset).write(0);
            ptr::addr_of_mut!((*property.cast::<UObject>()).class)
                .write(object_property_class.cast());
        }
        let mut structure = std::mem::MaybeUninit::<UStruct>::zeroed();
        let structure_ptr = structure.as_mut_ptr();
        unsafe {
            ptr::addr_of_mut!((*structure_ptr).children).write(property.cast());
            ptr::addr_of_mut!((*structure_ptr).ref_link).write(ptr::null_mut());
        }
        let mut data = [1usize as Object];
        let services = Hp2ClassResidueServices {
            is_editor: fixture_is_editor_enabled,
            static_class: fixture_static_class,
            is_pending_kill: fixture_pending_kill,
            modify: fixture_modify_counted,
        };
        MODIFY_CALLS.with(|calls| calls.set(0));
        MODIFY_SLOT.with(|slot| slot.set(data.as_mut_ptr()));
        unsafe {
            cleanup_destroyed(structure_ptr, data.as_mut_ptr().cast(), &services)
                .expect("editor object-reference cleanup");
        }
        assert!(data[0].is_null());
        MODIFY_CALLS.with(|calls| assert_eq!(calls.get(), 1));
    }
    #[test]
    fn fast_cleanup_uses_reference_link_without_editor_modification() {
        let mut object_property = std::mem::MaybeUninit::<UObjectProperty>::zeroed();
        let property = object_property.as_mut_ptr();
        unsafe {
            ptr::addr_of_mut!((*property.cast::<UProperty>()).array_dim).write(1);
            ptr::addr_of_mut!((*property.cast::<UProperty>()).offset).write(0);
            ptr::addr_of_mut!((*property).next_reference).write(ptr::null_mut());
        }
        let mut structure = std::mem::MaybeUninit::<UStruct>::zeroed();
        let structure_ptr = structure.as_mut_ptr();
        unsafe {
            ptr::addr_of_mut!((*structure_ptr).ref_link).write(property.cast());
        }
        let mut data = [1usize as Object];
        let services = Hp2ClassResidueServices {
            is_editor: fixture_is_editor,
            static_class: fixture_static_class,
            is_pending_kill: fixture_pending_kill,
            modify: fixture_modify_counted,
        };
        MODIFY_CALLS.with(|calls| calls.set(0));
        unsafe {
            cleanup_destroyed(structure_ptr, data.as_mut_ptr().cast(), &services)
                .expect("fast object-reference cleanup");
        }
        assert!(data[0].is_null());
        MODIFY_CALLS.with(|calls| assert_eq!(calls.get(), 0));
    }
}
