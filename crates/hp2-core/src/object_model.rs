//! Pure pointer-chain `UObject::IsA` / `UStruct::IsChildOf` mirror reads.
//!
//! Both native implementations (`Core/Inc/UnType.h` `UObject::IsA`,
//! `Core/Inc/UnClass.h` `UStruct::IsChildOf`) are plain `SuperField`-chain
//! walks over fields the ABI already mirrors (`UObject::class`,
//! `UClass::super_field`), guarded only by `guardSlow`/`checkSlow`, which are
//! no-ops outside debug builds. Callers that already hold both pointers (no
//! name lookup, no class construction) read the chain directly instead of
//! crossing into C++ for it.

use hp2_abi::engine::generated::{UClass, UObject};
use std::ffi::c_void;

/// Mirrors `UObject::IsA( UClass* SomeBase ) const`: walks `object`'s class
/// and that class's `SuperField` chain for `target`. A null `target` always
/// matches, matching the native `return SomeBase==NULL;` fallthrough.
///
/// # Safety
/// `object`, if non-null, must be a live `UObject*`; `target`, if non-null,
/// must be a live `UClass*` reachable through `object`'s class chain. The
/// chain must not be concurrently mutated.
pub unsafe fn is_a(object: *const c_void, target: *const c_void) -> bool {
    unsafe {
        if object.is_null() {
            return false;
        }
        let mut class = (*object.cast::<UObject>()).class;
        while !class.is_null() {
            if class.cast_const() == target {
                return true;
            }
            class = (*class.cast::<UClass>()).super_field;
        }
        target.is_null()
    }
}

/// Mirrors `UStruct::IsChildOf( const UStruct* SomeBase ) const`: walks
/// `class_self` and its `SuperField` chain for `target`, including
/// `class_self` itself. Unlike [`is_a`], a null `target` never matches.
///
/// # Safety
/// `class_self`, if non-null, must be a live `UStruct`-family pointer
/// (`UStruct`/`UState`/`UClass`/`UFunction`), sharing the common
/// `UObject`+`UField` prefix that places `SuperField` at the same offset;
/// `target` is compared by identity only, never dereferenced.
pub unsafe fn is_child_of(class_self: *const c_void, target: *const c_void) -> bool {
    unsafe {
        let mut node = class_self;
        while !node.is_null() {
            if node == target {
                return true;
            }
            node = (*node.cast::<UClass>()).super_field;
        }
        false
    }
}

/// Source `UObject::IsIn`: tests the outer chain, excluding the object itself;
/// a null target matches the fallthrough even when there is no outer.
///
/// # Safety
/// A non-null object and its complete acyclic outer chain are live identities.
pub unsafe fn is_in(object: *const c_void, target: *const c_void) -> bool {
    let mut outer = if object.is_null() { std::ptr::null_mut() } else {
        unsafe { (*object.cast::<UObject>()).outer }
    };
    while !outer.is_null() {
        if outer.cast_const() == target { return true; }
        outer = unsafe { (*outer.cast::<UObject>()).outer };
    }
    target.is_null()
}

#[cfg(test)]
mod tests {
    use super::*;

    // `UObject` is exactly 72 bytes (ending at `class`); `UClass::super_field`
    // sits at that same offset 72, so a struct embedding `UObject` directly
    // followed by a `super_field` pointer reproduces the real layout with no
    // extra padding.
    const _: () = assert!(size_of::<UObject>() == 72);
    const _: () = assert!(std::mem::offset_of!(UClass, super_field) == 72);

    #[repr(C)]
    struct FakeClass {
        object: UObject,
        super_field: *mut c_void,
    }

    fn new_fake() -> FakeClass {
        FakeClass {
            object: unsafe { std::mem::zeroed() },
            super_field: std::ptr::null_mut(),
        }
    }

    fn make_chain(depth: usize) -> Vec<Box<FakeClass>> {
        let mut classes: Vec<Box<FakeClass>> = (0..depth).map(|_| Box::new(new_fake())).collect();
        for i in (1..depth).rev() {
            let parent: *mut c_void = (&mut *classes[i - 1] as *mut FakeClass).cast();
            classes[i].super_field = parent;
        }
        classes
    }

    #[test]
    fn is_a_walks_the_super_field_chain_and_matches_any_ancestor() {
        let mut chain = make_chain(3);
        let leaf_class: *mut c_void = (&mut *chain[2] as *mut FakeClass).cast();
        let root_class: *mut c_void = (&mut *chain[0] as *mut FakeClass).cast();
        let mut object = new_fake().object;
        object.class = leaf_class;
        let object_ptr: *const c_void = (&raw const object).cast();
        assert!(unsafe { is_a(object_ptr, leaf_class) });
        assert!(unsafe { is_a(object_ptr, root_class) });
        drop(chain);
    }

    #[test]
    fn is_a_rejects_an_unrelated_class_and_accepts_null_as_the_universal_base() {
        let mut chain = make_chain(2);
        let other = Box::new(new_fake());
        let leaf_class: *mut c_void = (&mut *chain[1] as *mut FakeClass).cast();
        let other_class: *const c_void = (&*other as *const FakeClass).cast();
        let mut object = new_fake().object;
        object.class = leaf_class;
        let object_ptr: *const c_void = (&raw const object).cast();
        assert!(!unsafe { is_a(object_ptr, other_class) });
        assert!(unsafe { is_a(object_ptr, std::ptr::null()) });
    }

    #[test]
    fn is_a_rejects_a_null_object() {
        assert!(!unsafe { is_a(std::ptr::null(), std::ptr::null()) });
    }

    #[test]
    fn is_child_of_matches_the_class_itself_and_every_ancestor_but_not_null() {
        let chain = make_chain(3);
        let leaf: *const c_void = (&*chain[2] as *const FakeClass).cast();
        let mid: *const c_void = (&*chain[1] as *const FakeClass).cast();
        let root: *const c_void = (&*chain[0] as *const FakeClass).cast();
        assert!(unsafe { is_child_of(leaf, leaf) });
        assert!(unsafe { is_child_of(leaf, mid) });
        assert!(unsafe { is_child_of(leaf, root) });
        assert!(!unsafe { is_child_of(leaf, std::ptr::null()) });
    }

    #[test]
    fn is_child_of_rejects_a_descendant_and_an_unrelated_class() {
        let chain = make_chain(2);
        let other = Box::new(new_fake());
        let root: *const c_void = (&*chain[0] as *const FakeClass).cast();
        let leaf: *const c_void = (&*chain[1] as *const FakeClass).cast();
        let other_class: *const c_void = (&*other as *const FakeClass).cast();
        assert!(!unsafe { is_child_of(root, leaf) });
        assert!(!unsafe { is_child_of(leaf, other_class) });
    }
}
