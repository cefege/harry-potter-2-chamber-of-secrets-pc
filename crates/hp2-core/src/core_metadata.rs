//! Direct class, field, property and script metadata ownership over Core storage.

use crate::class_metadata::{
    self, FunctionParameterInfo, Hp2MetadataArchiveValue as Value, Hp2MetadataCheck as Check,
    Hp2MetadataInteger as Integer, Hp2MetadataKind as Kind, Hp2MetadataPointer as Pointer,
    MetadataOps,
};
use crate::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use crate::core_class_specs::NativeRecordKind;
use crate::object_archive::{ArchiveMode, ObjectArchive};
use hp2_abi::engine::generated::{
    FDependency, TArrayRaw, UClass, UField, UFunction, UObject, UProperty, UState, UStruct,
};
use std::{ffi::c_void, ptr};

type Object = *mut c_void;

const EMPTY_ARRAY_TAG: [u32; 1] = [0];
// Optional EX_DebugInfo probes one token and one INT, then rewinds both cursors.
const DEBUG_LOOKAHEAD_BYTES: i32 = 1 + std::mem::size_of::<i32>() as i32;

// A reference visitor can change its desired value or the live operand.
// Omit only a genuinely redundant write. This preserves ordinary immutable
// script storage during marking, without losing reference replacement effects.
#[cfg(test)]
thread_local! {
    // Decision instrumentation only; this is not a memory write watchpoint.
    static REFERENCE_OPERAND_WRITE_DECISIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn reference_operand_unchanged(original: &[u8], desired: &[u8], live: &[u8]) -> bool {
    let unchanged = desired == original && live == original;
    #[cfg(test)]
    if !unchanged {
        REFERENCE_OPERAND_WRITE_DECISIONS.with(|count| count.set(count.get() + 1));
    }
    unchanged
}

struct DirectExpressionContext<'access, 'archive, 'data> {
    access: CoreAccess<'access>,
    structure: *mut UStruct,
    archive: &'archive ObjectArchive<'data>,
    debug_lookahead: std::cell::Cell<bool>,
}

impl DirectExpressionContext<'_, '_, '_> {

    fn script_byte(&self, offset: i32, width: usize) -> RuntimeResult<*mut u8> {
        if offset < 0 {
            return Err(RuntimeError::new("Negative script offset"));
        }
        let structure = self.structure;
        let (data, count) = unsafe {
            let script = ptr::addr_of!((*structure).script).read_unaligned();
            let limit = if self.debug_lookahead.get() {
                script.num.saturating_add(DEBUG_LOOKAHEAD_BYTES).min(script.max)
            } else { script.num };
            (script.data.cast::<u8>(), limit)
        };
        if data.is_null()
            || offset as usize > count.max(0) as usize
            || width > (count.max(0) as usize).saturating_sub(offset as usize)
        {
            return Err(RuntimeError::new("Script operand outside bytecode"));
        }
        Ok(unsafe { data.add(offset as usize) })
    }

    fn object_index(&self, index: i32) -> RuntimeResult<*mut UObject> {
        if index == -1 {
            return Ok(ptr::null_mut());
        }
        let object = self.access.object_at(index);
        if object.is_null() {
            return Err(RuntimeError::new("Invalid script object index")
                .with_context(&index.to_string()));
        }
        Ok(object)
    }

}

impl crate::expression_archive::ExpressionOps for DirectExpressionContext<'_, '_, '_> {
    type Error = RuntimeError;

    unsafe fn transfer(&self, code: *mut i32, kind: crate::expression_archive::Hp2ExprOperand) -> RuntimeResult<bool> {
        use crate::expression_archive::Hp2ExprOperand::*;
        if code.is_null() { return Err(RuntimeError::new("Null script expression cursor")); }
        let offset = unsafe { code.read() };
        unsafe {
            let (width, name_none) = match kind {
                ExprByte => {
                    let mut value = if self.archive.is_loading() { 0 } else { self.script_byte(offset, 1)?.read() };
                    let original = [value];
                    self.archive.serialize_u8(&mut value)?;
                    let slot = self.script_byte(offset, 1)?;
                    if self.archive.mode() != ArchiveMode::Reference || !reference_operand_unchanged(&original, &[value], std::slice::from_raw_parts(slot, 1)) {
                        slot.write(value);
                    }
                    (1, false)
                }
                ExprWord => {
                    let mut value = if self.archive.is_loading() { 0 } else { self.script_byte(offset, 2)?.cast::<u16>().read_unaligned() };
                    let original = value.to_ne_bytes();
                    self.archive.serialize_u16(&mut value)?;
                    let slot = self.script_byte(offset, 2)?;
                    if self.archive.mode() != ArchiveMode::Reference || !reference_operand_unchanged(&original, &value.to_ne_bytes(), std::slice::from_raw_parts(slot, 2)) {
                        slot.cast::<u16>().write_unaligned(value);
                    }
                    (2, false)
                }
                ExprInt => {
                    let mut value = if self.archive.is_loading() { 0 } else { self.script_byte(offset, 4)?.cast::<i32>().read_unaligned() };
                    let original = value.to_ne_bytes();
                    self.archive.serialize_i32(&mut value)?;
                    let slot = self.script_byte(offset, 4)?;
                    if self.archive.mode() != ArchiveMode::Reference || !reference_operand_unchanged(&original, &value.to_ne_bytes(), std::slice::from_raw_parts(slot, 4)) {
                        slot.cast::<i32>().write_unaligned(value);
                    }
                    (4, false)
                }
                ExprFloat => {
                    let mut value = if self.archive.is_loading() { 0.0 } else { self.script_byte(offset, 4)?.cast::<f32>().read_unaligned() };
                    let original = value.to_bits().to_ne_bytes();
                    self.archive.serialize_f32(&mut value)?;
                    let slot = self.script_byte(offset, 4)?;
                    if self.archive.mode() != ArchiveMode::Reference || !reference_operand_unchanged(&original, &value.to_bits().to_ne_bytes(), std::slice::from_raw_parts(slot, 4)) {
                        slot.cast::<f32>().write_unaligned(value);
                    }
                    (4, false)
                }
                ExprName => {
                    let mut value = if self.archive.is_loading() { 0 } else { self.script_byte(offset, 4)?.cast::<i32>().read_unaligned() };
                    let original = value.to_ne_bytes();
                    self.archive.serialize_name_slot(&mut value)?;
                    let slot = self.script_byte(offset, 4)?;
                    if self.archive.mode() != ArchiveMode::Reference || !reference_operand_unchanged(&original, &value.to_ne_bytes(), std::slice::from_raw_parts(slot, 4)) {
                        slot.cast::<i32>().write_unaligned(value);
                    }
                    (4, value == 0)
                }
                ExprProperty | ExprStruct | ExprClass | ExprObject => {
                    let original = if self.archive.is_loading() { -1 } else {
                        self.script_byte(offset, 4)?.cast::<i32>().read_unaligned()
                    };
                    let mut object = if self.archive.is_loading() { ptr::null_mut() } else {
                        self.object_index(original)?
                    };
                    self.archive.serialize_object_slot(&mut object)?;
                    let index = if object.is_null() { -1 } else { (*object).index };
                    let slot = self.script_byte(offset, 4)?;
                    if self.archive.mode() != ArchiveMode::Reference || !reference_operand_unchanged(&original.to_ne_bytes(), &index.to_ne_bytes(), std::slice::from_raw_parts(slot, 4)) {
                        slot.cast::<i32>().write_unaligned(index);
                    }
                    (4, false)
                }
            };
            code.write(offset.wrapping_add(width));
            Ok(name_none)
        }
    }

    unsafe fn read_script(&self, offset: i32, width: i32) -> RuntimeResult<u32> {
        let bytes = usize::try_from(width).map_err(|_| RuntimeError::new("Invalid script operand width"))?;
        let slot = self.script_byte(offset, bytes)?;
        unsafe {
            Ok(match bytes {
                1 => u32::from(slot.read()),
                2 => u32::from(slot.cast::<u16>().read_unaligned()),
                4 => slot.cast::<u32>().read_unaligned(),
                _ => return Err(RuntimeError::new("Invalid script operand width")),
            })
        }
    }

    fn version(&self) -> i32 { self.archive.flags().version as i32 }
    fn tell(&self) -> RuntimeResult<i32> {
        if self.archive.mode() == ArchiveMode::MemoryCount { return Ok(-1); }
        if self.archive.mode() == ArchiveMode::Reference { return Ok(0); }
        i32::try_from(self.archive.tell()?).map_err(|_| RuntimeError::new("Expression archive position exceeds INT"))
    }
    fn seek(&self, position: i32) -> RuntimeResult<()> {
        if self.archive.mode() == ArchiveMode::MemoryCount { return Ok(()); }
        if self.archive.mode() == ArchiveMode::Reference { return Ok(()); }
        let position = usize::try_from(position).map_err(|_| RuntimeError::new("Negative expression archive position"))?;
        self.archive.seek(position)
    }
    fn teleporter(&self) -> RuntimeResult<bool> {
        // Match the two canonical full names without allocating a formatted path.
        let named = |object: *mut UObject, text: &str| -> RuntimeResult<bool> {
            Ok(!object.is_null() && unsafe { i32::from_ne_bytes((*object).name) }
                == self.access.intern_str(text, crate::name_owner::FindName::Find)?)
        };
        unsafe {
            let object = self.structure.cast::<UObject>();
            let class = (*object).class.cast::<UObject>();
            let outer = (*object).outer.cast::<UObject>();
            let engine = if named(class, "Function")? {
                if !named(object, "SpecialHandling")? || !named(outer, "Teleporter")? { return Ok(false); }
                (*outer).outer.cast::<UObject>()
            } else if named(class, "Class")? && named(object, "Teleporter")? {
                outer
            } else { return Ok(false); };
            Ok(named(engine, "Engine")? && (*engine).outer.is_null())
        }
    }
    fn scope<F: FnMut() -> RuntimeResult<u8>>(&self, mut body: F) -> RuntimeResult<u8> { body() }
    fn debug_lookahead<F: FnOnce() -> RuntimeResult<i32>>(&self, body: F) -> RuntimeResult<i32> {
        let previous = self.debug_lookahead.replace(true);
        let result = body();
        self.debug_lookahead.set(previous);
        result
    }
    fn check_alignment(&self, code: i32) -> RuntimeResult<()> {
        if code & 3 == 0 { Ok(()) } else {
            Err(RuntimeError::new("Expression code is not aligned").with_context(&code.to_string()))
        }
    }
    fn bad_token(&self, token: u8) -> RuntimeResult<()> {
        Err(RuntimeError::new(&format!("Bad expr token {token:02x}")))
    }
}

impl CoreAccess<'_> {
    /// Serializes one live Core reflection record through its canonical metadata policy.
    ///
    /// # Safety
    /// `object` is a live owned reflection record of `kind`; the archive mapping
    /// remains valid through callbacks and may reenter or replace other records.
    pub(crate) unsafe fn serialize_metadata(
        self,
        object: *mut UObject,
        archive: &ObjectArchive<'_>,
        kind: Kind,
    ) -> RuntimeResult<()> {
        let ops = DirectMetadataOps { access: self };
        // SAFETY: the archive is only erased for this synchronous generic policy call.
        unsafe {
            class_metadata::serialize_with(
                self.metadata_storage(),
                object.cast(),
                (archive as *const ObjectArchive<'_>).cast_mut().cast(),
                kind,
                &ops,
            )?;
            if kind == Kind::MetadataFunction && archive.is_loading() {
                crate::stock_input_fix::fix(self, object)?;
            }
            Ok(())
        }
    }

    /// Links a UStruct's owned fields, publishing exact inherited property chains.
    ///
    /// # Safety
    /// `structure` is a live owned UStruct record. Archive callbacks may reenter,
    /// so the implementation retains no field or class-body borrow across them.
    pub(crate) unsafe fn link_struct(
        self,
        structure: *mut UStruct,
        archive: Option<&ObjectArchive<'_>>,
        link_properties: bool,
    ) -> RuntimeResult<()> {
        unsafe { crate::property_link::link_struct_owned(self, structure, archive, link_properties) }
    }
}

struct DirectMetadataOps<'a> {
    access: CoreAccess<'a>,
}

impl DirectMetadataOps<'_> {
    fn archive<'a>(&self, archive: Object) -> &'a ObjectArchive<'a> {
        // SAFETY: callers pass the address of the live ObjectArchive for the
        // synchronous call, and no reference is retained by this adapter.
        unsafe { &*archive.cast::<ObjectArchive<'a>>() }
    }

    unsafe fn serialize_array_names(
        &self,
        archive: &ObjectArchive<'_>,
        array: *mut TArrayRaw,
    ) -> RuntimeResult<()> {
        unsafe {
            let mut count = ptr::addr_of!((*array).num).read();
            archive.serialize_compact_index(&mut count)?;
            if archive.is_loading() {
                let arrays = crate::array_storage::system_allocator();
                let slots = crate::array_storage::Hp2ArraySlots {
                    data: ptr::addr_of_mut!((*array).data),
                    num: ptr::addr_of_mut!((*array).num),
                    max: ptr::addr_of_mut!((*array).max),
                };
                crate::array_storage::empty(slots, 0, 4, EMPTY_ARRAY_TAG.as_ptr().cast(), arrays)
                    .map_err(|_| RuntimeError::new("Name array allocation failed"))?;
                crate::array_storage::add_zeroed(
                    slots,
                    count,
                    4,
                    EMPTY_ARRAY_TAG.as_ptr().cast(),
                    arrays,
                )
                .map_err(|_| RuntimeError::new("Name array allocation failed").with_context(&format!("count={count}, archive offset={:?}", archive.tell())))?;
            }
            let data = ptr::addr_of!((*array).data).read_unaligned().cast::<i32>();
            for index in 0..count {
                archive.serialize_name_slot(data.add(index as usize))?;
            }
        }
        Ok(())
    }

    unsafe fn serialize_dependencies(
        &self,
        archive: &ObjectArchive<'_>,
        array: *mut TArrayRaw,
    ) -> RuntimeResult<()> {
        unsafe {
            let mut count = ptr::addr_of!((*array).num).read();
            archive.serialize_compact_index(&mut count)?;
            if archive.is_loading() {
                let arrays = crate::array_storage::system_allocator();
                let slots = crate::array_storage::Hp2ArraySlots {
                    data: ptr::addr_of_mut!((*array).data),
                    num: ptr::addr_of_mut!((*array).num),
                    max: ptr::addr_of_mut!((*array).max),
                };
                crate::array_storage::empty(
                    slots,
                    0,
                    std::mem::size_of::<FDependency>() as i32,
                    EMPTY_ARRAY_TAG.as_ptr().cast(),
                    arrays,
                )
                .map_err(|_| RuntimeError::new("Class dependency allocation failed"))?;
                crate::array_storage::add_zeroed(
                    slots,
                    count,
                    std::mem::size_of::<FDependency>() as i32,
                    EMPTY_ARRAY_TAG.as_ptr().cast(),
                    arrays,
                )
                .map_err(|_| RuntimeError::new("Class dependency allocation failed"))?;
            }
            for index in 0..count {
                let dependency = ptr::addr_of!((*array).data)
                    .read_unaligned()
                    .cast::<FDependency>()
                    .add(index as usize);
                archive.serialize_object_slot(ptr::addr_of_mut!((*dependency).class).cast())?;
                let dependency = ptr::addr_of!((*array).data)
                    .read_unaligned()
                    .cast::<FDependency>()
                    .add(index as usize);
                let mut deep = ptr::addr_of!((*dependency).deep).read_unaligned();
                archive.serialize_i32(&mut deep)?;
                let dependency = ptr::addr_of!((*array).data)
                    .read_unaligned()
                    .cast::<FDependency>()
                    .add(index as usize);
                ptr::addr_of_mut!((*dependency).deep).write_unaligned(deep);
                let mut script_text_crc =
                    ptr::addr_of!((*dependency).script_text_crc).read_unaligned();
                archive.serialize_u32(&mut script_text_crc)?;
                let dependency = ptr::addr_of!((*array).data)
                    .read_unaligned()
                    .cast::<FDependency>()
                    .add(index as usize);
                ptr::addr_of_mut!((*dependency).script_text_crc)
                    .write_unaligned(script_text_crc);
            }
        }
        Ok(())
    }

    #[inline(never)]
    unsafe fn serialize_value(
        &self,
        archive: &ObjectArchive<'_>,
        kind: Value,
        field: Object,
    ) -> RuntimeResult<()> {
        unsafe {
            match kind {
                Value::MetadataByte => {
                    let slot = field.cast::<u8>();
                    let mut value = slot.read_unaligned();
                    archive.serialize_u8(&mut value)?;
                    slot.write_unaligned(value);
                }
                Value::MetadataWord => {
                    let slot = field.cast::<u16>();
                    let mut value = slot.read_unaligned();
                    archive.serialize_u16(&mut value)?;
                    slot.write_unaligned(value);
                }
                Value::MetadataInt => {
                    let slot = field.cast::<i32>();
                    let mut value = slot.read_unaligned();
                    archive.serialize_i32(&mut value)?;
                    slot.write_unaligned(value);
                }
                Value::MetadataDword => {
                    let slot = field.cast::<u32>();
                    let mut value = slot.read_unaligned();
                    archive.serialize_u32(&mut value)?;
                    slot.write_unaligned(value);
                }
                Value::MetadataQword => {
                    let value = field.cast::<u64>();
                    let mut bits = value.read_unaligned();
                    if archive.is_loading() {
                        let mut bytes = [0; 8];
                        archive.read(&mut bytes)?;
                        bits = u64::from_le_bytes(bytes);
                    } else if archive.is_saving() {
                        archive.write(&bits.to_le_bytes())?;
                    }
                    value.write_unaligned(bits);
                }
                Value::MetadataFieldReference
                | Value::MetadataTextBufferReference
                | Value::MetadataClassReference => {
                    archive.serialize_object_slot(field.cast())?;
                }
                Value::MetadataName => archive.serialize_name_slot(field.cast())?,
                Value::MetadataGuid => {
                    for index in 0..4 {
                        let slot = field.cast::<u32>().add(index as usize);
                        let mut value = slot.read_unaligned();
                        archive.serialize_u32(&mut value)?;
                        slot.write_unaligned(value);
                    }
                }
                Value::MetadataDependencyArray => {
                    self.serialize_dependencies(archive, field.cast())?;
                }
                Value::MetadataNameArray => self.serialize_array_names(archive, field.cast())?,
            }
        }
        Ok(())
    }

    unsafe fn parent_serialize(
        &self,
        object: Object,
        archive: &ObjectArchive<'_>,
        kind: Kind,
    ) -> RuntimeResult<()> {
        let base_kind = match kind {
            Kind::MetadataField => None,
            Kind::MetadataStruct => Some(Kind::MetadataField),
            Kind::MetadataState => Some(Kind::MetadataStruct),
            Kind::MetadataClass => Some(Kind::MetadataState),
            Kind::MetadataFunction => Some(Kind::MetadataStruct),
        };
        if let Some(base_kind) = base_kind {
            unsafe { self.access.serialize_metadata(object.cast(), archive, base_kind) }
        } else {
            unsafe { crate::core_lifetime::serialize_object_base(self.access, object.cast(), archive) }
        }
    }

    unsafe fn class_check(&self, object: Object, check: Check) -> RuntimeResult<()> {
        let class = object.cast::<UClass>();
        match check {
            Check::MetadataFriendlyNameValid => {
                let name = unsafe { i32::from_ne_bytes((*object.cast::<UStruct>()).friendly_name) };
                if name == 0 {
                    return Err(RuntimeError::new("FriendlyName!=NAME_None"));
                }
            }
            Check::MetadataDefaultSizeValid => {
                if unsafe { (*class).properties_size } < std::mem::size_of::<UObject>() as i32 {
                    return Err(RuntimeError::new("GetPropertiesSize()>=sizeof(UObject)"));
                }
            }
            Check::MetadataSuperclassLoaded => {
                let super_class = unsafe { (*class).super_field.cast::<UClass>() };
                let need_load = hp2_abi::engine::generated::CORE_LINKER_CONSTANTS.need_load;
                if !super_class.is_null()
                    && unsafe { (*super_class.cast::<UObject>()).object_flags } & need_load != 0
                {
                    return Err(RuntimeError::new("Superclass is not loaded"));
                }
            }
            Check::MetadataDefaultsMatch => {
                if unsafe { (*class).defaults.num } != unsafe { (*class).properties_size } {
                    return Err(RuntimeError::new("Defaults.Num!=GetPropertiesSize()"));
                }
            }
            Check::MetadataScriptNativeZero => {
                if unsafe { (*object.cast::<UFunction>()).i_native } != 0 {
                    return Err(RuntimeError::new("Script function has native index"));
                }
            }
            Check::MetadataNativeIndexValid => {
                let index = i32::from(unsafe { (*object.cast::<UFunction>()).i_native });
                if index < 0 || index as usize >= crate::vm_registry::NATIVE_COUNT {
                    return Err(RuntimeError::new("iNative<EX_Max"));
                }
            }
            Check::MetadataNativeFunctionPresent => {
                let native_index = i32::from(unsafe { (*object.cast::<UFunction>()).i_native });
                let name = self.access.name_text(unsafe {
                    i32::from_ne_bytes((*object.cast::<UObject>()).name)
                })?;
                let name: String = name.into_iter().filter_map(char::from_u32).collect();
                let owner = unsafe {
                    ptr::addr_of!((*object.cast::<UObject>()).outer).read_unaligned()
                };
                let mut present = false;
                for spec in crate::core_native_catalog::iter() {
                    let Some(native_name) = spec.token_name.rsplit("exec").next() else {
                        continue;
                    };
                    if native_name != name {
                        continue;
                    }
                    match spec.opcode {
                        Some(opcode) if opcode != native_index => continue,
                        None if native_index != 0 => continue,
                        _ => {}
                    }
                    let Some(class_name) = spec.token_name.strip_prefix("int")
                        .and_then(|value| value.split("exec").next())
                    else {
                        continue;
                    };
                    let native_class = self.access.native_class(class_name)?;
                    if !owner.is_null()
                        && unsafe {
                            crate::object_model::is_child_of(owner.cast(), native_class.cast())
                        }
                    {
                        present = true;
                        break;
                    }
                }
                if !present {
                    return Err(RuntimeError::new("GNatives[iNative]!=0").with_context(&name));
                }
            }
        }
        Ok(())
    }
}

unsafe impl MetadataOps for DirectMetadataOps<'_> {
    type Error = RuntimeError;

    unsafe fn integer(&self, object: Object, selector: Integer) -> RuntimeResult<i32> {
        use Integer::*;
        Ok(match selector {
            MetadataArchiveVersion => self.archive(object).flags().version as i32,
            MetadataArchiveLoading => i32::from(self.archive(object).is_loading()),
            MetadataArchiveSaving => i32::from(self.archive(object).is_saving()),
            MetadataIsProperty => i32::from(matches!(
                self.access.record_kind(object.cast())?,
                crate::core_class_specs::NativeRecordKind::Property
                    | crate::core_class_specs::NativeRecordKind::ByteProperty
                    | crate::core_class_specs::NativeRecordKind::IntProperty
                    | crate::core_class_specs::NativeRecordKind::BoolProperty
                    | crate::core_class_specs::NativeRecordKind::FloatProperty
                    | crate::core_class_specs::NativeRecordKind::ObjectProperty
                    | crate::core_class_specs::NativeRecordKind::ClassProperty
                    | crate::core_class_specs::NativeRecordKind::NameProperty
                    | crate::core_class_specs::NativeRecordKind::StringProperty
                    | crate::core_class_specs::NativeRecordKind::FixedArrayProperty
                    | crate::core_class_specs::NativeRecordKind::ArrayProperty
                    | crate::core_class_specs::NativeRecordKind::MapProperty
                    | crate::core_class_specs::NativeRecordKind::StructProperty
            )),
            MetadataIsStruct => i32::from(matches!(
                self.access.record_kind(object.cast())?,
                crate::core_class_specs::NativeRecordKind::Struct
                    | crate::core_class_specs::NativeRecordKind::State
                    | crate::core_class_specs::NativeRecordKind::Class
                    | crate::core_class_specs::NativeRecordKind::Function
                    | crate::core_class_specs::NativeRecordKind::Enum
            )),
        })
    }

    unsafe fn super_serialize(&self, object: Object, archive: Object, kind: Kind) -> RuntimeResult<()> {
        unsafe { self.parent_serialize(object, self.archive(archive), kind) }
    }
    fn flatten_superclasses(&self) -> bool {
        true
    }

    #[inline(never)]
    unsafe fn serialize_value(&self, archive: Object, value: Value, field: Object) -> RuntimeResult<()> {
        unsafe { self.serialize_value(self.archive(archive), value, field) }
    }
    #[inline(never)]
    unsafe fn serialize_reference(
        &self,
        archive: Object,
        value: Value,
        field: Object,
    ) -> RuntimeResult<()> {
        use Value::*;
        match value {
            MetadataFieldReference | MetadataTextBufferReference | MetadataClassReference => {
                unsafe { self.archive(archive).serialize_object_slot(field.cast()) }
            }
            _ => unsafe { self.serialize_value(self.archive(archive), value, field) },
        }
    }

    unsafe fn check(&self, object: Object, check: Check) -> RuntimeResult<()> {
        unsafe { self.class_check(object, check) }
    }

    unsafe fn serialize_expr(&self, object: Object, archive: Object, code: *mut i32) -> RuntimeResult<()> {
        let context = DirectExpressionContext {
            access: self.access,
            structure: object.cast(),
            archive: self.archive(archive),
            debug_lookahead: std::cell::Cell::new(false),
        };
        unsafe { crate::expression_archive::serialize_with(code, &context).map(|_| ()) }
    }

    unsafe fn script_mismatch(&self, actual: i32, expected: i32) -> RuntimeResult<()> {
        Err(RuntimeError::new("ScriptSize mismatch")
            .with_context(&format!("actual={actual}, expected={expected}")))
    }

    unsafe fn reset_script(&self, object: Object, size: i32) -> RuntimeResult<()> {
        if size < 0 { return Err(RuntimeError::new("Negative script size")); }
        unsafe {
            let script = ptr::addr_of_mut!((*object.cast::<UStruct>()).script);
            let slots = crate::array_storage::Hp2ArraySlots {
                data: ptr::addr_of_mut!((*script).data),
                num: ptr::addr_of_mut!((*script).num),
                max: ptr::addr_of_mut!((*script).max),
            };
            let arrays = crate::array_storage::system_allocator();
            crate::array_storage::empty(slots, 0, 1, EMPTY_ARRAY_TAG.as_ptr().cast(), arrays)
                .map_err(|_| RuntimeError::new("Script allocation failed"))?;
            crate::array_storage::add_zeroed(slots, size, 1, EMPTY_ARRAY_TAG.as_ptr().cast(), arrays)
                .map_err(|_| RuntimeError::new("Script allocation failed"))?;
            if size != 0 {
                // Native TArray slack is not bytecode. Initialize only the five
                // probe bytes and retain the source's logical Script.Num.
                let padding = crate::array_storage::add_zeroed(slots, DEBUG_LOOKAHEAD_BYTES, 1,
                    EMPTY_ARRAY_TAG.as_ptr().cast(), arrays);
                slots.num.write(size);
                padding.map_err(|_| RuntimeError::new("Script lookahead allocation failed"))?;
            }
        }
        Ok(())
    }

    unsafe fn link(&self, object: Object, archive: Object, props: i32) -> RuntimeResult<()> {
        unsafe {
            self.access.link_struct(
                object.cast(),
                if archive.is_null() { None } else { Some(self.archive(archive)) },
                props != 0,
            )
        }
    }

    unsafe fn set_flags(&self, object: Object, flags: u32) -> RuntimeResult<()> {
        unsafe { (*object.cast::<UObject>()).object_flags |= flags; }
        Ok(())
    }

    unsafe fn system_config(&self, object: Object) -> RuntimeResult<()> {
        let name = self.access.intern_str("System", crate::name_owner::FindName::Add)?;
        unsafe { (*object.cast::<UClass>()).class_config_name = name.to_ne_bytes(); }
        Ok(())
    }
    unsafe fn initialize_default(&self, object: Object) -> RuntimeResult<()> {
        unsafe { self.access.initialize_default(object.cast()) }
    }

    unsafe fn tagged_defaults(&self, object: Object, archive: Object) -> RuntimeResult<()> {
        let class = object.cast::<UClass>();
        let parent = unsafe { (*class).super_field.cast::<UClass>() };
        let (defaults, count) = if parent.is_null() {
            (ptr::null(), 0)
        } else {
            unsafe { ((*parent).defaults.data.cast::<u8>().cast_const(), (*parent).properties_size) }
        };
        let data = unsafe { ptr::addr_of!((*class).defaults.data).read_unaligned().cast() };
        unsafe {
            self.access.tagged_properties(
                object.cast(),
                self.archive(archive),
                data,
                defaults,
                count,
            )
        }
    }

    unsafe fn load_config(&self, object: Object) -> RuntimeResult<()> {
        let default = unsafe { self.access.default_object(object.cast())? };
        unsafe { self.access.load_config(default) }
    }

    unsafe fn load_localized(&self, object: Object) -> RuntimeResult<()> {
        let default = unsafe { self.access.default_object(object.cast())? };
        unsafe { self.access.load_localized(default) }
    }

    unsafe fn flush_localized(&self) -> RuntimeResult<()> {
        self.access.flush_localized()
    }

    unsafe fn count_defaults(&self, object: Object, archive: Object) -> RuntimeResult<()> {
        let defaults = unsafe { ptr::read_unaligned(ptr::addr_of!((*object.cast::<UClass>()).defaults)) };
        let count = usize::try_from(defaults.num)
            .map_err(|_| RuntimeError::new("Invalid class defaults count"))?;
        let capacity = usize::try_from(defaults.max)
            .map_err(|_| RuntimeError::new("Invalid class defaults capacity"))?;
        self.archive(archive).count_bytes(count, capacity)
    }

    unsafe fn binary_defaults(&self, object: Object, archive: Object) -> RuntimeResult<()> {
        let class = object.cast::<UClass>();
        let data = unsafe { ptr::addr_of!((*class).defaults.data).read_unaligned().cast() };
        unsafe {
            self.access.binary_properties(
                class.cast(),
                self.archive(archive),
                data,
            )
        }
    }

    unsafe fn pointer(&self, object: Object, selector: Pointer) -> RuntimeResult<Object> {
        use Pointer::*;
        Ok(unsafe {
            match selector {
                MetadataInheritanceSuper if self.access.record_kind(object.cast())? == NativeRecordKind::Function => ptr::null_mut(),
                MetadataInheritanceSuper => (*object.cast::<UField>()).super_field,
                MetadataSuperState => (*object.cast::<UField>()).super_field,
                MetadataSuperClass => (*object.cast::<UClass>()).super_field,
                MetadataRootClass => self.access.native_class("Object")?.cast(),
            }
        })
    }

    unsafe fn reset_class_defaults(&self, object: Object) -> RuntimeResult<()> {
        unsafe { self.access.reset_class_defaults(object.cast()) }
    }

    unsafe fn function_parameter(&self, property: Object) -> RuntimeResult<Option<FunctionParameterInfo>> {
        if property.is_null() { return Ok(None); }
        let property = property.cast::<UProperty>();
        Ok(Some(unsafe { FunctionParameterInfo {
            flags: (*property).property_flags,
            offset: (*property).offset,
            array_dim: (*property).array_dim,
            element_size: (*property).element_size,
            next: (*property.cast::<UField>()).next,
        } }))
    }
}


#[cfg(test)]
#[path = "core_metadata_reference_tests.rs"]
mod reference_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_access::{CoreOptions, CoreOwner};
    use crate::expression_archive::token::{DEBUG_INFO, END_FUNCTION_PARMS, FIRST_NATIVE};
    use crate::object_archive::{ArchiveFlags, ArchiveMapping};
    use crate::package_archive::ExportArchive;

    struct NoReferences;
    impl ArchiveMapping for NoReferences {
        fn load_object(&self, _: i32) -> RuntimeResult<*mut UObject> { Err(RuntimeError::new("Unexpected object reference")) }
        fn save_object(&self, _: *mut UObject) -> RuntimeResult<i32> { Err(RuntimeError::new("Unexpected object reference")) }
        fn load_name(&self, _: i32) -> RuntimeResult<i32> { Err(RuntimeError::new("Unexpected name reference")) }
        fn save_name(&self, _: i32) -> RuntimeResult<i32> { Err(RuntimeError::new("Unexpected name reference")) }
        fn visit_object(&self, _: *mut *mut UObject) -> RuntimeResult<()> { Err(RuntimeError::new("Unexpected reference traversal")) }
        fn visit_name(&self, _: i32) -> RuntimeResult<()> { Err(RuntimeError::new("Unexpected name traversal")) }
    }
    fn output(_: CoreAccess<'_>, _: i32, _: &[u32]) -> RuntimeResult<()> { Ok(()) }

    #[test]
    fn native_call_at_script_end_peeks_and_rewinds_without_widening_operand_bounds() {
        let root = std::path::PathBuf::from(std::env::var_os("HP2_ARTIFACT_DIR").expect("isolated artifacts"))
            .join(format!("script-lookahead-{}", std::process::id()));
        std::fs::create_dir_all(root.join("System")).unwrap();
        std::fs::write(root.join("System/Default.ini"), "").unwrap();
        std::fs::write(root.join("System/DefUser.ini"), "").unwrap();
        let owner = CoreOwner::new(CoreOptions {
            data_root: root.clone(), system_ini: root.join("System/Default.ini"),
            user_ini: root.join("System/DefUser.ini"), command_line: Vec::new(), output, log_file: None,
        }).unwrap();
        let access = owner.access();
        unsafe {
            let structure = access.construct_object(access.native_class("Core.Struct").unwrap(),
                access.transient_package(), 0, 0, ptr::null_mut()).unwrap().cast::<UStruct>();
            for tail in [&[0xa5][..], &[DEBUG_INFO, 99, 0, 0, 0][..]] {
                DirectMetadataOps { access }.reset_script(structure.cast(), 2).unwrap();
                let script = ptr::addr_of!((*structure).script).read_unaligned();
                assert!(script.max >= 7);
                let mut wire = vec![FIRST_NATIVE, END_FUNCTION_PARMS];
                wire.extend_from_slice(tail);
                let archive = ObjectArchive::loading(ExportArchive::from_slice(&wire),
                    ArchiveFlags::new(79, 0, false), &NoReferences);
                let context = DirectExpressionContext { access, structure, archive: &archive,
                    debug_lookahead: std::cell::Cell::new(false) };
                let mut code = 0;
                assert_eq!(crate::expression_archive::serialize_with(&mut code, &context).unwrap(), FIRST_NATIVE);
                assert_eq!(code, 2);
                assert_eq!(archive.tell().unwrap(), 2);
                assert_eq!(ptr::addr_of!((*structure).script.num).read_unaligned(), 2);
                assert_eq!(std::slice::from_raw_parts(script.data.cast::<u8>(), 2), &[FIRST_NATIVE, END_FUNCTION_PARMS]);
                assert_eq!(std::slice::from_raw_parts(script.data.cast::<u8>().add(2), tail.len()), tail);
                assert!(context.script_byte(2, 1).is_err(), "ordinary operands must still reject the logical end");
            }
        }
        owner.shutdown().unwrap();
    }
}
