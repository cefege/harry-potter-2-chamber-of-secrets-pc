//! Property layout, inherited field chains, replication and native binding policy.
//! Native callbacks expose live typed storage; no field arrays are copied. Rust
//! owns the call-local replication-end map using the engine's fallible allocator.

use crate::array_storage::{self, Hp2ArrayServices, Hp2ArraySlots};
use crate::map_storage::{self, Hp2MapServices, Hp2MapSlots};
use hp2_abi::engine::generated::{
    FRepRecord, TArrayRaw, UArrayProperty, UBoolProperty, UClass, UField,
    UFixedArrayProperty, UMapProperty, UObject, UProperty, UStruct, UStructProperty,
};
use std::ffi::c_void;
use std::ptr::{self, null_mut};
use std::mem::size_of;

type Object = *mut c_void;
pub const PROPERTY: i32 = 1;
pub const OBJECT_PROPERTY: i32 = 2;
pub const STRUCT_PROPERTY: i32 = 4;
pub const FUNCTION: i32 = 8;
pub const NET: i32 = 0x20;
pub const CONFIG: i32 = 0x4000;
pub const CTOR: i32 = 0x400000;
// Scalar selectors for the native typed storage seam.
pub const SIZE: i32 = 0;
pub const ALIGN: i32 = 1;
pub const OFFSET: i32 = 2;
pub const CPP: i32 = 3;
pub const PROPERTY_SIZE: i32 = 4;
pub const PROPERTY_ALIGN: i32 = 5;
pub const FLAGS: i32 = 6;
pub const REP_OFFSET: i32 = 7;
pub const ARRAY_DIM: i32 = 8;
pub const FUNCTION_FLAGS: i32 = 9;
pub const REP_INDEX: i32 = 10;
// Pointer selectors. Potentially virtual access is always exception bridged.
pub const CHILDREN: i32 = 0;
pub const NEXT: i32 = 1;
pub const OUTER: i32 = 2;
pub const SUPER: i32 = 3;
pub const OWNER_CLASS: i32 = 4;
pub const REP_OWNER: i32 = 5;
pub const SUPER_FUNCTION: i32 = 6;
pub const CLASS_SUPER: i32 = 7;
// Chain selectors, shared by the root head and property next-slot setter.
pub const REFERENCE_CHAIN: i32 = 0;
pub const STRUCT_CHAIN: i32 = 1;
pub const PROPERTY_CHAIN: i32 = 2;
pub const CONFIG_CHAIN: i32 = 3;
pub const CTOR_CHAIN: i32 = 4;

#[repr(C)]
pub struct Hp2PropertyLinkServices {
    pub pointer: unsafe extern "C" fn(*mut c_void, i32, *mut *mut c_void) -> i32,
    pub integer: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub set_integer: unsafe extern "C" fn(*mut c_void, i32, i32),
    pub kind: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub editor: unsafe extern "C" fn() -> i32,
    pub preload: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub link: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
    /// Returns a native UObject member slot (Index, Outer, Flags, Name, Class),
    /// or -1 when this is not a loading native Object member.
    pub native_member: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    /// Null previous writes the root head; otherwise writes previous's next slot.
    pub set_chain: unsafe extern "C" fn(*mut c_void, *mut c_void, i32, *mut c_void),
    pub set_rep_owner: unsafe extern "C" fn(*mut c_void, *mut c_void),
    pub condition_end: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub condition_equal: unsafe extern "C" fn(*mut c_void, *mut c_void, i32, *mut i32) -> i32,
}

fn status(value: i32) -> Result<(), i32> {
    if value == 0 { Ok(()) } else { Err(value) }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hp2PropertyTypeLayoutKind {
    TypeLayoutByte = 0,
    TypeLayoutInt = 1,
    TypeLayoutBool = 2,
    TypeLayoutFloat = 3,
    TypeLayoutObject = 4,
    TypeLayoutName = 5,
    TypeLayoutString = 6,
    TypeLayoutFixedArray = 7,
    TypeLayoutArray = 8,
    TypeLayoutMap = 9,
    TypeLayoutStruct = 10,
}

/// Actual native constants, supplied by the compiler-probed caller. This record
/// is immutable and disjoint from every object callbacks can mutate.
#[repr(C)]
pub struct Hp2PropertyTypeLayoutAbi {
    pub property_alignment: i32,
    pub byte_size: i32,
    pub int_size: i32,
    pub bool_size: i32,
    pub float_size: i32,
    pub object_size: i32,
    pub name_size: i32,
    pub string_size: i32,
    pub array_size: i32,
    pub map_size: i32,
    pub object_alignment: i32,
    pub name_alignment: i32,
    pub string_alignment: i32,
    pub array_alignment: i32,
    pub map_alignment: i32,
    pub first_bitfield: u32,
}

/// Checked native leaves. Copy the table before calling any leaf.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PropertyTypeLayoutServices {
    pub is_kind: unsafe extern "C" fn(Object, Hp2PropertyTypeLayoutKind, *mut i32) -> i32,
    pub owner_size: unsafe extern "C" fn(Object, *mut i32) -> i32,
    pub owner_merge_bools: unsafe extern "C" fn(Object, *mut i32) -> i32,
    pub preload: unsafe extern "C" fn(Object, Object) -> i32,
    pub link: unsafe extern "C" fn(Object, Object) -> i32,
    pub checked_size: unsafe extern "C" fn(u64, *mut i32) -> i32,
}

pub(crate) trait TypeLayoutOps {
    type Error;
    fn is_kind(&self, property: Object, kind: Hp2PropertyTypeLayoutKind) -> Result<bool, Self::Error>;
    fn owner_size(&self, property: Object) -> Result<i32, Self::Error>;
    fn owner_merge_bools(&self, property: Object) -> Result<bool, Self::Error>;
    fn preload(&self, object: Object) -> Result<(), Self::Error>;
    fn link(&self, property: Object) -> Result<(), Self::Error>;
    fn checked_size(&self, bytes: u64) -> Result<i32, Self::Error>;
}

struct LegacyTypeLayoutOps {
    archive: Object,
    services: Hp2PropertyTypeLayoutServices,
}

impl TypeLayoutOps for LegacyTypeLayoutOps {
    type Error = i32;
    fn is_kind(&self, property: Object, kind: Hp2PropertyTypeLayoutKind) -> Result<bool, i32> {
        let mut value = 0;
        unsafe { status((self.services.is_kind)(property, kind, &mut value))?; }
        Ok(value != 0)
    }
    fn owner_size(&self, property: Object) -> Result<i32, i32> {
        let mut value = 0;
        unsafe { status((self.services.owner_size)(property, &mut value))?; }
        Ok(value)
    }
    fn owner_merge_bools(&self, property: Object) -> Result<bool, i32> {
        let mut value = 0;
        unsafe { status((self.services.owner_merge_bools)(property, &mut value))?; }
        Ok(value != 0)
    }
    fn preload(&self, object: Object) -> Result<(), i32> {
        unsafe { status((self.services.preload)(self.archive, object)) }
    }
    fn link(&self, property: Object) -> Result<(), i32> {
        unsafe { status((self.services.link)(property, self.archive)) }
    }
    fn checked_size(&self, bytes: u64) -> Result<i32, i32> {
        let mut value = 0;
        unsafe { status((self.services.checked_size)(bytes, &mut value))?; }
        Ok(value)
    }
}

/// # Safety
/// Objects and services satisfy the native property layout contract.
pub unsafe fn type_alignment(
    property: *mut UProperty, abi: &Hp2PropertyTypeLayoutAbi, services: Hp2PropertyTypeLayoutServices,
) -> Result<i32, i32> {
    unsafe { type_alignment_with(property, abi, &LegacyTypeLayoutOps { archive: null_mut(), services }) }
}

/// # Safety
/// Property, archive, previous property and services remain live through hooks.
pub unsafe fn type_layout(
    property: *mut UProperty, archive: Object, previous: Object, kind: Hp2PropertyTypeLayoutKind,
    abi: &Hp2PropertyTypeLayoutAbi, services: Hp2PropertyTypeLayoutServices,
) -> Result<(), i32> {
    unsafe { type_layout_with(property, previous, kind, abi, &LegacyTypeLayoutOps { archive, services }) }
}

const NATIVE: u32 = 0x1000;

fn type_align(size: i32, alignment: i32) -> i32 {
    size.wrapping_add(alignment.wrapping_sub(1)) & !alignment.wrapping_sub(1)
}


/// # Safety
/// Objects must have their actual compiler-probed native layout and remain
/// live across exception-bridged callbacks. Successful family classification
/// permits the corresponding derived cast. `abi` is immutable and disjoint.
pub(crate) unsafe fn type_alignment_with<O: TypeLayoutOps>(
    property: *mut UProperty, abi: &Hp2PropertyTypeLayoutAbi, ops: &O,
) -> Result<i32, O::Error> {
    use Hp2PropertyTypeLayoutKind::*;
    unsafe {
        if (*property).cpp_property != 0 || (*property).property_flags & NATIVE != 0 {
            return Ok(abi.property_alignment);
        }
        if ops.is_kind(property.cast(), TypeLayoutObject)? {
            return Ok(abi.object_alignment.min(abi.property_alignment));
        }
        if ops.is_kind(property.cast(), TypeLayoutName)? {
            return Ok(abi.name_alignment.min(abi.property_alignment));
        }
        if ops.is_kind(property.cast(), TypeLayoutString)? {
            return Ok(abi.string_alignment.min(abi.property_alignment));
        }
        if ops.is_kind(property.cast(), TypeLayoutArray)? {
            return Ok(abi.array_alignment.min(abi.property_alignment));
        }
        if ops.is_kind(property.cast(), TypeLayoutMap)? {
            return Ok(abi.map_alignment.min(abi.property_alignment));
        }
        if ops.is_kind(property.cast(), TypeLayoutFixedArray)? {
            return type_alignment_with(
                (*property.cast::<UFixedArrayProperty>()).inner.cast(), abi, ops,
            );
        }
        if ops.is_kind(property.cast(), TypeLayoutStruct)? {
            return Ok((*(*property.cast::<UStructProperty>())
                .r#struct
                .cast::<UStruct>())
            .properties_align);
        }
        Ok(if (*property).element_size == 2 {
            2
        } else if (*property).element_size >= 4 {
            abi.property_alignment
        } else {
            1
        })
    }
}

/// # Safety
/// Same storage/callback contract as `type_alignment`; `kind` describes the
/// actual derived property. Fixed-array Count > 0 is a native precondition.
/// No object reference is retained across a callback. Failed leaves preserve
/// the original statement's committed prefix, without rollback.
pub(crate) unsafe fn type_layout_with<O: TypeLayoutOps>(
    property: *mut UProperty, previous: Object, kind: Hp2PropertyTypeLayoutKind,
    abi: &Hp2PropertyTypeLayoutAbi, ops: &O,
) -> Result<(), O::Error> {
    use Hp2PropertyTypeLayoutKind::*;
    unsafe {
        match kind {
            TypeLayoutBool => {
                let boolean = property.cast::<UBoolProperty>();
                let native_offset = (*property).offset;
                let native_mask = (*boolean).bit_mask;
                if (*property).cpp_property != 0 {
                    (*property).offset = native_offset;
                    (*boolean).bit_mask = native_mask;
                } else {
                    let previous_bool =
                        !previous.is_null() && ops.is_kind(previous, TypeLayoutBool)?;
                    let merge = ops.owner_merge_bools(property.cast())?;
                    if merge
                        && previous_bool
                        && (*previous.cast::<UBoolProperty>()).bit_mask.wrapping_shl(1) != 0
                    {
                        (*property).offset = (*previous.cast::<UProperty>()).offset;
                        (*boolean).bit_mask =
                            (*previous.cast::<UBoolProperty>()).bit_mask.wrapping_shl(1);
                    } else {
                        let size = ops.owner_size(property.cast())?;
                        (*property).offset = type_align(size, abi.bool_size);
                        (*boolean).bit_mask = abi.first_bitfield;
                    }
                }
                (*property).element_size = abi.bool_size;
                return Ok(());
            }
            TypeLayoutFixedArray => {
                let fixed = property.cast::<UFixedArrayProperty>();
                ops.preload((*fixed).inner)?;
                ops.link((*fixed).inner)?;
                let product = ((*(*fixed).inner.cast::<UProperty>()).element_size as u64)
                    .wrapping_mul((*fixed).count as u64);
                (*property).element_size = ops.checked_size(product)?;
            }
            TypeLayoutArray => {
                let array = property.cast::<UArrayProperty>();
                ops.preload((*array).inner)?;
                ops.link((*array).inner)?;
                (*property).element_size = abi.array_size;
            }
            TypeLayoutMap => {
                let map = property.cast::<UMapProperty>();
                ops.preload((*map).key)?;
                ops.link((*map).key)?;
                ops.preload((*map).value)?;
                ops.link((*map).value)?;
                (*property).element_size = abi.map_size;
            }
            TypeLayoutStruct => {
                let structure = property.cast::<UStructProperty>();
                ops.preload((*structure).r#struct)?;
                (*property).element_size =
                    (*(*structure).r#struct.cast::<UStruct>()).properties_size;
            }
            TypeLayoutByte => (*property).element_size = abi.byte_size,
            TypeLayoutInt => (*property).element_size = abi.int_size,
            TypeLayoutFloat => (*property).element_size = abi.float_size,
            TypeLayoutObject => (*property).element_size = abi.object_size,
            TypeLayoutName => (*property).element_size = abi.name_size,
            TypeLayoutString => (*property).element_size = abi.string_size,
        }
        let size = ops.owner_size(property.cast())?;
        let alignment = match kind {
            TypeLayoutByte | TypeLayoutInt | TypeLayoutFloat => (*property).element_size,
            _ => type_alignment_with(property, abi, ops)?,
        };
        (*property).offset = type_align(size, alignment);
        match kind {
            TypeLayoutFixedArray => {
                if (*property).property_flags & NATIVE == 0 {
                    (*property).property_flags |= (*(*property.cast::<UFixedArrayProperty>())
                        .inner
                        .cast::<UProperty>())
                    .property_flags
                        & CTOR as u32;
                }
            }
            TypeLayoutString | TypeLayoutArray | TypeLayoutMap => {
                if (*property).property_flags & NATIVE == 0 {
                    (*property).property_flags |= CTOR as u32;
                }
            }
            TypeLayoutStruct => {
                if !(*(*property.cast::<UStructProperty>())
                    .r#struct
                    .cast::<UStruct>())
                .constructor_link
                .is_null()
                    && (*property).property_flags & NATIVE == 0
                {
                    (*property).property_flags |= CTOR as u32;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// # Safety
/// Native admission assertions have succeeded and the actual fixed-array
/// storage is writable. Only Inner followed by Count is published.
pub unsafe fn publish_fixed_inner(property: *mut UFixedArrayProperty, inner: Object, count: i32) {
    unsafe {
        (*property).inner = inner;
        (*property).count = count;
    }
}

/// # Safety
/// Native admission assertions have succeeded and actual array storage is
/// writable. Only Inner is published.
pub unsafe fn publish_array_inner(property: *mut UArrayProperty, inner: Object) {
    unsafe {
        (*property).inner = inner;
    }
}

unsafe fn pointer(s: &Hp2PropertyLinkServices, object: Object, field: i32) -> Result<Object, i32> {
    let mut result = null_mut();
    status(unsafe { (s.pointer)(object, field, &mut result) })?;
    Ok(result)
}

struct Fields {
    structure: Object,
    field: Object,
    mask: i32,
}
impl Fields {
    unsafe fn new(root: Object, mask: i32, s: &Hp2PropertyLinkServices) -> Result<Self, i32> {
        let mut it = Self {
            structure: root,
            field: unsafe { pointer(s, root, CHILDREN)? },
            mask,
        };
        unsafe {
            it.seek(s)?;
        }
        Ok(it)
    }
    unsafe fn seek(&mut self, s: &Hp2PropertyLinkServices) -> Result<(), i32> {
        unsafe {
            while !self.structure.is_null() {
                while !self.field.is_null() {
                    if self.mask == 0 || (s.kind)(self.field, self.mask) != 0 {
                        return Ok(());
                    }
                    self.field = pointer(s, self.field, NEXT)?;
                }
                self.structure = pointer(s, self.structure, SUPER)?;
                if !self.structure.is_null() {
                    self.field = pointer(s, self.structure, CHILDREN)?;
                }
            }
        }
        Ok(())
    }
    unsafe fn advance(&mut self, s: &Hp2PropertyLinkServices) -> Result<(), i32> {
        self.field = unsafe { pointer(s, self.field, NEXT)? };
        unsafe { self.seek(s) }
    }
}

/// # Safety
/// All objects and callbacks must remain live; callbacks must capture exceptions.
pub unsafe fn layout(
    root: Object,
    ar: Object,
    props: i32,
    native_offsets: &[i32; 5],
    s: &Hp2PropertyLinkServices,
) -> Result<(), i32> {
    unsafe {
        if props != 0 {
            (s.set_integer)(root, SIZE, 0);
            (s.set_integer)(root, ALIGN, 4);
            if !pointer(s, root, SUPER)?.is_null() {
                status((s.preload)(ar, pointer(s, root, SUPER)?))?;
                (s.set_integer)(root, SIZE, (s.integer)(pointer(s, root, SUPER)?, SIZE));
                (s.set_integer)(root, ALIGN, (s.integer)(pointer(s, root, SUPER)?, ALIGN));
            }
        }
        let mut previous = null_mut();
        let mut field = pointer(s, root, CHILDREN)?;
        while !field.is_null() {
            if props == 0 && pointer(s, field, OUTER)? != root {
                break;
            }
            status((s.preload)(ar, field))?;
            if props != 0 && pointer(s, field, OUTER)? != root {
                break;
            }
            if (s.kind)(field, PROPERTY) != 0 {
                let saved = (s.integer)(field, OFFSET);
                status((s.link)(field, ar, previous))?;
                if props == 0 || (s.integer)(field, CPP) != 0 {
                    (s.set_integer)(field, OFFSET, saved);
                }
                if props != 0 {
                    let native = (s.native_member)(root, field);
                    if native >= 0 {
                        (s.set_integer)(field, OFFSET, native_offsets[native as usize]);
                    }
                    (s.set_integer)(
                        root,
                        ALIGN,
                        (s.integer)(root, ALIGN).max((s.integer)(field, PROPERTY_ALIGN)),
                    );
                    let end =
                        (s.integer)(field, OFFSET).wrapping_add((s.integer)(field, PROPERTY_SIZE));
                    (s.set_integer)(root, SIZE, (s.integer)(root, SIZE).max(end));
                }
                previous = field;
            }
            field = pointer(s, field, NEXT)?;
        }
        if props != 0 {
            let align = (s.integer)(root, ALIGN);
            let size = (s.integer)(root, SIZE).wrapping_add(align.wrapping_sub(1))
                & !align.wrapping_sub(1);
            (s.set_integer)(root, SIZE, size);
        }
    }
    Ok(())
}

/// # Safety
/// Same live-storage and exception-bridge contract as `layout`.
pub unsafe fn reference_chain(
    root: Object,
    chain: i32,
    s: &Hp2PropertyLinkServices,
) -> Result<(), i32> {
    unsafe {
        let mask = if chain == REFERENCE_CHAIN {
            OBJECT_PROPERTY
        } else {
            STRUCT_PROPERTY
        };
        let mut it = Fields::new(root, mask, s)?;
        let mut previous = null_mut();
        while !it.field.is_null() {
            (s.set_chain)(root, previous, chain, it.field);
            previous = it.field;
            it.advance(s)?;
        }
        (s.set_chain)(root, previous, chain, null_mut());
    }
    Ok(())
}

const CONDITION_MAP_TAG: [crate::ansi::Tchar; 1] = [0];

#[derive(Clone, Copy)]
struct ConditionEnd {
    property: Object,
    end: i32,
    next: i32,
}

// Only POD lives in these allocations; no native container or typed destructor
// is retained. The existing map policy supplies lookup, insertion and rehashing.
struct ConditionEnds {
    pairs: *mut c_void,
    num: i32,
    max: i32,
    hash: *mut i32,
    unpublished_hash: *mut c_void,
    hash_count: i32,
    key: Object,
    end: i32,
    arrays: Hp2ArrayServices,
}

impl ConditionEnds {
    unsafe fn new(arrays: Hp2ArrayServices) -> Result<Self, i32> {
        let mut map = Self {
            pairs: null_mut(),
            num: 0,
            max: 0,
            hash: null_mut(),
            unpublished_hash: null_mut(),
            hash_count: 8,
            key: null_mut(),
            end: 0,
            arrays,
        };
        // SAFETY: Empty local slots and POD callbacks satisfy map storage's contract.
        unsafe { map.run(map_storage::operation::REHASH)? };
        Ok(map)
    }

    unsafe fn run(&mut self, operation: i32) -> Result<i32, i32> {
        let map = self as *mut Self;
        // SAFETY: Derive disjoint slot pointers from one owner pointer. Callbacks
        // use raw field accesses, never reborrow the entire owner over these slots.
        let slots = unsafe {
            Hp2MapSlots {
                hash: ptr::addr_of_mut!((*map).hash),
                count: ptr::addr_of_mut!((*map).hash_count),
            }
        };
        let services = Hp2MapServices {
            context: map.cast(),
            leaf: condition_map_leaf,
            query: condition_map_query,
            allocator: Hp2ArrayServices {
                context: map.cast(),
                realloc: condition_hash_realloc,
                checked_int_size: condition_hash_checked_size,
                free: condition_hash_free,
            },
            tag: CONDITION_MAP_TAG.as_ptr().cast(),
        };
        // SAFETY: All slots and callbacks address this exclusively owned local map.
        let result = unsafe { map_storage::run(slots, services, operation) };
        if self.unpublished_hash == self.hash.cast() {
            self.unpublished_hash = null_mut();
        }
        result
    }

    unsafe fn set(&mut self, key: Object, end: i32) -> Result<(), i32> {
        self.key = key;
        self.end = end;
        // SAFETY: The request operands remain unchanged until the operation returns.
        unsafe { self.run(map_storage::operation::SET).map(|_| ()) }
    }

    unsafe fn get(&mut self, key: Object) -> Result<i32, i32> {
        self.key = key;
        // SAFETY: FIND returns either a valid pair index or -1.
        let index = unsafe { self.run(map_storage::operation::FIND)? };
        if index < 0 {
            return Err(1);
        }
        // SAFETY: The map is local and no callback runs between lookup and this read.
        Ok(unsafe { (*self.pairs.cast::<ConditionEnd>().add(index as usize)).end })
    }

    fn release(&mut self) -> Result<(), i32> {
        let hash = std::mem::replace(&mut self.hash, null_mut());
        let unpublished_hash = std::mem::replace(&mut self.unpublished_hash, null_mut());
        let pairs = std::mem::replace(&mut self.pairs, null_mut());
        // SAFETY: Each allocation is released once by its original allocator.
        // Cleanup also runs after errors; native leaves retain the first exception.
        unsafe {
            let unpublished_result = if unpublished_hash.is_null() {
                Ok(())
            } else {
                status((self.arrays.free)(self.arrays.context, unpublished_hash))
            };
            let hash_result = if hash.is_null() {
                Ok(())
            } else {
                status((self.arrays.free)(self.arrays.context, hash.cast()))
            };
            let pairs_result = if pairs.is_null() {
                Ok(())
            } else {
                status((self.arrays.free)(self.arrays.context, pairs))
            };
            unpublished_result.and(hash_result).and(pairs_result)
        }
    }
}

impl Drop for ConditionEnds {
    fn drop(&mut self) {
        let _ = self.release();
    }
}

// Rehash can fail while freeing the old buckets, after allocating but before
// publishing the replacement. Keep that allocation owned without changing the
// shared native map policy's partial-publication behavior.
unsafe extern "C" fn condition_hash_realloc(
    context: Object,
    data: Object,
    bytes: usize,
    max: *const i32,
    width: i32,
    tag: *const c_void,
    out: *mut Object,
) -> i32 {
    // SAFETY: This allocator is used only for local map bucket allocations.
    unsafe {
        let map = context.cast::<ConditionEnds>();
        let arrays = ptr::addr_of!((*map).arrays).read();
        let result = (arrays.realloc)(arrays.context, data, bytes, max, width, tag, out);
        if result == 0 {
            ptr::addr_of_mut!((*map).unpublished_hash).write(out.read());
        }
        result
    }
}

unsafe extern "C" fn condition_hash_checked_size(
    context: Object,
    bytes: usize,
    out: *mut i32,
) -> i32 {
    // SAFETY: The copied engine allocator remains live throughout the operation.
    unsafe {
        let arrays = ptr::addr_of!((*context.cast::<ConditionEnds>()).arrays).read();
        (arrays.checked_int_size)(arrays.context, bytes, out)
    }
}

unsafe extern "C" fn condition_hash_free(context: Object, data: Object) -> i32 {
    // SAFETY: The bucket allocation belongs to the copied engine allocator.
    unsafe {
        let arrays = ptr::addr_of!((*context.cast::<ConditionEnds>()).arrays).read();
        (arrays.free)(arrays.context, data)
    }
}

unsafe extern "C" fn condition_map_leaf(
    context: Object,
    operation: i32,
    index: i32,
    argument: i32,
    out: *mut i32,
) -> i32 {
    // SAFETY: Only map_storage calls this leaf with live local slots and indices.
    unsafe {
        let map = context.cast::<ConditionEnds>();
        if operation == map_storage::leaf::NUM {
            out.write(ptr::addr_of!((*map).num).read());
            return 0;
        }
        let pair = ptr::addr_of!((*map).pairs).read().cast::<ConditionEnd>().add(index as usize);
        out.write(match operation {
            map_storage::leaf::NEXT => ptr::addr_of!((*pair).next).read(),
            map_storage::leaf::LINK => {
                ptr::addr_of_mut!((*pair).next).write(argument);
                0
            }
            map_storage::leaf::HASH => (ptr::addr_of!((*pair).property).read().addr() >> 4) as i32,
            _ => return 1,
        });
    }
    0
}

unsafe extern "C" fn condition_map_query(
    context: Object,
    operation: i32,
    index: i32,
    _: i32,
    out: *mut i32,
) -> i32 {
    // SAFETY: map_storage supplies valid indices; append uses the same fallible
    // allocation policy as native POD arrays and publishes the pair only on success.
    unsafe {
        let map = context.cast::<ConditionEnds>();
        let result = match operation {
            map_storage::query::HASH => (ptr::addr_of!((*map).key).read().addr() >> 4) as i32,
            map_storage::query::CONSTRUCT => {
                let index = match array_storage::add(
                    Hp2ArraySlots {
                        data: ptr::addr_of_mut!((*map).pairs),
                        num: ptr::addr_of_mut!((*map).num),
                        max: ptr::addr_of_mut!((*map).max),
                    },
                    1,
                    std::mem::size_of::<ConditionEnd>() as i32,
                    CONDITION_MAP_TAG.as_ptr().cast(),
                    ptr::addr_of!((*map).arrays).read(),
                ) {
                    Ok(index) => index,
                    Err(error) => return error,
                };
                ptr::addr_of!((*map).pairs).read().cast::<ConditionEnd>().add(index as usize).write(
                    ConditionEnd {
                        property: ptr::addr_of!((*map).key).read(),
                        end: ptr::addr_of!((*map).end).read(),
                        next: -1,
                    },
                );
                index
            }
            map_storage::query::KEY_EQ => {
                let pair = ptr::addr_of!((*map).pairs).read().cast::<ConditionEnd>().add(index as usize);
                i32::from(ptr::addr_of!((*pair).property).read() == ptr::addr_of!((*map).key).read())
            }
            map_storage::query::ASSIGN => {
                let pair = ptr::addr_of!((*map).pairs).read().cast::<ConditionEnd>().add(index as usize);
                ptr::addr_of_mut!((*pair).end).write(ptr::addr_of!((*map).end).read());
                0
            }
            _ => return 1,
        };
        out.write(result);
    }
    0
}

/// # Safety
/// Root, callbacks and allocator must remain live. On error no chain tails are
/// cleared and no earlier mutations are rolled back; temporary storage is freed.
pub unsafe fn cleanup_chains(
    root: Object,
    arrays: Hp2ArrayServices,
    s: &Hp2PropertyLinkServices,
) -> Result<(), i32> {
    // SAFETY: The caller supplies the engine allocator for this call-local POD owner.
    let mut ends = unsafe { ConditionEnds::new(arrays)? };
    let result = (|| unsafe {
        let mut property = null_mut();
        let mut config = null_mut();
        let mut constructor = null_mut();
        let mut it = Fields::new(root, PROPERTY, s)?;
        while !it.field.is_null() {
            let current = it.field;
            if (s.integer)(current, FLAGS) & NET != 0 && (s.editor)() == 0 {
                (s.set_rep_owner)(current, current);
                let mut end = 0;
                status((s.condition_end)(current, &mut end))?;
                ends.set(current, end)?;
                let mut earlier = Fields::new(root, PROPERTY, s)?;
                while earlier.field != current {
                    let candidate = earlier.field;
                    if (s.integer)(candidate, FLAGS) & NET != 0 {
                        let candidate_end = ends.get(candidate)?;
                        let len = end.wrapping_sub((s.integer)(current, REP_OFFSET));
                        if len == candidate_end.wrapping_sub((s.integer)(candidate, REP_OFFSET)) {
                            let mut equal = 0;
                            status((s.condition_equal)(current, candidate, len, &mut equal))?;
                            if equal != 0 {
                                (s.set_rep_owner)(candidate, pointer(s, current, REP_OWNER)?);
                            }
                        }
                    }
                    earlier.advance(s)?;
                }
            }
            if (s.integer)(current, FLAGS) & CTOR != 0 {
                (s.set_chain)(root, constructor, CTOR_CHAIN, current);
                constructor = current;
            }
            if (s.integer)(current, FLAGS) & CONFIG != 0 {
                (s.set_chain)(root, config, CONFIG_CHAIN, current);
                config = current;
            }
            (s.set_chain)(root, property, PROPERTY_CHAIN, current);
            property = current;
            it.advance(s)?;
        }
        (s.set_chain)(root, property, PROPERTY_CHAIN, null_mut());
        (s.set_chain)(root, config, CONFIG_CHAIN, null_mut());
        (s.set_chain)(root, constructor, CTOR_CHAIN, null_mut());
        Ok(())
    })();
    result.and(ends.release())
}

#[repr(C)]
pub struct Hp2ClassBindServices {
    pub check: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub has_constructor: unsafe extern "C" fn(*mut c_void) -> i32,
    pub native: unsafe extern "C" fn(*mut c_void) -> i32,
    pub editor: unsafe extern "C" fn() -> i32,
    pub superclass: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub lookup: unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> i32,
    pub accept_export: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i32,
    pub missing_export: unsafe extern "C" fn(*mut c_void) -> i32,
    pub bind: unsafe extern "C" fn(*mut c_void) -> i32,
    pub inherit: unsafe extern "C" fn(*mut c_void, *mut c_void),
}

/// # Safety
/// Native export lookup may register constructors as a side effect. Callbacks
/// must expose current values, never a pre-lookup constructor snapshot.
pub unsafe fn bind(class: Object, s: &Hp2ClassBindServices) -> Result<(), i32> {
    unsafe {
        status((s.check)(class, 0))?;
        if (s.has_constructor)(class) == 0 && (s.native)(class) != 0 {
            let mut export = null_mut();
            status((s.lookup)(class, &mut export))?;
            if !export.is_null() {
                status((s.accept_export)(class, export))?;
            } else if (s.editor)() == 0 {
                status((s.missing_export)(class))?;
            }
        }
        if (s.has_constructor)(class) == 0 && !(s.superclass)(class).is_null() {
            status((s.bind)((s.superclass)(class)))?;
            (s.inherit)(class, (s.superclass)(class));
            crate::class_ops::inherit_actor_ops(class, (s.superclass)(class));
        }
        status((s.check)(class, 1))
    }
}

#[repr(C)]
pub struct Hp2ClassNetworkServices {
    pub compare: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut i32) -> i32,
    /// Native Sort guard, reentering `sort_network_fields` over live class slots.
    pub sort_guard: unsafe extern "C" fn(*mut c_void) -> i32,
}

unsafe fn compare(s: &Hp2ClassNetworkServices, a: Object, b: Object) -> Result<i32, i32> {
    let mut result = 0;
    status(unsafe { (s.compare)(a, b, &mut result) })?;
    Ok(result)
}

/// Exact native Sort partition/selection order, including ties and error prefixes.
///
/// # Safety
/// `class` must address live canonical NetFields. Comparisons may reenter and
/// relocate its buffer, but must preserve the original live index range.
pub unsafe fn sort_network_fields(
    class: *mut UClass,
    s: &Hp2ClassNetworkServices,
) -> Result<(), i32> {
    // SAFETY: every access reloads the canonical buffer after any callback.
    unsafe {
        let slots = network_slots(ptr::addr_of_mut!((*class).net_fields));
        crate::sort::quicksort(
            slots.num.read() as isize,
            |_| Ok(()),
            |i, j| {
                let fields = slots.data.read_unaligned().cast::<Object>();
                let a = fields.add(i as usize).read_unaligned();
                let b = fields.add(j as usize).read_unaligned();
                compare(s, a, b)
            },
            |i, j| {
                let fields = slots.data.read_unaligned().cast::<Object>();
                let a = fields.add(i as usize).read_unaligned();
                let b = fields.add(j as usize).read_unaligned();
                fields.add(i as usize).write_unaligned(b);
                fields.add(j as usize).write_unaligned(a);
            },
        )
    }
}

const NETWORK_TAG: [u32; 1] = [0];

unsafe fn network_slots(array: *mut TArrayRaw) -> Hp2ArraySlots {
    // SAFETY: callers supply a live canonical array header.
    unsafe {
        Hp2ArraySlots {
            data: ptr::addr_of_mut!((*array).data),
            num: ptr::addr_of_mut!((*array).num),
            max: ptr::addr_of_mut!((*array).max),
        }
    }
}

unsafe fn add_network_field(
    slots: Hp2ArraySlots,
    field: Object,
    arrays: Hp2ArrayServices,
) -> Result<(), i32> {
    // SAFETY: fields are trivial pointer elements; reload storage after allocation.
    unsafe {
        let index = array_storage::add(
            slots, 1, size_of::<Object>() as i32, NETWORK_TAG.as_ptr().cast(), arrays,
        )?;
        slots.data.read_unaligned().cast::<Object>().add(index as usize).write_unaligned(field);
        Ok(())
    }
}

unsafe fn copy_network_reps(
    class: *mut UClass,
    parent: *mut UClass,
    arrays: Hp2ArrayServices,
) -> Result<(), i32> {
    // SAFETY: direct TArray assignment empties with source slack, then constructs
    // each element. Reload source count and buffers across allocator reentry;
    // the obsolete native adapter's conditional-expression temporary is gone.
    unsafe {
        if class == parent {
            return Ok(());
        }
        let target = network_slots(ptr::addr_of_mut!((*class).class_reps));
        let count = if parent.is_null() { 0 } else { (*parent).class_reps.num };
        let width = size_of::<FRepRecord>() as i32;
        array_storage::empty(target, count, width, NETWORK_TAG.as_ptr().cast(), arrays)?;
        let mut i = 0;
        while !parent.is_null() && i < (*parent).class_reps.num {
            let index = array_storage::add(target, 1, width, NETWORK_TAG.as_ptr().cast(), arrays)?;
            let source = ptr::addr_of!((*parent).class_reps.data).read_unaligned().cast::<FRepRecord>().add(i as usize);
            let destination = target.data.read_unaligned().cast::<FRepRecord>().add(index as usize);
            // The opaque trailing field is not Rust padding and may be
            // uninitialized. Only the two native members may be materialized.
            ptr::addr_of_mut!((*destination).property)
                .write_unaligned(ptr::addr_of!((*source).property).read_unaligned());
            ptr::addr_of_mut!((*destination).index)
                .write_unaligned(ptr::addr_of!((*source).index).read_unaligned());
            i += 1;
        }
        Ok(())
    }
}

/// # Safety
/// Called after native Super::Link. Canonical class arrays and allocator must
/// remain live. Callbacks may relocate arrays but must preserve active elements.
pub unsafe fn network(
    class: Object,
    arrays: Hp2ArrayServices,
    s: &Hp2PropertyLinkServices,
    n: &Hp2ClassNetworkServices,
) -> Result<(), i32> {
    unsafe {
        if (s.editor)() != 0 {
            return Ok(());
        }
        let canonical = class.cast::<UClass>();
        let fields = network_slots(ptr::addr_of_mut!((*canonical).net_fields));
        let reps = network_slots(ptr::addr_of_mut!((*canonical).class_reps));
        array_storage::empty(fields, 0, size_of::<Object>() as i32, NETWORK_TAG.as_ptr().cast(), arrays)?;
        copy_network_reps(canonical, pointer(s, class, CLASS_SUPER)?.cast(), arrays)?;
        let mut it = Fields::new(class, 0, s)?;
        while !it.field.is_null() && pointer(s, it.field, OWNER_CLASS)? == class {
            let field = it.field;
            if (s.kind)(field, PROPERTY) != 0 {
                if (s.integer)(field, FLAGS) & NET != 0 {
                    add_network_field(fields, field, arrays)?;
                    if pointer(s, field, OUTER)? == class {
                        (s.set_integer)(field, REP_INDEX, reps.num.read() as u16 as i32);
                        let mut index = 0;
                        while index < (s.integer)(field, ARRAY_DIM) {
                            let slot = array_storage::add(
                                reps, 1, size_of::<FRepRecord>() as i32,
                                NETWORK_TAG.as_ptr().cast(), arrays,
                            )?;
                            let record = reps.data.read_unaligned().cast::<FRepRecord>().add(slot as usize);
                            ptr::addr_of_mut!((*record).property).write_unaligned(field);
                            ptr::addr_of_mut!((*record).index).write_unaligned(index);
                            index += 1;
                        }
                    }
                }
            } else if (s.kind)(field, FUNCTION) != 0
                && (s.integer)(field, FUNCTION_FLAGS) & 0x40 != 0
                && pointer(s, field, SUPER_FUNCTION)?.is_null()
            {
                add_network_field(fields, field, arrays)?;
            }
            it.advance(s)?;
        }
        array_storage::shrink(fields, size_of::<Object>() as i32, NETWORK_TAG.as_ptr().cast(), arrays)?;
        if fields.num.read() > 1 {
            status((n.sort_guard)(class))?;
        }
        Ok(())
    }
}
/// Direct layout and link-chain construction for one Rust-owned UStruct.
///
/// # Safety
/// `structure` and every field in its child/super chains are live canonical
/// records. Preload and archive calls may reenter, so no borrowed field data is
/// retained across either operation.
/// Finds the reflected UClass that owns `structure`, if `structure` is itself
/// a class object or is nested below one.
unsafe fn reflected_owner_class(
    access: crate::core_access::CoreAccess<'_>,
    structure: *mut UStruct,
) -> crate::core_access::RuntimeResult<*mut UClass> {
    let metaclass = access.native_class("Class")?;
    let mut object = structure.cast::<hp2_abi::engine::generated::UObject>();
    let mut class = unsafe { ptr::addr_of!((*object).class).read_unaligned() };
    if class == metaclass.cast() {
        return Ok(object.cast());
    }
    object = unsafe { ptr::addr_of!((*object).outer).read_unaligned() }.cast();
    while !object.is_null() {
        class = unsafe { ptr::addr_of!((*object).class).read_unaligned() };
        if class == metaclass.cast() {
            return Ok(object.cast());
        }
        object = unsafe { ptr::addr_of!((*object).outer).read_unaligned() }.cast();
    }
    Ok(ptr::null_mut())
}

unsafe fn authored_native_offset(
    access: crate::core_access::CoreAccess<'_>,
    class: *mut UClass,
    property: *mut UProperty,
) -> crate::core_access::RuntimeResult<i32> {
    use crate::core_access::RuntimeError;
    let name = i32::from_ne_bytes(unsafe {
        ptr::addr_of!((*property.cast::<hp2_abi::engine::generated::UObject>()).name)
            .read_unaligned()
    });
    let owner = unsafe {
        ptr::addr_of!((*property.cast::<hp2_abi::engine::generated::UObject>()).outer)
            .read_unaligned()
    };
    if let Some(offset) = access.cpp_property_offset(owner.cast(), name) {
        return Ok(offset);
    }
    if let Some(fields) = access.startup_struct_fields(owner.cast()) {
        for declaration in fields {
            if access.intern_str(declaration.name, crate::name_owner::FindName::Find)? != name {
                continue;
            }
            let expected_kind = match declaration.kind {
                crate::core_class_specs::StartupScalarKind::Byte =>
                    crate::core_class_specs::NativeRecordKind::ByteProperty,
                crate::core_class_specs::StartupScalarKind::Int =>
                    crate::core_class_specs::NativeRecordKind::IntProperty,
                crate::core_class_specs::StartupScalarKind::Float =>
                    crate::core_class_specs::NativeRecordKind::FloatProperty,
            };
            let actual_kind = unsafe { access.record_kind(property.cast())? };
            let array_dim = unsafe { ptr::addr_of!((*property).array_dim).read_unaligned() };
            if actual_kind != expected_kind || array_dim != 1 {
                return Err(RuntimeError::new("Native struct member metadata mismatch"));
            }
            return i32::try_from(declaration.offset)
                .map_err(|_| RuntimeError::new("Native struct member offset exceeds INT"));
        }
    }
    if class.is_null() {
        return Err(RuntimeError::new("Native property has no owning class"));
    }
    let metadata = unsafe { access.class_metadata(class)? };
    let spec = metadata.native_spec
        .ok_or_else(|| RuntimeError::new("Native property class has no static declaration"))?;
    for candidate in spec.properties {
        if access.intern_str(candidate.name, crate::name_owner::FindName::Find)? == name {
            return i32::try_from(candidate.offset)
                .map_err(|_| RuntimeError::new("Native property offset exceeds INT"));
        }
    }
    let property_name: String = access.name_text(name)?.into_iter()
        .map(|unit| char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect();
    let owner_name = if owner.is_null() {
        "<null>".to_owned()
    } else {
        let owner_index = i32::from_ne_bytes(unsafe {
            ptr::addr_of!((*owner.cast::<hp2_abi::engine::generated::UObject>()).name)
                .read_unaligned()
        });
        access.name_text(owner_index)?.into_iter()
            .map(|unit| char::from_u32(unit).unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect()
    };
    Err(RuntimeError::new("Native property missing from static declaration")
        .with_context(&spec.short_name)
        .with_context(&owner_name)
        .with_context(&property_name))
}

const OWNED_LAYOUT_ABI: Hp2PropertyTypeLayoutAbi = Hp2PropertyTypeLayoutAbi {
    property_alignment: hp2_abi::engine::generated::CORE_PROPERTY_ALIGNMENT,
    byte_size: size_of::<u8>() as i32,
    int_size: size_of::<i32>() as i32,
    bool_size: size_of::<u32>() as i32,
    float_size: size_of::<f32>() as i32,
    object_size: size_of::<*mut UObject>() as i32,
    name_size: size_of::<hp2_abi::engine::generated::FName>() as i32,
    string_size: size_of::<TArrayRaw>() as i32,
    array_size: size_of::<TArrayRaw>() as i32,
    map_size: size_of::<crate::core_records::MapStorageRaw>() as i32,
    object_alignment: std::mem::align_of::<*mut UObject>() as i32,
    name_alignment: std::mem::align_of::<hp2_abi::engine::generated::FName>() as i32,
    string_alignment: std::mem::align_of::<TArrayRaw>() as i32,
    array_alignment: std::mem::align_of::<TArrayRaw>() as i32,
    map_alignment: std::mem::align_of::<crate::core_records::MapStorageRaw>() as i32,
    first_bitfield: 1,
};

fn owned_layout_kind(
    access: crate::core_access::CoreAccess<'_>, property: Object,
) -> crate::core_access::RuntimeResult<Hp2PropertyTypeLayoutKind> {
    use crate::core_class_specs::NativeRecordKind as K;
    use Hp2PropertyTypeLayoutKind::*;
    Ok(match unsafe { access.record_kind(property.cast())? } {
        K::ByteProperty => TypeLayoutByte,
        K::IntProperty => TypeLayoutInt,
        K::BoolProperty => TypeLayoutBool,
        K::FloatProperty => TypeLayoutFloat,
        K::ObjectProperty | K::ClassProperty => TypeLayoutObject,
        K::NameProperty => TypeLayoutName,
        K::StringProperty => TypeLayoutString,
        K::FixedArrayProperty => TypeLayoutFixedArray,
        K::ArrayProperty => TypeLayoutArray,
        K::MapProperty => TypeLayoutMap,
        K::StructProperty => TypeLayoutStruct,
        _ => return Err(crate::core_access::RuntimeError::new("Invalid concrete property layout kind")),
    })
}

struct OwnedTypeLayoutOps<'access, 'archive, 'data> {
    access: crate::core_access::CoreAccess<'access>,
    archive: Option<&'archive crate::object_archive::ObjectArchive<'data>>,
}

impl TypeLayoutOps for OwnedTypeLayoutOps<'_, '_, '_> {
    type Error = crate::core_access::RuntimeError;
    fn is_kind(&self, property: Object, kind: Hp2PropertyTypeLayoutKind) -> Result<bool, Self::Error> {
        Ok(owned_layout_kind(self.access, property)? as i32 == kind as i32)
    }
    fn owner_size(&self, property: Object) -> Result<i32, Self::Error> {
        use crate::core_class_specs::NativeRecordKind as K;
        let owner = unsafe { (*property.cast::<UObject>()).outer.cast::<UObject>() };
        Ok(match unsafe { self.access.record_kind(owner)? } {
            K::Struct | K::State | K::Class | K::Function => unsafe { (*owner.cast::<UStruct>()).properties_size },
            _ => 0,
        })
    }
    fn owner_merge_bools(&self, property: Object) -> Result<bool, Self::Error> {
        let owner = unsafe { (*property.cast::<UObject>()).outer.cast::<UObject>() };
        Ok(unsafe { self.access.record_kind(owner)? } != crate::core_class_specs::NativeRecordKind::Function)
    }
    fn preload(&self, object: Object) -> Result<(), Self::Error> {
        if self.archive.is_some() {
            unsafe { crate::core_loader::preload(self.access, object.cast())?; }
        }
        Ok(())
    }
    fn link(&self, property: Object) -> Result<(), Self::Error> {
        let kind = owned_layout_kind(self.access, property)?;
        unsafe { type_layout_with(property.cast(), null_mut(), kind, &OWNED_LAYOUT_ABI, self) }
    }
    fn checked_size(&self, bytes: u64) -> Result<i32, Self::Error> {
        i32::try_from(bytes).map_err(|_| Self::Error::new("Reflected property size exceeds INT"))
    }
}

/// The native Object loading-layout fixup binds script-visible identity fields
/// to the canonical prefix, independently of UProperty::CppProperty.
fn owned_object_member_offset(
    access: crate::core_access::CoreAccess<'_>,
    property: *mut UProperty,
) -> crate::core_access::RuntimeResult<Option<i32>> {
    const MEMBERS: [(&str, usize); 5] = [
        ("ObjectInternal", std::mem::offset_of!(UObject, index)),
        ("Outer", std::mem::offset_of!(UObject, outer)),
        ("ObjectFlags", std::mem::offset_of!(UObject, object_flags)),
        ("Name", std::mem::offset_of!(UObject, name)),
        ("Class", std::mem::offset_of!(UObject, class)),
    ];
    let name = i32::from_ne_bytes(unsafe { (*property.cast::<UObject>()).name });
    for (member, offset) in MEMBERS {
        if access.intern_str(member, crate::name_owner::FindName::Find)? == name {
            return Ok(Some(offset as i32));
        }
    }
    Ok(None)
}

pub(crate) unsafe fn link_struct_owned(
    access: crate::core_access::CoreAccess<'_>,
    structure: *mut UStruct,
    archive: Option<&crate::object_archive::ObjectArchive<'_>>,
    link_properties: bool,
) -> crate::core_access::RuntimeResult<()> {
    use crate::core_class_specs::NativeRecordKind as K;
    use crate::core_access::RuntimeError;

    if structure.is_null() {
        return Err(RuntimeError::new("Cannot link null UStruct"));
    }
    unsafe {
        let mut size = 0i32;
        let mut alignment = 4i32;
        let mut super_field = if access.record_kind(structure.cast())? == K::Function {
            ptr::null_mut()
        } else {
            ptr::addr_of!((*structure.cast::<UField>()).super_field).read_unaligned()
        };
        if !super_field.is_null() && link_properties {
            if let Some(archive) = archive {
                crate::core_loader::preload(access, super_field.cast())?;
                let _ = archive;
            }
            let parent = super_field.cast::<UStruct>();
            size = ptr::addr_of!((*parent).properties_size).read_unaligned();
            alignment = ptr::addr_of!((*parent).properties_align).read_unaligned().max(4);
        }
        let owner_class = unsafe { reflected_owner_class(access, structure)? };
        let object_loading_layout = link_properties
            && ptr::addr_of!((*access.metadata_storage()).loading_layout).read() == structure.cast()
            && structure.cast::<UClass>() == access.native_class("Object")?;
        let layout_ops = OwnedTypeLayoutOps { access, archive };

        let mut property_head: *mut UProperty = ptr::null_mut();
        let mut previous_property: *mut UProperty = ptr::null_mut();
        let mut field = ptr::addr_of!((*structure).children).read_unaligned().cast::<UField>();
        while !field.is_null() {
            if ptr::addr_of!((*field.cast::<UObject>()).outer).read_unaligned() != structure.cast() {
                break;
            }
            if link_properties {
                ptr::addr_of_mut!((*structure).properties_size).write_unaligned(size);
                ptr::addr_of_mut!((*structure).properties_align).write_unaligned(alignment);
            }
            if let Some(archive) = archive {
                crate::core_loader::preload(access, field.cast())?;
                let _ = archive;
            }
            let kind = access.record_kind(field.cast())?;
            if matches!(
                kind,
                K::Property | K::IntProperty | K::FloatProperty | K::BoolProperty
                    | K::ByteProperty | K::ObjectProperty | K::ClassProperty
                    | K::NameProperty | K::StringProperty | K::FixedArrayProperty
                    | K::ArrayProperty | K::MapProperty | K::StructProperty
            ) {
                let property = field.cast::<UProperty>();
                let dim = ptr::addr_of!((*property).array_dim).read_unaligned();
                let saved_offset = ptr::addr_of!((*property).offset).read_unaligned();
                let layout_kind = owned_layout_kind(access, property.cast())?;
                type_layout_with(property, previous_property.cast(), layout_kind, &OWNED_LAYOUT_ABI, &layout_ops)?;
                let element_size = ptr::addr_of!((*property).element_size).read_unaligned();
                let alignment_for_property = type_alignment_with(property, &OWNED_LAYOUT_ABI, &layout_ops)?;
                if dim <= 0
                    || element_size <= 0
                    || alignment_for_property <= 0
                    || (alignment_for_property & (alignment_for_property - 1)) != 0
                {
                    return Err(RuntimeError::new("Invalid reflected property layout"));
                }
                let native = ptr::addr_of!((*property).cpp_property).read_unaligned() != 0;
                if native {
                    let offset = authored_native_offset(access, owner_class, property)?;
                    ptr::addr_of_mut!((*property).offset).write_unaligned(offset);
                } else if !link_properties {
                    ptr::addr_of_mut!((*property).offset).write_unaligned(saved_offset);
                }
                if link_properties {
                    if object_loading_layout {
                        if let Some(offset) = owned_object_member_offset(access, property)? {
                            ptr::addr_of_mut!((*property).offset).write_unaligned(offset);
                        }
                    }
                    let offset = ptr::addr_of!((*property).offset).read_unaligned();
                    let span = dim.checked_mul(element_size)
                        .ok_or_else(|| RuntimeError::new("Reflected property layout overflow"))?;
                    let end = offset.checked_add(span)
                        .filter(|_| offset >= 0)
                        .ok_or_else(|| RuntimeError::new("Invalid reflected property offset"))?;
                    ptr::addr_of_mut!((*property).offset).write_unaligned(offset);
                    size = size.max(end);
                    alignment = alignment.max(alignment_for_property);
                }
                if !previous_property.is_null() {
                    ptr::addr_of_mut!((*previous_property).property_link_next)
                        .write_unaligned(property.cast());
                } else {
                    property_head = property;
                }
                previous_property = property;
            }
            field = ptr::addr_of!((*field).next).read_unaligned().cast();
        }
        let mut owner = super_field.cast::<UStruct>();
        while !owner.is_null() {
            let mut inherited = ptr::addr_of!((*owner).children)
                .read_unaligned()
                .cast::<UField>();
            while !inherited.is_null() {
                if ptr::addr_of!((*inherited.cast::<UObject>()).outer).read_unaligned()
                    != owner.cast()
                {
                    break;
                }
                if let Some(archive) = archive {
                    crate::core_loader::preload(access, inherited.cast())?;
                    let _ = archive;
                }
                let inherited_kind = access.record_kind(inherited.cast())?;
                if matches!(
                    inherited_kind,
                    K::Property | K::IntProperty | K::FloatProperty | K::BoolProperty
                        | K::ByteProperty | K::ObjectProperty | K::ClassProperty
                        | K::NameProperty | K::StringProperty | K::FixedArrayProperty
                        | K::ArrayProperty | K::MapProperty | K::StructProperty
                ) {
                    let property = inherited.cast::<UProperty>();
                    if !previous_property.is_null() {
                        ptr::addr_of_mut!((*previous_property).property_link_next)
                            .write_unaligned(property.cast());
                    } else {
                        property_head = property;
                    }
                    previous_property = property;
                }
                inherited = ptr::addr_of!((*inherited).next).read_unaligned().cast();
            }
            owner = ptr::addr_of!((*owner.cast::<UField>()).super_field)
                .read_unaligned()
                .cast::<UStruct>();
        }
        ptr::addr_of_mut!((*structure).property_link).write_unaligned(property_head.cast());
        if !previous_property.is_null() {
            ptr::addr_of_mut!((*previous_property).property_link_next).write_unaligned(ptr::null_mut());
        }
        if link_properties {
            ptr::addr_of_mut!((*structure).properties_align).write_unaligned(alignment);
            let rounded = size.checked_add(alignment - 1)
                .ok_or_else(|| RuntimeError::new("Reflected property layout overflow"))?
                & !(alignment - 1);
            let minimum = if access.record_kind(structure.cast())? == K::Class {
                size_of::<UObject>() as i32
            } else {
                0
            };
            ptr::addr_of_mut!((*structure).properties_size)
                .write_unaligned(rounded.max(minimum));
        }

        ptr::addr_of_mut!((*structure).ref_link).write_unaligned(ptr::null_mut());
        ptr::addr_of_mut!((*structure).struct_link).write_unaligned(ptr::null_mut());
        ptr::addr_of_mut!((*structure).config_link).write_unaligned(ptr::null_mut());
        ptr::addr_of_mut!((*structure).constructor_link).write_unaligned(ptr::null_mut());
        let mut reference_tail: *mut c_void = ptr::null_mut();
        let mut struct_tail: *mut c_void = ptr::null_mut();
        let mut config_tail: *mut UProperty = ptr::null_mut();
        let mut constructor_tail: *mut UProperty = ptr::null_mut();
        let mut owner = structure;
        loop {
            let mut child = ptr::addr_of!((*owner).children).read_unaligned().cast::<UField>();
            while !child.is_null() {
                let child_owner = ptr::addr_of!((*child.cast::<UObject>()).outer).read_unaligned();
                if child_owner != owner.cast() { break; }
                let field_kind = access.record_kind(child.cast())?;
                if matches!(field_kind, K::ObjectProperty | K::ClassProperty) {
                    let property = child.cast::<hp2_abi::engine::generated::UObjectProperty>();
                    if reference_tail.is_null() {
                        ptr::addr_of_mut!((*structure).ref_link).write_unaligned(child.cast());
                    } else {
                        ptr::addr_of_mut!((*reference_tail.cast::<hp2_abi::engine::generated::UObjectProperty>()).next_reference)
                            .write_unaligned(child.cast());
                    }
                    reference_tail = child.cast();
                    let _ = property;
                } else if field_kind == K::StructProperty {
                    if struct_tail.is_null() {
                        ptr::addr_of_mut!((*structure).struct_link).write_unaligned(child.cast());
                    } else {
                        ptr::addr_of_mut!((*struct_tail.cast::<UStructProperty>()).next_struct)
                            .write_unaligned(child.cast());
                    }
                    struct_tail = child.cast();
                }
                if matches!(
                    field_kind,
                    K::Property | K::IntProperty | K::FloatProperty | K::BoolProperty
                        | K::ByteProperty | K::ObjectProperty | K::ClassProperty
                        | K::NameProperty | K::StringProperty | K::FixedArrayProperty
                        | K::ArrayProperty | K::MapProperty | K::StructProperty
                ) {
                    let property = child.cast::<UProperty>();
                    let flags = ptr::addr_of!((*property).property_flags).read_unaligned();
                    if flags & CONFIG as u32 != 0 {
                        if config_tail.is_null() {
                            ptr::addr_of_mut!((*structure).config_link).write_unaligned(property.cast());
                        } else {
                            ptr::addr_of_mut!((*config_tail).config_link_next).write_unaligned(property.cast());
                        }
                        config_tail = property;
                    }
                    if flags & CTOR as u32 != 0 {
                        if constructor_tail.is_null() {
                            ptr::addr_of_mut!((*structure).constructor_link).write_unaligned(property.cast());
                        } else {
                            ptr::addr_of_mut!((*constructor_tail).constructor_link_next).write_unaligned(property.cast());
                        }
                        constructor_tail = property;
                    }
                }
                child = ptr::addr_of!((*child).next).read_unaligned().cast();
            }
            let super_struct = if access.record_kind(owner.cast())? == K::Function {
                ptr::null_mut()
            } else {
                ptr::addr_of!((*owner.cast::<UField>()).super_field)
                    .read_unaligned().cast::<UStruct>()
            };
            if super_struct.is_null() { break; }
            owner = super_struct;
        }
        if !reference_tail.is_null() {
            ptr::addr_of_mut!((*reference_tail.cast::<hp2_abi::engine::generated::UObjectProperty>()).next_reference)
                .write_unaligned(ptr::null_mut());
        }
        if !struct_tail.is_null() {
            ptr::addr_of_mut!((*struct_tail.cast::<UStructProperty>()).next_struct).write_unaligned(ptr::null_mut());
        }
        if !config_tail.is_null() {
            ptr::addr_of_mut!((*config_tail).config_link_next).write_unaligned(ptr::null_mut());
        }
        if !constructor_tail.is_null() {
            ptr::addr_of_mut!((*constructor_tail).constructor_link_next).write_unaligned(ptr::null_mut());
        }
        if matches!(access.record_kind(structure.cast())?, K::State | K::Class) {
            let state = structure.cast::<hp2_abi::engine::generated::UState>();
            let parent = ptr::addr_of!((*state.cast::<UField>()).super_field)
                .read_unaligned()
                .cast::<hp2_abi::engine::generated::UState>();
            let hash = ptr::addr_of_mut!((*state).vf_hash);
            if parent.is_null() {
                ptr::write_bytes(
                    hash.cast::<u8>(),
                    0,
                    size_of::<*mut c_void>() * crate::class_metadata::METADATA_HASH_COUNT as usize,
                );
            } else {
                ptr::copy(
                    ptr::addr_of!((*parent).vf_hash),
                    hash,
                    1,
                );
            }
            let mut owner = structure;
            let mut child = ptr::addr_of!((*owner).children).read_unaligned().cast::<UField>();
            loop {
                while child.is_null() {
                    let next_owner = ptr::addr_of!((*owner.cast::<UField>()).super_field)
                        .read_unaligned()
                        .cast::<UStruct>();
                    if next_owner.is_null() {
                        break;
                    }
                    owner = next_owner;
                    child = ptr::addr_of!((*owner).children).read_unaligned().cast();
                }
                if child.is_null()
                    || ptr::addr_of!((*child.cast::<UObject>()).outer).read_unaligned()
                        != structure.cast()
                {
                    break;
                }
                let child_kind = access.record_kind(child.cast())?;
                if matches!(
                    child_kind,
                    K::Struct | K::State | K::Class | K::Function | K::Enum
                ) {
                    let name = i32::from_ne_bytes(
                        ptr::addr_of!((*child.cast::<UObject>()).name).read_unaligned(),
                    );
                    let bucket = name & (crate::class_metadata::METADATA_HASH_COUNT - 1);
                    let bucket_ptr = ptr::addr_of_mut!((*state).vf_hash)
                        .cast::<*mut c_void>()
                        .add(bucket as usize);
                    ptr::addr_of_mut!((*child).hash_next)
                        .write_unaligned(bucket_ptr.read_unaligned());
                    bucket_ptr.write_unaligned(child.cast());
                }
                child = ptr::addr_of!((*child).next).read_unaligned().cast();
            }
        }
    }
    Ok(())
}
