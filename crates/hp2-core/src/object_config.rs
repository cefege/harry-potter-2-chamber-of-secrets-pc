//! Configuration/localization traversal and deferred-default policy.
//! Native callbacks own property values, config sections and scratch text. In
//! particular, section names are not copied: the legacy circular scratch aliasing
//! remains observable when an ImportText callback synchronously reenters Core.
use crate::object_collections::PendingLocalized;
use std::ffi::c_void;
use std::ptr;
type Result<T, E = i32> = std::result::Result<T, E>;
const CLASS_CONFIG: usize = 0x4;
const CLASS_LOCALIZED: usize = 0x20;
const CLASS_PER_OBJECT: usize = 0x400;
/// Native CPF_Config flag shared by canonical reflected-property consumers.
pub const CPF_CONFIG_FLAG: u32 = 0x4000;
const CPF_LOCALIZED: usize = 0x8000;
const CPF_GLOBAL: usize = 0x40000;

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2ObjectConfigOperation {
    Load,
    Save,
    Localized,
    FlushLocalized,
    Reset,
}

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2ObjectConfigLeaf {
    Class,
    ClassFlags,
    Super,
    Outer,
    Index,
    Name,
    Path,
    ConfigName,
    Transient,
    SetFilename,
    SetSection,
    SetPackage,
    LoadSuper,
    LocalizedSuper,
    Children,
    InheritanceSuper,
    NextField,
    IsProperty,
    PropertyFlags,
    OwnerClass,
    SelectProperty,
    ArrayDim,
    PropertyKind,
    PropertyKey,
    IndexedKey,
    GetSection,
    FindValues,
    ClearValues,
    PrepareArray,
    DestroyArray,
    AllocateArray,
    ImportArray,
    ReadScalar,
    ImportScalar,
    RemoveKey,
    ArrayCount,
    ExportArray,
    AddArray,
    ExportScalar,
    SetScalar,
    FlushConfig,
    NextClassIndex,
    ObjectCount,
    ObjectAt,
    ChildOf,
    ReloadDefault,
    Editor,
    Localize,
    HasText,
    ImportLocalized,
    DefaultsCount,
    PropertiesSize,
    LocalizedDefault,
    IsSystemConfig,
    IsUserConfig,
    ResetSource,
    ResetReadSection,
    ResetSetKey,
    NextObjectIndex,
    IsA,
    ReloadObject,
    PostEditChange,
    ResetSectionCount,
    ResetEntryKey,
    ResetEntryValue,
}

#[repr(C)]
pub struct Hp2ObjectConfigArgs {
    pub object: usize,
    pub object_class: usize,
    pub filename: usize,
    pub flags: u32,
    pub propagate: i32,
}

#[repr(C)]
pub struct Hp2ObjectConfigServices {
    pub context: *mut std::ffi::c_void,
    pub call: unsafe extern "C" fn(
        *mut std::ffi::c_void,
        Hp2ObjectConfigLeaf,
        usize,
        usize,
        *mut usize,
    ) -> i32,
}

struct Native<'a>(&'a Hp2ObjectConfigServices, *mut PendingLocalized);
impl Native<'_> {
    fn call(&self, leaf: Hp2ObjectConfigLeaf, a: usize, b: usize) -> Result<usize> {
        let mut result = 0;
        let status = unsafe { (self.0.call)(self.0.context, leaf, a, b, &mut result) };
        if status == 0 { Ok(result) } else { Err(status) }
    }
    fn defer(&self, class: usize) -> Result<()> {
        unsafe {
            if !(*self.1).contains(class as *mut c_void) {
                (*self.1).append(class as *mut c_void);
            }
        }
        Ok(())
    }
    fn filename(&self, q: &Hp2ObjectConfigArgs) -> Result<bool> {
        use Hp2ObjectConfigLeaf::*;
        let class = self.call(Class, q.object, 0)?;
        let per_object = self.call(ClassFlags, class, 0)? & CLASS_PER_OBJECT != 0
            && self.call(Index, q.object, 0)? as i32 != -1;
        let filename = if q.filename != 0 {
            q.filename
        } else if per_object && self.call(Outer, q.object, 0)? != self.call(Transient, 0, 0)? {
            self.call(Name, self.call(Outer, q.object, 0)?, 0)?
        } else {
            self.call(ConfigName, self.call(Class, q.object, 0)?, 0)?
        };
        self.call(SetFilename, filename, 0)?;
        Ok(per_object)
    }
    fn section(
        &self,
        object: usize,
        class: usize,
        property: usize,
        per_object: bool,
    ) -> Result<()> {
        use Hp2ObjectConfigLeaf::*;
        let section = if per_object {
            self.call(Name, object, 0)?
        } else {
            let base = if self.call(PropertyFlags, property, 0)? & CPF_GLOBAL != 0 {
                self.call(OwnerClass, property, 0)?
            } else {
                class
            };
            self.call(Path, base, 0)?
        };
        self.call(SetSection, section, 0)?;
        Ok(())
    }
    fn properties(
        &self,
        mut class: usize,
        mut visit: impl FnMut(usize) -> Result<()>,
    ) -> Result<()> {
        use Hp2ObjectConfigLeaf::*;
        // Match TFieldIterator's live inherited chain, including lazily materialized
        // fields and mutation of Next during an import/export callback.
        while class != 0 {
            let mut field = self.call(Children, class, 0)?;
            while field != 0 {
                if self.call(IsProperty, field, 0)? != 0 {
                    visit(field)?;
                }
                field = self.call(NextField, field, 0)?;
            }
            class = self.call(InheritanceSuper, class, 0)?;
        }
        Ok(())
    }
    fn load(&self, q: &Hp2ObjectConfigArgs) -> Result<()> {
        use Hp2ObjectConfigLeaf::*;
        let class = if q.object_class != 0 {
            q.object_class
        } else {
            self.call(Class, q.object, 0)?
        };
        if self.call(ClassFlags, class, 0)? & CLASS_CONFIG == 0 {
            return Ok(());
        }
        if q.propagate != 0 {
            let parent = self.call(Super, class, 0)?;
            if parent != 0 {
                self.call(LoadSuper, parent, q.propagate as usize)?;
            }
        }
        let per_object = self.filename(q)?;
        self.properties(class, |p| {
            if self.call(PropertyFlags, p, 0)? & CPF_CONFIG_FLAG as usize == 0 {
                return Ok(());
            }
            self.section(q.object, class, p, per_object)?;
            self.call(SelectProperty, p, 0)?;
            let kind = self.call(PropertyKind, 0, 0)?;
            if kind != 0 {
                if self.call(GetSection, 0, 0)? != 0 {
                    let count = self.call(FindValues, 0, 0)?;
                    if kind == 1 {
                        self.call(PrepareArray, 0, 0)?;
                        self.call(DestroyArray, 0, 0)?;
                        self.call(AllocateArray, count, 0)?;
                        for dest in 0..count {
                            self.call(ImportArray, count - dest - 1, dest)?;
                        }
                    }
                    // Map loading intentionally reads the MultiFind list without
                    // importing it: that is the engine's legacy map behavior.
                    self.call(ClearValues, 0, 0)?;
                }
            } else {
                let mut index = 0;
                while index < self.call(ArrayDim, p, 0)? {
                    if self.call(ArrayDim, p, 0)? != 1 {
                        self.call(IndexedKey, index, 0)?;
                    }
                    if self.call(ReadScalar, 0, 0)? != 0 {
                        self.call(ImportScalar, index, 0)?;
                    }
                    index += 1;
                }
            }
            Ok(())
        })
    }
    fn save(&self, q: &Hp2ObjectConfigArgs) -> Result<()> {
        use Hp2ObjectConfigLeaf::*;
        let per_object = self.filename(q)?;
        let class = self.call(Class, q.object, 0)?;
        self.properties(class, |p| {
            if self.call(PropertyFlags, p, 0)? & q.flags as usize != q.flags as usize {
                return Ok(());
            }
            self.section(q.object, self.call(Class, q.object, 0)?, p, per_object)?;
            self.call(SelectProperty, p, 1)?;
            let kind = self.call(PropertyKind, 0, 0)?;
            if kind != 0 {
                self.call(GetSection, 1, 0)?;
                self.call(RemoveKey, 0, 0)?;
                if kind == 1 {
                    self.call(PrepareArray, 0, 0)?;
                    let mut index = 0;
                    while index < self.call(ArrayCount, 0, 0)? {
                        self.call(ExportArray, index, 0)?;
                        self.call(AddArray, 0, 0)?;
                        index += 1;
                    }
                }
                // Map saving deletes the key and deliberately emits no entries.
            } else {
                let mut index = 0;
                while index < self.call(ArrayDim, p, 0)? {
                    if self.call(ArrayDim, p, 0)? != 1 {
                        self.call(IndexedKey, index, 0)?;
                    }
                    self.call(ExportScalar, index, 0)?;
                    self.call(SetScalar, 0, 0)?;
                    index += 1;
                }
            }
            Ok(())
        })?;
        self.call(FlushConfig, 0, 0)?;
        let mut base = self.call(Class, q.object, 0)?;
        loop {
            let parent = self.call(Super, base, 0)?;
            if parent == 0 || self.call(ClassFlags, parent, 0)? & CLASS_CONFIG == 0 {
                break;
            }
            base = parent;
        }
        if base != 0 {
            let mut index = self.call(NextClassIndex, (-1i32) as usize, 0)?;
            while index < self.call(ObjectCount, 0, 0)? {
                let class = self.call(ObjectAt, index, 0)?;
                if self.call(ChildOf, class, base)? != 0 {
                    self.call(ReloadDefault, class, 0)?;
                }
                index = self.call(NextClassIndex, index, 0)?;
            }
        }
        Ok(())
    }
    fn reset(&self, q: &Hp2ObjectConfigArgs) -> Result<()> {
        use Hp2ObjectConfigLeaf::*;
        let class = q.object_class;
        let source = if self.call(IsSystemConfig, class, 0)? != 0 {
            0
        } else if self.call(IsUserConfig, class, 0)? != 0 {
            1
        } else {
            return Ok(());
        };
        self.call(ResetSource, source, 0)?;
        // Retain the historical key-only admission limit, but never let an
        // admitted oversized value overflow the original inline buffer.
        const SECTION_CAPACITY: usize = 32767;
        let mut buffer = std::mem::MaybeUninit::<[i32; SECTION_CAPACITY]>::uninit();
        let inline = buffer.as_mut_ptr().cast::<i32>();
        let mut spill: Option<Vec<i32>> = None;
        let mut length = 0;
        if self.call(ResetReadSection, 0, 0)? != 0 {
            // SAFETY: typed native entries supply live NUL-terminated strings.
            // Snapshot everything before SetString can mutate native maps.
            unsafe {
                for index in 0..self.call(ResetSectionCount, 0, 0)? {
                    let key = self.call(ResetEntryKey, index, 0)? as *const i32;
                    let key_len = libc::wcslen(key);
                    if length + key_len + 1 >= SECTION_CAPACITY {
                        break;
                    }
                    let value = self.call(ResetEntryValue, index, 0)? as *const i32;
                    let value_len = libc::wcslen(value);
                    let needed = length + key_len + value_len + 2;
                    if needed + 1 > SECTION_CAPACITY {
                        // needed >= SECTION_CAPACITY: after this entry the
                        // admission predicate rejects even an empty next key.
                        // Consequently spill is always the final snapshot entry.
                        let mut owned = Vec::with_capacity(needed + 1);
                        owned.extend_from_slice(std::slice::from_raw_parts(inline, length));
                        owned.extend_from_slice(std::slice::from_raw_parts(key, key_len));
                        owned.push(i32::from(b'='));
                        owned.extend_from_slice(std::slice::from_raw_parts(value, value_len));
                        owned.push(0);
                        spill = Some(owned);
                    } else {
                        std::ptr::copy_nonoverlapping(key, inline.add(length), key_len);
                        *inline.add(length + key_len) = i32::from(b'=');
                        std::ptr::copy_nonoverlapping(
                            value, inline.add(length + key_len + 1), value_len,
                        );
                        *inline.add(needed - 1) = 0;
                    }
                    length = needed;
                }
                let start = if let Some(owned) = spill.as_mut() {
                    owned.push(0);
                    owned.as_mut_ptr()
                } else {
                    *inline.add(length) = 0;
                    inline
                };
                let mut key = start;
                while *key != 0 {
                    let mut end = key;
                    let mut separator: *mut i32 = std::ptr::null_mut();
                    while *end != 0 {
                        if *end == i32::from(b'=') && separator.is_null() {
                            separator = end;
                        }
                        end = end.add(1);
                    }
                    let next = end.add(1);
                    if !separator.is_null() {
                        *separator = 0;
                        self.call(ResetSetKey, key as usize, separator.add(1) as usize)?;
                    }
                    key = next;
                }
            }
        }
        // These are live iterators: reload callbacks may add/remove objects.
        let mut index = self.call(NextClassIndex, (-1i32) as usize, 0)?;
        while index < self.call(ObjectCount, 0, 0)? {
            let candidate = self.call(ObjectAt, index, 0)?;
            if self.call(ChildOf, candidate, class)? != 0 {
                self.call(ReloadDefault, candidate, 0)?;
            }
            index = self.call(NextClassIndex, index, 0)?;
        }
        let mut index = self.call(NextObjectIndex, (-1i32) as usize, 0)?;
        while index < self.call(ObjectCount, 0, 0)? {
            let object = self.call(ObjectAt, index, 0)?;
            if self.call(IsA, object, class)? != 0 {
                self.call(ReloadObject, object, 0)?;
                self.call(PostEditChange, object, 0)?;
            }
            index = self.call(NextObjectIndex, index, 0)?;
        }
        Ok(())
    }
    fn localized(&self, q: &Hp2ObjectConfigArgs) -> Result<()> {
        use Hp2ObjectConfigLeaf::*;
        let class = if q.object_class != 0 {
            q.object_class
        } else {
            self.call(Class, q.object, 0)?
        };
        if self.call(ClassFlags, class, 0)? & CLASS_LOCALIZED == 0 {
            return Ok(());
        }
        if self.call(Children, class, 0)? == 0 {
            let parent = self.call(Super, class, 0)?;
            if parent != 0 && self.call(Children, parent, 0)? == 0 {
                return self.defer(class);
            }
        }
        if self.call(Editor, 0, 0)? != 0 {
            return self.defer(class);
        }
        if q.propagate != 0 {
            let parent = self.call(Super, class, 0)?;
            if parent != 0 {
                self.call(LocalizedSuper, parent, q.propagate as usize)?;
            }
        }
        let owner = if self.call(Index, q.object, 0)? as i32 == -1 {
            class
        } else {
            q.object
        };
        let package = self.call(Name, self.call(Outer, owner, 0)?, 0)?;
        self.call(SetPackage, package, 0)?;
        let section = self.call(Name, owner, 0)?;
        self.call(SetSection, section, 0)?;
        let mut count = 0;
        self.properties(class, |p| {
            if self.call(PropertyFlags, p, 0)? & CPF_LOCALIZED == 0 {
                return Ok(());
            }
            count += 1;
            self.call(SelectProperty, p, 1)?;
            let mut index = 0;
            while index < self.call(ArrayDim, p, 0)? {
                self.call(PropertyKey, 0, 0)?;
                if self.call(ArrayDim, p, 0)? != 1 {
                    self.call(IndexedKey, index, 0)?;
                }
                self.call(Localize, 0, 0)?;
                if self.call(HasText, 0, 0)? != 0 {
                    self.call(ImportLocalized, index, 0)?;
                }
                index += 1;
            }
            Ok(())
        })?;
        if count == 0 && self.call(Super, class, 0)? != 0 {
            self.defer(class)?;
        }
        Ok(())
    }
    fn flush_localized(&self) -> Result<()> {
        use Hp2ObjectConfigLeaf::*;
        let count = unsafe { (*self.1).len() };
        for _ in 0..count {
            // Nested LoadLocalized calls can drain the live queue, or append
            // entries. Keep the original bound but never index an empty queue.
            if unsafe { (*self.1).len() } == 0 {
                break;
            }
            let pending = unsafe { (*self.1).remove_first() as usize };
            let bytes = self.call(DefaultsCount, pending, 0)?;
            if bytes == 0 || bytes != self.call(PropertiesSize, pending, 0)? {
                unsafe {
                    (*self.1).append(pending as *mut c_void);
                }
                continue;
            }
            self.call(LocalizedDefault, pending, 0)?;
        }
        Ok(())
    }
}

/// # Safety
/// The native state and handles must remain live through synchronous callbacks.
/// Pending storage is engine-thread serialized and may be reentered by leaves.
pub unsafe fn execute(
    services: &Hp2ObjectConfigServices,
    op: Hp2ObjectConfigOperation,
    q: &Hp2ObjectConfigArgs,
    pending: *mut PendingLocalized,
) -> Result<()> {
    let n = Native(services, pending);
    match op {
        Hp2ObjectConfigOperation::Load => n.load(q),
        Hp2ObjectConfigOperation::Save => n.save(q),
        Hp2ObjectConfigOperation::Localized => n.localized(q),
        Hp2ObjectConfigOperation::FlushLocalized => n.flush_localized(),
        Hp2ObjectConfigOperation::Reset => n.reset(q),
    }
}

#[cfg(test)]
mod tests;
impl crate::core_access::CoreAccess<'_> {
    /// Reads a value from the default System.ini cache.
    pub(crate) fn config_get(self, section: &[u32], key: &[u32]) -> crate::core_access::RuntimeResult<Option<Vec<u32>>> {
        unsafe { &mut *self.config_storage() }.get(None, section, key)
    }

    /// Sets a value in the default System.ini cache.
    pub(crate) fn config_set(self, section: &[u32], key: &[u32], value: &[u32]) -> crate::core_access::RuntimeResult<()> {
        unsafe { &mut *self.config_storage() }.set(None, section, key, value)
    }

    /// Saves selected reflected config properties and reloads affected defaults.
    ///
    /// # Safety
    /// `object` is live and its class, defaults, and linked property records remain valid.
    pub unsafe fn save_config(
        self,
        object: *mut hp2_abi::engine::generated::UObject,
        flags: u32,
        filename_override: Option<&[u32]>,
    ) -> crate::core_access::RuntimeResult<()> {
        use crate::core_access::RuntimeError;
        use hp2_abi::engine::generated::{UArrayProperty, UClass, UField, UProperty, UStruct, TArrayRaw};
        if object.is_null() { return Err(RuntimeError::new("Null config object")); }
        let class = unsafe { (*object).class.cast::<UClass>() };
        if class.is_null() { return Err(RuntimeError::new("Config object has no class")); }
        let per_object = unsafe { (*class).class_flags } & CLASS_PER_OBJECT as u32 != 0 && unsafe { (*object).index } != -1;
        let outer = unsafe { (*object).outer.cast::<hp2_abi::engine::generated::UObject>() };
        let filename = if let Some(filename) = filename_override {
            filename.to_vec()
        } else if per_object && outer != self.transient_package() {
            self.name_text(i32::from_ne_bytes(unsafe { (*outer).name }))?
        } else {
            self.name_text(i32::from_ne_bytes(unsafe { (*class).class_config_name }))?
        };
        let default = unsafe { self.default_object(class)? };
        let mut structure = class.cast::<UStruct>();
        while !structure.is_null() {
            let mut field = unsafe { (*structure).children.cast::<UField>() };
            while !field.is_null() {
                let property = field.cast::<UProperty>();
                let kind = self.record_kind(field.cast())?;
                if is_property_kind(kind) && unsafe { (*property).property_flags & flags } == flags {
                    let property_name = self.name_text(i32::from_ne_bytes(unsafe { (*field.cast::<hp2_abi::engine::generated::UObject>()).name }))?;
                    let section = if per_object {
                        self.name_text(i32::from_ne_bytes(unsafe { (*object).name }))?
                    } else {
                        let base = if unsafe { (*property).property_flags } & CPF_GLOBAL as u32 != 0 {
                            unsafe { (*property.cast::<hp2_abi::engine::generated::UObject>()).outer.cast::<UClass>() }
                        } else { class };
                        class_path(self, base.cast())?
                    };
                    if matches!(kind, crate::core_class_specs::NativeRecordKind::ArrayProperty | crate::core_class_specs::NativeRecordKind::MapProperty) {
                        unsafe { &mut *self.config_storage() }.remove_key(Some(&filename), &section, &property_name)?;
                        if kind == crate::core_class_specs::NativeRecordKind::ArrayProperty {
                            let address = unsafe { self.property_address(object, property)? };
                            let default_address = unsafe { self.property_address(default, property)? };
                            let array = unsafe { ptr::read_unaligned(address.cast::<TArrayRaw>()) };
                            let defaults = unsafe { ptr::read_unaligned(default_address.cast::<TArrayRaw>()) };
                            let inner = unsafe { (*property.cast::<UArrayProperty>()).inner.cast::<UProperty>() };
                            if inner.is_null() { return Err(RuntimeError::new("Array property has no inner property")); }
                            let width = usize::try_from(unsafe { (*inner).element_size })
                                .ok().filter(|width| *width != 0)
                                .ok_or_else(|| RuntimeError::new("Invalid array element size"))?;
                            if array.num < 0 || (array.num > 0 && array.data.is_null()) {
                                return Err(RuntimeError::new("Invalid dynamic array storage"));
                            }
                            for index in 0..array.num {
                                let value = unsafe { array.data.cast::<u8>().add(index as usize * width) };
                                let default_value = if index < defaults.num && !defaults.data.is_null() {
                                    unsafe { defaults.data.cast::<u8>().add(index as usize * width) }
                                } else { ptr::null_mut() };
                                let mut text = Vec::new();
                                unsafe { self.property_export_text_at(inner, value, default_value, 0, &mut text)?; }
                                unsafe { &mut *self.config_storage() }.append_value(Some(&filename), &section, &property_name, &text)?;
                            }
                        }
                    } else {
                        let dimension = unsafe { (*property).array_dim.max(0) };
                        let address = unsafe { self.property_address(object, property)? };
                        let default_address = unsafe { self.property_address(default, property)? };
                        for index in 0..dimension {
                            let key = indexed_key(self, &property_name, dimension, index)?;
                            let mut text = Vec::new();
                            unsafe { self.property_export_text_index(property, address, default_address, index, 0, &mut text)?; }
                            unsafe { &mut *self.config_storage() }.set(Some(&filename), &section, &key, &text)?;
                        }
                    }
                }
                field = unsafe { (*field).next.cast() };
            }
            structure = unsafe { (*structure.cast::<UField>()).super_field.cast::<UStruct>() };
        }
        unsafe { &mut *self.config_storage() }.flush(None, false)?;
        let mut base = class;
        loop {
            let parent = unsafe { (*base.cast::<UField>()).super_field.cast::<UClass>() };
            if parent.is_null() || unsafe { (*parent).class_flags } & CLASS_CONFIG as u32 == 0 { break; }
            base = parent;
        }
        if unsafe { (*base).class_flags } & CLASS_CONFIG as u32 != 0 {
            let mut index = -1;
            loop {
                unsafe { crate::object_storage::next_index(self.storage(), &mut index, ptr::null_mut(), any_config_object) }
                    .map_err(|_| RuntimeError::new("Object iteration failed"))?;
                if index >= self.object_count() { break; }
                let candidate = self.object_at(index);
                if !candidate.is_null()
                    && self.record_kind(candidate.cast())? == crate::core_class_specs::NativeRecordKind::Class
                    && class_is_child_of(candidate.cast(), base)
                {
                    let defaults = unsafe { self.default_object(candidate.cast())? };
                    unsafe { self.load_config_class(defaults, candidate.cast(), None, true)?; }
                }
            }
        }
        Ok(())
    }
    /// Loads configured properties for a canonical object, including inherited defaults.
    ///
    /// # Safety
    /// `object` is a live canonical object with a fully linked class hierarchy.
    pub unsafe fn load_config(self, object: *mut hp2_abi::engine::generated::UObject) -> crate::core_access::RuntimeResult<()> {
        if object.is_null() { return Err(crate::core_access::RuntimeError::new("Null config object")); }
        unsafe { self.load_config_class(object, ptr::null_mut(), None, false) }
    }

    unsafe fn load_config_class(
        self,
        object: *mut hp2_abi::engine::generated::UObject,
        class_override: *mut hp2_abi::engine::generated::UClass,
        filename: Option<Vec<u32>>,
        propagate: bool,
    ) -> crate::core_access::RuntimeResult<()> {
        use crate::core_access::RuntimeError;
        use hp2_abi::engine::generated::{UClass, UField, UProperty, UStruct};
        let class = if class_override.is_null() {
            unsafe { (*object).class.cast::<UClass>() }
        } else { class_override };
        if class.is_null() { return Err(RuntimeError::new("Config object has no class")); }
        if unsafe { (*class).class_flags } & CLASS_CONFIG as u32 == 0 { return Ok(()); }
        if propagate {
            let parent = unsafe { (*class).super_field.cast::<UClass>() };
            if !parent.is_null() {
                unsafe { self.load_config_class(object, parent, filename.clone(), true)?; }
            }
        }
        let per_object = unsafe { (*class).class_flags } & CLASS_PER_OBJECT as u32 != 0
            && unsafe { (*object).index } != -1;
        let filename = if per_object
            && unsafe { (*object).outer.cast::<hp2_abi::engine::generated::UObject>() } != self.transient_package()
        {
            let outer = unsafe { (*object).outer.cast::<hp2_abi::engine::generated::UObject>() };
            self.name_text(i32::from_ne_bytes(unsafe { (*outer).name }))?
        } else {
            self.name_text(i32::from_ne_bytes(unsafe { (*class).class_config_name }))?
        };
        let mut structure = class.cast::<UStruct>();
        while !structure.is_null() {
            let mut field = unsafe { (*structure).children.cast::<UField>() };
            while !field.is_null() {
                let property = field.cast::<UProperty>();
                if is_property_kind(self.record_kind(field.cast())?)
                    && unsafe { (*property).property_flags } & CPF_CONFIG_FLAG != 0
                {
                    let field_name = self.name_text(i32::from_ne_bytes(unsafe { (*field.cast::<hp2_abi::engine::generated::UObject>()).name }))?;
                    let section = if per_object {
                        self.name_text(i32::from_ne_bytes(unsafe { (*object).name }))?
                    } else {
                        let base = if unsafe { (*property).property_flags } & CPF_GLOBAL as u32 != 0 {
                            unsafe { (*property.cast::<hp2_abi::engine::generated::UObject>()).outer.cast::<UClass>() }
                        } else { class };
                        class_path(self, base.cast())?
                    };
                    let array_kind = self.record_kind(field.cast())?;
                    if array_kind == crate::core_class_specs::NativeRecordKind::ArrayProperty {
                        let config = unsafe { &mut *self.config_storage() };
                        let section_exists = config.has_section(Some(&filename), &section)?;
                        if section_exists {
                            let mut values = config.get_all(Some(&filename), &section, &field_name)?;
                            // The cache returns native hash-chain order (newest
                            // first); ImportArray reverses it into file order.
                            values.reverse();
                            let default = unsafe { self.default_object(class)? };
                            unsafe {
                                self.property_import_array_values(
                                    property.cast(),
                                    object,
                                    &values,
                                    default,
                                    0,
                                )?;
                            }
                        }
                    } else if array_kind != crate::core_class_specs::NativeRecordKind::MapProperty {
                        let dimension = unsafe { (*property).array_dim.max(0) };
                        let default = unsafe { self.default_object(class)? };
                        let value_address = unsafe { self.property_address(object, property)? };
                        let default_address = unsafe { self.property_address(default, property)? };
                        for index in 0..dimension {
                            let key = indexed_key(self, &field_name, dimension, index)?;
                            let value = unsafe { &mut *self.config_storage() }
                                .get(Some(&filename), &section, &key)?;
                            if let Some(value) = value {
                                let offset = index * unsafe { (*property).element_size };
                                let target = unsafe { value_address.offset(offset as isize) };
                                let default_value = unsafe { default_address.offset(offset as isize) };
                                unsafe {
                                    self.property_import_text_at(
                                        property,
                                        target,
                                        &value,
                                        default_value,
                                        0,
                                    )?;
                                }
                            }
                        }
                    }
                }
                field = unsafe { (*field).next.cast() };
            }
            structure = unsafe { (*structure.cast::<UField>()).super_field.cast::<UStruct>() };
        }
        Ok(())
    }

    /// Loads localized properties for one canonical object.
    ///
    /// # Safety
    /// `object` is live; its class hierarchy and property offsets are fully linked.
    pub unsafe fn load_localized(self, object: *mut hp2_abi::engine::generated::UObject) -> crate::core_access::RuntimeResult<()> {
        use hp2_abi::engine::generated::{UClass, UField, UProperty, UStruct};
        if object.is_null() { return Err(crate::core_access::RuntimeError::new("Null localized object")); }
        let class = unsafe { (*object).class.cast::<UClass>() };
        if class.is_null() || unsafe { (*class).class_flags } & CLASS_LOCALIZED as u32 == 0 { return Ok(()); }
        if self.is_editor() { unsafe { self.queue_localized(class.cast()) }; return Ok(()); }
        if unsafe { (*class.cast::<UStruct>()).children.is_null() } {
            let parent = unsafe { (*class.cast::<UField>()).super_field.cast::<UStruct>() };
            if !parent.is_null() && unsafe { (*parent).children.is_null() } {
                unsafe { self.queue_localized(class.cast()) };
                return Ok(());
            }
        }
        let mut structure = class.cast::<UStruct>();
        let owner = if unsafe { (*object).index } == -1 { class.cast() } else { object };
        let package_outer = unsafe { (*owner).outer.cast::<hp2_abi::engine::generated::UObject>() };
        let package = if package_outer.is_null() { Vec::new() } else {
            self.name_text(i32::from_ne_bytes(unsafe { (*package_outer).name }))?
        };
        let section = self.name_text(i32::from_ne_bytes(unsafe { (*owner).name }))?;
        let mut localized_count = 0;
        while !structure.is_null() {
            let mut field = unsafe { (*structure).children.cast::<UField>() };
            while !field.is_null() {
                let property = field.cast::<UProperty>();
                if is_property_kind(self.record_kind(field.cast())?)
                    && unsafe { (*property).property_flags } & CPF_LOCALIZED as u32 != 0
                {
                    localized_count += 1;
                    let key_base = self.name_text(i32::from_ne_bytes(unsafe {
                        (*field.cast::<hp2_abi::engine::generated::UObject>()).name
                    }))?;
                    let dim = unsafe { (*property).array_dim.max(0) };
                    let default = unsafe { self.default_object(class)? };
                    let value_address = unsafe { self.property_address(object, property)? };
                    let default_address = unsafe { self.property_address(default, property)? };
                    for index in 0..dim {
                        let key = indexed_key(self, &key_base, dim, index)?;
                        let text = self.localize_text(&section, &key, Some(&package), None, true)?;
                        if !text.is_empty() {
                            let offset = index * unsafe { (*property).element_size };
                            unsafe {
                                self.property_import_text_at(
                                    property,
                                    value_address.offset(offset as isize),
                                    &text,
                                    default_address.offset(offset as isize),
                                    0,
                                )?;
                            }
                        }
                    }
                }
                field = unsafe { (*field).next.cast() };
            }
            structure = unsafe { (*structure.cast::<UField>()).super_field.cast::<UStruct>() };
        }
        if localized_count == 0 && !unsafe { (*class).super_field.is_null() } {
            unsafe { self.queue_localized(class.cast()) };
        }
        Ok(())
    }

    pub fn flush_localized(self) -> crate::core_access::RuntimeResult<()> {
        let start_count = self.pending_localized_count();
        for _ in 0..start_count {
            if self.pending_localized_count() == 0 { break; }
            let class_object = self.pending_localized_at(0);
            if class_object.is_null() { continue; }
            // Remove before any count lookup or callback; nested loads may drain the queue.
            if !self.take_pending_localized(0, class_object) { continue; }
            let class = class_object.cast::<hp2_abi::engine::generated::UClass>();
            let counts_match = unsafe {
                (*class).properties_size > 0 && (*class).defaults.num == (*class).properties_size
            };
            if !counts_match {
                unsafe { self.queue_localized(class_object); }
                continue;
            }
            let defaults = unsafe { self.default_object(class)? };
            unsafe { self.load_localized(defaults)?; }
        }
        Ok(())
    }

    /// Restores a class's config section from its matching default INI file.
    ///
    /// # Safety
    /// `class` is a live canonical class with linked defaults.
    pub unsafe fn reset_config(self, class: *mut hp2_abi::engine::generated::UClass) -> crate::core_access::RuntimeResult<()> {
        use crate::core_access::RuntimeError;
        if class.is_null() { return Err(RuntimeError::new("Null config class")); }
        let config_name = self.name_text(i32::from_ne_bytes(unsafe { (*class).class_config_name }))?;
        let source: &[u32] = if ascii_equal(&config_name, b"User") { &[68, 101, 102, 85, 115, 101, 114, 46, 105, 110, 105] }
            else if ascii_equal(&config_name, b"System") { &[68, 101, 102, 97, 117, 108, 116, 46, 105, 110, 105] }
            else { return Ok(()); };
        let section = class_path(self, class.cast())?;
        let pairs = unsafe { &mut *self.config_storage() }.section_pairs(Some(source), &section)?;
        for (key, value) in pairs {
            unsafe { &mut *self.config_storage() }.set(Some(&config_name), &section, &key, &value)?;
        }
        // Reset the selected class and its descendants, preserving the native
        // default-first ordering before propagating to live instances.
        let mut index = -1;
        loop {
            unsafe {
                crate::object_storage::next_index(
                    self.storage(),
                    &mut index,
                    ptr::null_mut(),
                    any_config_object,
                )
            }
            .map_err(|_| RuntimeError::new("Object iteration failed"))?;
            if index >= self.object_count() { break; }
            let candidate = self.object_at(index);
            if candidate.is_null()
                || self.record_kind(candidate.cast())? != crate::core_class_specs::NativeRecordKind::Class
            {
                continue;
            }
            let selected_class = candidate.cast::<hp2_abi::engine::generated::UClass>();
            if !class_is_child_of(selected_class, class) { continue; }
            let defaults = unsafe { self.default_object(selected_class)? };
            unsafe { self.load_config_class(defaults, selected_class, None, true)?; }
        }
        let mut index = -1;
        loop {
            unsafe {
                crate::object_storage::next_index(
                    self.storage(),
                    &mut index,
                    ptr::null_mut(),
                    any_config_object,
                )
            }
            .map_err(|_| RuntimeError::new("Object iteration failed"))?;
            if index >= self.object_count() { break; }
            let candidate = self.object_at(index);
            if candidate.is_null()
                || self.record_kind(candidate.cast())? == crate::core_class_specs::NativeRecordKind::Class
            {
                continue;
            }
            let candidate_class = unsafe { (*candidate).class.cast::<hp2_abi::engine::generated::UClass>() };
            if candidate_class.is_null() || !class_is_child_of(candidate_class, class) { continue; }
            unsafe { self.load_config_class(candidate, candidate_class, None, true)?; }
            unsafe { self.post_edit_change(candidate)?; }
        }
        Ok(())
    }
}

fn is_property_kind(kind: crate::core_class_specs::NativeRecordKind) -> bool {
    matches!(
        kind,
        crate::core_class_specs::NativeRecordKind::Property
            | crate::core_class_specs::NativeRecordKind::ByteProperty
            | crate::core_class_specs::NativeRecordKind::IntProperty
            | crate::core_class_specs::NativeRecordKind::BoolProperty
            | crate::core_class_specs::NativeRecordKind::FloatProperty
            | crate::core_class_specs::NativeRecordKind::ObjectProperty
            | crate::core_class_specs::NativeRecordKind::ClassProperty
            | crate::core_class_specs::NativeRecordKind::NameProperty
            | crate::core_class_specs::NativeRecordKind::StringProperty
            | crate::core_class_specs::NativeRecordKind::StructProperty
            | crate::core_class_specs::NativeRecordKind::FixedArrayProperty
            | crate::core_class_specs::NativeRecordKind::ArrayProperty
            | crate::core_class_specs::NativeRecordKind::MapProperty
    )
}
fn class_path(access: crate::core_access::CoreAccess<'_>, object: *mut hp2_abi::engine::generated::UObject) -> crate::core_access::RuntimeResult<Vec<u32>> {
    let mut chain = Vec::new();
    let mut current = object;
    while !current.is_null() {
        chain.push(access.name_text(i32::from_ne_bytes(unsafe { (*current).name }))?);
        current = unsafe { (*current).outer.cast() };
    }
    chain.reverse();
    let mut path = Vec::new();
    for (index, name) in chain.into_iter().enumerate() {
        if index != 0 { path.push(46); }
        path.extend(name);
    }
    Ok(path)
}

fn indexed_key(
    access: crate::core_access::CoreAccess<'_>,
    name: &[u32],
    dim: i32,
    index: i32,
) -> crate::core_access::RuntimeResult<Vec<u32>> {
    if dim == 1 { return Ok(name.to_vec()); }
    let mut key = name.to_vec();
    key.push(91);
    key.extend(index.to_string().bytes().map(u32::from));
    key.push(93);
    let _ = access;
    Ok(key)
}

fn ascii_equal(name: &[u32], ascii: &[u8]) -> bool {
    name.iter().copied().take_while(|&unit| unit != 0).map(|unit| {
        if (65..=90).contains(&unit) { unit + 32 } else { unit }
    }).eq(ascii.iter().map(|&byte| u32::from(byte.to_ascii_lowercase())))
}
unsafe extern "C" fn any_config_object(
    _: *mut c_void,
    _: *mut c_void,
    found: *mut i32,
) -> i32 {
    unsafe { found.write(1) };
    0
}

fn class_is_child_of(
    mut child: *mut hp2_abi::engine::generated::UClass,
    parent: *mut hp2_abi::engine::generated::UClass,
) -> bool {
    while !child.is_null() {
        if std::ptr::eq(child, parent) { return true; }
        child = unsafe { (*child).super_field.cast() };
    }
    false
}
