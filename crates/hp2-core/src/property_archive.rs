//! Binary/tagged property stream policy over live native reflection and archive leaves.
//! Legacy scopes keep C++ archives on their stack; direct scopes use synchronous Rust closures.

use std::ffi::c_void;
type Object = *mut c_void;
type Result<T = ()> = std::result::Result<T, i32>;
pub type Hp2PropertyArchiveContinuation = unsafe extern "C" fn(Object, Object) -> i32;

/// ABI view of a live native FPropertyTag. Native layout is asserted field-by-field.
#[repr(C)]
pub struct Hp2PropertyArchiveTag {
    pub property_type: u8,
    pub info: u8,
    pub name: i32,
    pub item_name: i32,
    pub size: i32,
    pub array_index: i32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2PropertyArchivePointer {
    ArchiveChildren,
    ArchiveNext,
    ArchiveSuper,
    ArchivePropertyLink,
    ArchivePropertyLinkNext,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2PropertyArchiveInteger {
    ArchiveName,
    ArchiveIsProperty,
    ArchiveArrayDim,
    ArchiveOffset,
    ArchiveElementSize,
    ArchiveType,
    ArchiveVersion,
    ArchiveLoading,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2PropertyArchiveName {
    ArchiveFloatParams,
    ArchiveColorParams,
    ArchiveRand,
    ArchiveAuxAnims,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hp2PropertyArchiveWarning {
    ArchiveMissingProperty,
    ArchiveTypeMismatch,
    ArchiveArrayBounds,
    ArchiveStructMismatch,
    ArchiveNotSerializable,
    ArchiveSkipping,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2PropertyArchiveServices {
    pub string_property: i32,
    pub str_property: i32,
    pub struct_property: i32,
    pub pointer: unsafe extern "C" fn(Object, Hp2PropertyArchivePointer, *mut Object) -> i32,
    pub integer: unsafe extern "C" fn(Object, Hp2PropertyArchiveInteger) -> i32,
    pub should_serialize: unsafe extern "C" fn(Object, Object) -> i32,
    pub name_matches: unsafe extern "C" fn(Object, Hp2PropertyArchiveName, *mut i32) -> i32,
    pub struct_name: unsafe extern "C" fn(Object, *mut i32) -> i32,
    pub defaults: unsafe extern "C" fn(Object, *mut Object, *mut i32) -> i32,
    pub note_name: unsafe extern "C" fn(Object, i32),
    pub note_index: unsafe extern "C" fn(Object, i32),
    pub serialize_item: unsafe extern "C" fn(Object, Object, Object) -> i32,
    pub matches: unsafe extern "C" fn(Object, Object, Object, i32, *mut i32) -> i32,
    pub stream: unsafe extern "C" fn(Object, i32, Hp2PropertyArchiveContinuation, Object) -> i32,
    pub load_tag_scope: unsafe extern "C" fn(Object, Hp2PropertyArchiveContinuation, Object) -> i32,
    pub save_tag_scope: unsafe extern "C" fn(
        Object,
        Object,
        i32,
        Object,
        Hp2PropertyArchiveContinuation,
        Object,
    ) -> i32,
    pub read_tag: unsafe extern "C" fn(Object, *mut *const Hp2PropertyArchiveTag) -> i32,
    pub write_tag: unsafe extern "C" fn(Object) -> i32,
    pub tagged_item: unsafe extern "C" fn(Object, Object, Object) -> i32,
    pub read_ansi_byte: unsafe extern "C" fn(Object, *mut u8) -> i32,
    pub assign_fixed_string: unsafe extern "C" fn(Object, Object, *const i32) -> i32,
    pub warn: unsafe extern "C" fn(Object, Object, Hp2PropertyArchiveWarning) -> i32,
    pub is_xlevel: unsafe extern "C" fn(Object) -> i32,
    pub read_byte: unsafe extern "C" fn(Object) -> i32,
    pub end_names: unsafe extern "C" fn(Object) -> i32,
}
/// Engine-thread leaves for binary and tagged property traversal.
///
/// Implementations use shared-reference capabilities. A successful scope hook
/// invokes its `FnOnce` body exactly once before returning; a failed scope may
/// return before invoking it. Hooks never retain a body or callback-provided
/// raw pointer beyond the synchronous operation or active scope.
pub trait PropertyArchiveOps {
    /// Error returned by an archive or property operation.
    type Error;

    /// Native property-type identity for serialized FString tags.
    fn string_property(&self) -> i32;
    /// Native property-type identity for `StrProperty`.
    fn str_property(&self) -> i32;
    /// Native property-type identity for `StructProperty`.
    fn struct_property(&self) -> i32;

    /// Resolve a reflected pointer relation.
    ///
    /// # Safety
    /// `object` must be live and valid for `field`.
    unsafe fn pointer(
        &self,
        object: Object,
        field: Hp2PropertyArchivePointer,
    ) -> std::result::Result<Object, Self::Error>;
    /// Read the requested archive/property integer.
    ///
    /// # Safety
    /// `object` must be live and valid for `field`.
    unsafe fn integer(
        &self,
        object: Object,
        field: Hp2PropertyArchiveInteger,
    ) -> std::result::Result<i32, Self::Error>;
    /// Query whether a property participates in serialization.
    ///
    /// # Safety
    /// Both pointers must be live for the property serialization query.
    unsafe fn should_serialize(
        &self,
        property: Object,
        context: Object,
    ) -> std::result::Result<bool, Self::Error>;
    /// Compare an object or property name against a catalog identity.
    ///
    /// # Safety
    /// `object` must be live for the name query.
    unsafe fn name_matches(
        &self,
        object: Object,
        name: Hp2PropertyArchiveName,
    ) -> std::result::Result<bool, Self::Error>;
    /// Return a struct property's reflected struct name.
    ///
    /// # Safety
    /// `property` must be a live struct property.
    unsafe fn struct_name(
        &self,
        property: Object,
    ) -> std::result::Result<i32, Self::Error>;
    /// Resolve the object's defaults record and byte count.
    ///
    /// # Safety
    /// `context` must be live and initialized for default lookup.
    unsafe fn defaults(
        &self,
        context: Object,
    ) -> std::result::Result<(Object, i32), Self::Error>;
    /// Record the archive-level diagnostic name.
    ///
    /// # Safety
    /// `context` must be the live root archive context, including inside tag scopes.
    unsafe fn note_name(
        &self,
        context: Object,
        name: i32,
    ) -> std::result::Result<(), Self::Error>;
    /// Record the archive-level array index.
    ///
    /// # Safety
    /// `context` must be the live root archive context, including inside tag scopes.
    unsafe fn note_index(
        &self,
        context: Object,
        index: i32,
    ) -> std::result::Result<(), Self::Error>;
    /// Serialize one binary property value.
    ///
    /// # Safety
    /// `context`, `property`, and `value` must satisfy the selected property's
    /// serialization contract.
    unsafe fn serialize_item(
        &self,
        context: Object,
        property: Object,
        value: Object,
    ) -> std::result::Result<(), Self::Error>;
    /// Compare a value with its defaults for tagged saving.
    ///
    /// # Safety
    /// All pointers must identify live property/default/value storage.
    unsafe fn matches(
        &self,
        property: Object,
        data: Object,
        defaults: Object,
        index: i32,
    ) -> std::result::Result<i32, Self::Error>;
    /// Read the next live tagged-property record.
    /// # Safety
    /// `context` must be a readable tagged archive. The returned raw tag is
    /// valid only during its active scope and must not be retained afterward.
    unsafe fn read_tag(
        &self,
        context: Object,
    ) -> std::result::Result<*const Hp2PropertyArchiveTag, Self::Error>;
    /// Write the current tagged-property header.
    ///
    /// # Safety
    /// `context` must be writable for a tag header.
    unsafe fn write_tag(&self, context: Object) -> std::result::Result<(), Self::Error>;
    /// Serialize one tagged property value.
    ///
    /// # Safety
    /// All pointers must identify live tag, property, and value storage.
    unsafe fn tagged_item(
        &self,
        context: Object,
        property: Object,
        value: Object,
    ) -> std::result::Result<(), Self::Error>;
    /// Read one ANSI byte from the legacy fixed-string archive representation.
    ///
    /// # Safety
    /// `context` must be a readable fixed-string tag archive.
    unsafe fn read_ansi_byte(&self, context: Object) -> std::result::Result<u8, Self::Error>;
    /// Assign the fixed-buffer FString representation to a property.
    ///
    /// # Safety
    /// `text` must be a terminated 32-bit character buffer valid only for this call.
    unsafe fn assign_fixed_string(
        &self,
        context: Object,
        property: Object,
        text: *const i32,
    ) -> std::result::Result<(), Self::Error>;
    /// Report a tagged-property warning.
    ///
    /// # Safety
    /// `context` must be live; `property` may be null for a missing property.
    unsafe fn warn(
        &self,
        context: Object,
        property: Object,
        warning: Hp2PropertyArchiveWarning,
    ) -> std::result::Result<(), Self::Error>;
    /// Query whether the archive has XLevel compatibility behavior.
    ///
    /// # Safety
    /// `context` must be live for the archive query.
    unsafe fn is_xlevel(&self, context: Object) -> std::result::Result<bool, Self::Error>;
    /// Consume one skipped serialized byte.
    ///
    /// # Safety
    /// `context` must be readable for one byte.
    unsafe fn read_byte(&self, context: Object) -> std::result::Result<(), Self::Error>;
    /// Finish the archive's name table.
    ///
    /// # Safety
    /// `context` must be live for name-table finalization.
    unsafe fn end_names(&self, context: Object) -> std::result::Result<(), Self::Error>;

    /// Run the complete binary or tagged traversal inside the archive stream.
    ///
    /// # Safety
    /// `context` and body state must remain live. Invoke `body` exactly once
    /// synchronously before returning, unless entering the scope fails; do not
    /// retain the closure.
    unsafe fn stream<F>(
        &self,
        context: Object,
        loading: bool,
        body: F,
    ) -> std::result::Result<(), Self::Error>
    where
        F: FnOnce() -> std::result::Result<(), Self::Error>;
    /// Run one load tag within its live archive scope.
    ///
    /// # Safety
    /// `context` must remain live. Invoke `body` exactly once synchronously
    /// before returning, unless entering the scope fails. The tag context is
    /// valid only for the closure and must not be retained.
    unsafe fn load_tag_scope<F>(
        &self,
        context: Object,
        body: F,
    ) -> std::result::Result<(), Self::Error>
    where
        F: FnOnce(Object) -> std::result::Result<(), Self::Error>;
    /// Run one save tag within its live archive scope.
    ///
    /// # Safety
    /// `context`, `property`, and `value` must remain live. Invoke `body`
    /// exactly once synchronously before returning, unless entering the scope
    /// fails; do not retain its tag context.
    unsafe fn save_tag_scope<F>(
        &self,
        context: Object,
        property: Object,
        index: i32,
        value: Object,
        body: F,
    ) -> std::result::Result<(), Self::Error>
    where
        F: FnOnce(Object) -> std::result::Result<(), Self::Error>;
}
fn status(code: i32) -> Result {
    if code == 0 { Ok(()) } else { Err(code) }
}
struct LegacyPropertyArchiveOps<'a> {
    services: &'a Hp2PropertyArchiveServices,
}

struct LegacyContinuation<F> {
    body: Option<F>,
}

unsafe extern "C" fn invoke_legacy_stream<F>(_: Object, user: Object) -> i32
where
    F: FnOnce() -> Result,
{
    let continuation = unsafe { &mut *user.cast::<LegacyContinuation<F>>() };
    match continuation.body.take() {
        Some(body) => body().err().unwrap_or(0),
        None => 1,
    }
}

unsafe extern "C" fn invoke_legacy_tag<F>(tag: Object, user: Object) -> i32
where
    F: FnOnce(Object) -> Result,
{
    let continuation = unsafe { &mut *user.cast::<LegacyContinuation<F>>() };
    match continuation.body.take() {
        Some(body) => body(tag).err().unwrap_or(0),
        None => 1,
    }
}

impl PropertyArchiveOps for LegacyPropertyArchiveOps<'_> {
    type Error = i32;

    fn string_property(&self) -> i32 {
        self.services.string_property
    }

    fn str_property(&self) -> i32 {
        self.services.str_property
    }

    fn struct_property(&self) -> i32 {
        self.services.struct_property
    }

    unsafe fn pointer(
        &self,
        object: Object,
        field: Hp2PropertyArchivePointer,
    ) -> std::result::Result<Object, Self::Error> {
        let mut value = std::ptr::null_mut();
        status(unsafe { (self.services.pointer)(object, field, &mut value) })?;
        Ok(value)
    }

    unsafe fn integer(
        &self,
        object: Object,
        field: Hp2PropertyArchiveInteger,
    ) -> std::result::Result<i32, Self::Error> {
        Ok(unsafe { (self.services.integer)(object, field) })
    }

    unsafe fn should_serialize(
        &self,
        property: Object,
        context: Object,
    ) -> std::result::Result<bool, Self::Error> {
        Ok(unsafe { (self.services.should_serialize)(property, context) != 0 })
    }

    unsafe fn name_matches(
        &self,
        object: Object,
        name: Hp2PropertyArchiveName,
    ) -> std::result::Result<bool, Self::Error> {
        let mut value = 0;
        status(unsafe { (self.services.name_matches)(object, name, &mut value) })?;
        Ok(value != 0)
    }

    unsafe fn struct_name(
        &self,
        property: Object,
    ) -> std::result::Result<i32, Self::Error> {
        let mut value = 0;
        status(unsafe { (self.services.struct_name)(property, &mut value) })?;
        Ok(value)
    }

    unsafe fn defaults(
        &self,
        context: Object,
    ) -> std::result::Result<(Object, i32), Self::Error> {
        let mut defaults = std::ptr::null_mut();
        let mut size = 0;
        status(unsafe { (self.services.defaults)(context, &mut defaults, &mut size) })?;
        Ok((defaults, size))
    }

    unsafe fn note_name(
        &self,
        context: Object,
        name: i32,
    ) -> std::result::Result<(), Self::Error> {
        unsafe { (self.services.note_name)(context, name) };
        Ok(())
    }

    unsafe fn note_index(
        &self,
        context: Object,
        index: i32,
    ) -> std::result::Result<(), Self::Error> {
        unsafe { (self.services.note_index)(context, index) };
        Ok(())
    }

    unsafe fn serialize_item(
        &self,
        context: Object,
        property: Object,
        value: Object,
    ) -> std::result::Result<(), Self::Error> {
        status(unsafe { (self.services.serialize_item)(context, property, value) })
    }

    unsafe fn matches(
        &self,
        property: Object,
        data: Object,
        defaults: Object,
        index: i32,
    ) -> std::result::Result<i32, Self::Error> {
        let mut value = 0;
        status(unsafe {
            (self.services.matches)(property, data, defaults, index, &mut value)
        })?;
        Ok(value)
    }

    unsafe fn read_tag(
        &self,
        context: Object,
    ) -> std::result::Result<*const Hp2PropertyArchiveTag, Self::Error> {
        let mut tag = std::ptr::null();
        status(unsafe { (self.services.read_tag)(context, &mut tag) })?;
        Ok(tag)
    }

    unsafe fn write_tag(&self, context: Object) -> std::result::Result<(), Self::Error> {
        status(unsafe { (self.services.write_tag)(context) })
    }

    unsafe fn tagged_item(
        &self,
        context: Object,
        property: Object,
        value: Object,
    ) -> std::result::Result<(), Self::Error> {
        status(unsafe { (self.services.tagged_item)(context, property, value) })
    }

    unsafe fn read_ansi_byte(&self, context: Object) -> std::result::Result<u8, Self::Error> {
        let mut byte = 0;
        status(unsafe { (self.services.read_ansi_byte)(context, &mut byte) })?;
        Ok(byte)
    }

    unsafe fn assign_fixed_string(
        &self,
        context: Object,
        property: Object,
        text: *const i32,
    ) -> std::result::Result<(), Self::Error> {
        status(unsafe { (self.services.assign_fixed_string)(context, property, text) })
    }

    unsafe fn warn(
        &self,
        context: Object,
        property: Object,
        warning: Hp2PropertyArchiveWarning,
    ) -> std::result::Result<(), Self::Error> {
        status(unsafe { (self.services.warn)(context, property, warning) })
    }

    unsafe fn is_xlevel(&self, context: Object) -> std::result::Result<bool, Self::Error> {
        Ok(unsafe { (self.services.is_xlevel)(context) != 0 })
    }

    unsafe fn read_byte(&self, context: Object) -> std::result::Result<(), Self::Error> {
        status(unsafe { (self.services.read_byte)(context) })
    }

    unsafe fn end_names(&self, context: Object) -> std::result::Result<(), Self::Error> {
        status(unsafe { (self.services.end_names)(context) })
    }

    unsafe fn stream<F>(
        &self,
        context: Object,
        loading: bool,
        body: F,
    ) -> std::result::Result<(), Self::Error>
    where
        F: FnOnce() -> std::result::Result<(), Self::Error>,
    {
        let mut continuation = LegacyContinuation { body: Some(body) };
        status(unsafe {
            (self.services.stream)(
                context,
                i32::from(loading),
                invoke_legacy_stream::<F>,
                (&mut continuation as *mut LegacyContinuation<F>).cast(),
            )
        })
    }

    unsafe fn load_tag_scope<F>(
        &self,
        context: Object,
        body: F,
    ) -> std::result::Result<(), Self::Error>
    where
        F: FnOnce(Object) -> std::result::Result<(), Self::Error>,
    {
        let mut continuation = LegacyContinuation { body: Some(body) };
        status(unsafe {
            (self.services.load_tag_scope)(
                context,
                invoke_legacy_tag::<F>,
                (&mut continuation as *mut LegacyContinuation<F>).cast(),
            )
        })
    }

    unsafe fn save_tag_scope<F>(
        &self,
        context: Object,
        property: Object,
        index: i32,
        value: Object,
        body: F,
    ) -> std::result::Result<(), Self::Error>
    where
        F: FnOnce(Object) -> std::result::Result<(), Self::Error>,
    {
        let mut continuation = LegacyContinuation { body: Some(body) };
        status(unsafe {
            (self.services.save_tag_scope)(
                context,
                property,
                index,
                value,
                invoke_legacy_tag::<F>,
                (&mut continuation as *mut LegacyContinuation<F>).cast(),
            )
        })
    }
}
unsafe fn pointer<O: PropertyArchiveOps + ?Sized>(
    s: &O,
    object: Object,
    field: Hp2PropertyArchivePointer,
) -> std::result::Result<Object, O::Error> {
    unsafe { s.pointer(object, field) }
}
unsafe fn named<O: PropertyArchiveOps + ?Sized>(
    s: &O,
    object: Object,
    name: Hp2PropertyArchiveName,
) -> std::result::Result<bool, O::Error> {
    unsafe { s.name_matches(object, name) }
}
struct Fields {
    owner: Object,
    field: Object,
}
impl Fields {
    unsafe fn new<O: PropertyArchiveOps + ?Sized>(
        owner: Object,
        s: &O,
    ) -> std::result::Result<Self, O::Error> {
        let mut fields = Self {
            owner,
            field: unsafe { pointer(s, owner, Hp2PropertyArchivePointer::ArchiveChildren)? },
        };
        unsafe {
            fields.seek(s)?;
        }
        Ok(fields)
    }
    unsafe fn seek<O: PropertyArchiveOps + ?Sized>(
        &mut self,
        s: &O,
    ) -> std::result::Result<(), O::Error> {
        use Hp2PropertyArchivePointer::*;
        while !self.owner.is_null() {
            while !self.field.is_null() {
                if unsafe { s.integer(self.field, Hp2PropertyArchiveInteger::ArchiveIsProperty)? }
                    != 0
                {
                    return Ok(());
                }
                self.field = unsafe { pointer(s, self.field, ArchiveNext)? };
            }
            self.owner = unsafe { pointer(s, self.owner, ArchiveSuper)? };
            if !self.owner.is_null() {
                self.field = unsafe { pointer(s, self.owner, ArchiveChildren)? };
            }
        }
        Ok(())
    }
    unsafe fn advance<O: PropertyArchiveOps + ?Sized>(
        &mut self,
        s: &O,
    ) -> std::result::Result<(), O::Error> {
        self.field = unsafe { pointer(s, self.field, Hp2PropertyArchivePointer::ArchiveNext)? };
        unsafe { self.seek(s) }
    }
}
unsafe fn offset<O: PropertyArchiveOps + ?Sized>(
    s: &O,
    property: Object,
    index: i32,
) -> std::result::Result<i32, O::Error> {
    use Hp2PropertyArchiveInteger::*;
    unsafe {
        Ok(s.integer(property, ArchiveOffset)?.wrapping_add(
            index.wrapping_mul(s.integer(property, ArchiveElementSize)?),
        ))
    }
}
fn at(data: Object, offset: i32) -> Object {
    data.cast::<u8>().wrapping_offset(offset as isize).cast()
}

/// `ARRAY_COUNT(Temp)` from the original `TCHAR Temp[255]` stack buffer.
const FIXED_STRING_TEMP_LEN: usize = 255;

/// `PropertyArchiveFixedString` (`UnClass.cpp:822`): reads an ANSI,
/// NUL-terminated byte string from the archive into a fixed `Temp` buffer
/// (one `TCHAR` per byte, via `FromAnsi`), then assigns it to the `FString`
/// at the property's tagged offset. `MaxCount` is `Tag->Size+1` when
/// `Tag->Size` is nonzero, else the buffer's own length; clamped here to
/// `FIXED_STRING_TEMP_LEN` to avoid writing past a fixed Rust array for a
/// malformed/oversized `Tag->Size` (the original's `TCHAR Temp[255]` has no
/// such bound and overflows its stack buffer in that case — undefined
/// behavior, not a deterministic sequence to replicate).
unsafe fn fixed_string<O: PropertyArchiveOps + ?Sized>(
    s: &O,
    tag_context: Object,
    property: Object,
    tag: *const Hp2PropertyArchiveTag,
) -> std::result::Result<(), O::Error> {
    unsafe {
        let mut temp = [0i32; FIXED_STRING_TEMP_LEN];
        let size = (*tag).size;
        let max_count = if size != 0 {
            size.wrapping_add(1)
        } else {
            FIXED_STRING_TEMP_LEN as i32
        };
        let max_count = max_count.clamp(1, FIXED_STRING_TEMP_LEN as i32);
        let mut count = 0usize;
        while (count as i32) < max_count - 1 {
            let byte = s.read_ansi_byte(tag_context)?;
            let ch = byte as i32;
            temp[count] = ch;
            count += 1;
            if ch == 0 {
                break;
            }
        }
        temp[count] = 0;
        s.assign_fixed_string(tag_context, property, temp.as_ptr())
    }
}

/// # Safety
/// Native object/context/data and callbacks must remain live through reentrant item serialization.
pub unsafe fn binary(
    object: Object,
    context: Object,
    data: Object,
    s: &Hp2PropertyArchiveServices,
) -> Result {
    let legacy = LegacyPropertyArchiveOps { services: s };
    unsafe { binary_with(object, context, data, &legacy) }
}

/// Runs binary property traversal through a Rust-owned archive capability.
///
/// # Safety
/// Object, archive, and value records must remain valid for each operation.
pub unsafe fn binary_with<O: PropertyArchiveOps>(
    object: Object,
    context: Object,
    data: Object,
    s: &O,
) -> std::result::Result<(), O::Error> {
    use Hp2PropertyArchiveInteger::*;
    use Hp2PropertyArchiveName::*;
    unsafe {
        let mut fields = Fields::new(object, s)?;
        while !fields.field.is_null() {
            let property = fields.field;
            let name = s.integer(property, ArchiveName)?;
            s.note_name(context, name)?;
            // The obsolete Max value is consumed even when Rand itself is skipped.
            if s.integer(context, ArchiveVersion)? < 76
                && (named(s, object, ArchiveFloatParams)?
                    || named(s, object, ArchiveColorParams)?)
                && named(s, property, ArchiveRand)?
            {
                let property_offset = s.integer(property, ArchiveOffset)?;
                s.serialize_item(context, property, at(data, property_offset))?;
            }
            if s.should_serialize(property, context)? {
                let mut index = 0i32;
                s.note_index(context, index)?;
                while index < s.integer(property, ArchiveArrayDim)? {
                    s.serialize_item(context, property, at(data, offset(s, property, index)?))?;
                    index = index.wrapping_add(1);
                    s.note_index(context, index)?;
                }
            }
            fields.advance(s)?;
        }
        Ok(())
    }
}
struct Stream<'a, O: PropertyArchiveOps> {
    object: Object,
    context: Object,
    data: Object,
    defaults: Object,
    defaults_count: i32,
    persistent: bool,
    loading: bool,
    s: &'a O,
}
struct Load<'a, O: PropertyArchiveOps> {
    stream: *const Stream<'a, O>,
    ended: bool,
}
unsafe fn load_one<O: PropertyArchiveOps>(
    tag_context: Object,
    load: *mut Load<'_, O>,
) -> std::result::Result<(), O::Error> {
    use Hp2PropertyArchiveInteger::*;
    use Hp2PropertyArchivePointer::*;
    use Hp2PropertyArchiveWarning::*;
    unsafe {
        let stream = (*load).stream;
        let s = (*stream).s;
        let tag = s.read_tag(tag_context)?;
        if (*tag).name == 0 {
            (*load).ended = true;
            return Ok(());
        }
        let context = (*stream).context;
        s.note_name(context, (*tag).name)?;
        let mut property = pointer(s, (*stream).object, ArchivePropertyLink)?;
        while !property.is_null() && s.integer(property, ArchiveName)? != (*tag).name {
            property = pointer(s, property, ArchivePropertyLinkNext)?;
        }
        let warning;
        if property.is_null() {
            warning = ArchiveMissingProperty;
        } else if i32::from((*tag).property_type) == s.string_property()
            && s.integer(property, ArchiveType)? == s.str_property()
        {
            s.note_index(context, (*tag).array_index)?;
            fixed_string(s, tag_context, property, tag)?;
            return Ok(());
        } else if i32::from((*tag).property_type) != s.integer(property, ArchiveType)? {
            warning = ArchiveTypeMismatch;
        } else if (*tag).array_index >= s.integer(property, ArchiveArrayDim)? {
            warning = ArchiveArrayBounds;
        } else {
            let mismatch = if i32::from((*tag).property_type) == s.struct_property() {
                (*tag).item_name != s.struct_name(property)?
            } else {
                false
            };
            if mismatch {
                warning = ArchiveStructMismatch;
            } else if !s.should_serialize(property, context)? {
                warning = ArchiveNotSerializable;
            } else {
                s.note_index(context, (*tag).array_index)?;
                s.tagged_item(
                    tag_context,
                    property,
                    at(
                        (*stream).data,
                        offset(s, property, (*tag).array_index)?,
                    ),
                )?;
                return Ok(());
            }
        }
        if warning != ArchiveNotSerializable || !s.is_xlevel(tag_context)? {
            s.warn(tag_context, property, warning)?;
        }
        if !s.is_xlevel(tag_context)? {
            s.warn(tag_context, property, ArchiveSkipping)?;
        }
        let mut index = 0i32;
        while index < (*tag).size {
            s.read_byte(context)?;
            index = index.wrapping_add(1);
        }
        Ok(())
    }
}
unsafe fn stream_body<O: PropertyArchiveOps>(
    stream: *const Stream<'_, O>,
) -> std::result::Result<(), O::Error> {
    use Hp2PropertyArchiveInteger::*;
    unsafe {
        let s = (*stream).s;
        let context = (*stream).context;
        if (*stream).loading {
            let mut load = Load {
                stream,
                ended: false,
            };
            let load_pointer = &raw mut load;
            while !load.ended {
                s.load_tag_scope(context, |tag_context| unsafe {
                    load_one(tag_context, load_pointer)
                })?;
            }
        } else {
            let mut fields = Fields::new((*stream).object, s)?;
            while !fields.field.is_null() {
                let property = fields.field;
                if s.should_serialize(property, context)? {
                    let name = s.integer(property, ArchiveName)?;
                    s.note_name(context, name)?;
                    if !(*stream).persistent
                        || !named(s, property, Hp2PropertyArchiveName::ArchiveAuxAnims)?
                    {
                        let mut index = 0i32;
                        while index < s.integer(property, ArchiveArrayDim)? {
                            s.note_index(context, index)?;
                            let value_offset = offset(s, property, index)?;
                            let mut matches = 0;
                            if !(*stream).persistent {
                                let defaults = if value_offset
                                    .wrapping_add(s.integer(property, ArchiveElementSize)?)
                                    <= (*stream).defaults_count
                                {
                                    (*stream).defaults
                                } else {
                                    std::ptr::null_mut()
                                };
                                matches = s.matches(
                                    property,
                                    (*stream).data,
                                    defaults,
                                    index,
                                )?;
                            }
                            if matches == 0 {
                                let value = at((*stream).data, value_offset);
                                s.save_tag_scope(
                                    context,
                                    property,
                                    index,
                                    value,
                                    |tag_context| unsafe {
                                        s.write_tag(tag_context)?;
                                        s.tagged_item(tag_context, property, value)
                                    },
                                )?;
                            }
                            index = index.wrapping_add(1);
                            s.note_index(context, index)?;
                        }
                    }
                }
                fields.advance(s)?;
            }
            s.end_names(context)?;
        }
        Ok(())
    }
}
/// # Safety
/// Context/data/defaults and all stack tag continuations must remain live until completion.
pub unsafe fn tagged(
    object: Object,
    context: Object,
    data: Object,
    persistent: bool,
    s: &Hp2PropertyArchiveServices,
) -> Result {
    let legacy = LegacyPropertyArchiveOps { services: s };
    unsafe { tagged_with(object, context, data, persistent, &legacy) }
}

/// Runs tagged-property traversal through a Rust-owned archive capability.
///
/// # Safety
/// Object, archive, data, and defaults records must remain valid for every
/// operation and synchronous scope closure.
pub unsafe fn tagged_with<O: PropertyArchiveOps>(
    object: Object,
    context: Object,
    data: Object,
    persistent: bool,
    s: &O,
) -> std::result::Result<(), O::Error> {
    unsafe {
        let (defaults, defaults_count) = s.defaults(context)?;
        tagged_with_defaults(object, context, data, persistent, defaults, defaults_count, s)
    }
}

/// Runs tagged traversal with defaults already resolved by the canonical class owner.
///
/// # Safety
/// The inputs satisfy the same record and synchronous-callback contract as [`tagged_with`].
pub unsafe fn tagged_with_defaults<O: PropertyArchiveOps>(
    object: Object,
    context: Object,
    data: Object,
    persistent: bool,
    defaults: Object,
    defaults_count: i32,
    s: &O,
) -> std::result::Result<(), O::Error> {
    unsafe {
        let loading = s.integer(context, Hp2PropertyArchiveInteger::ArchiveLoading)? != 0;
        let stream = Stream {
            object,
            context,
            data,
            defaults,
            defaults_count,
            persistent,
            loading,
            s,
        };
        let stream_pointer = &raw const stream;
        s.stream(context, loading, || unsafe { stream_body(stream_pointer) })
    }
}
