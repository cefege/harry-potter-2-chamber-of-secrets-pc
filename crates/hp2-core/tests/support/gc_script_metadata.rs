//! Read-only, same-process preflight of Reference BinaryProperties traversal.
//!
//! This module does not preload, intern, fill Core caches, invoke callbacks,
//! execute events, collect, or write an artifact. The caller owns output and
//! acceptance after the synchronous scan returns.
use hp2_abi::engine::generated::{CORE_LINKER_CONSTANTS, UClass, UField, UObject, UProperty, UStruct};
use hp2_core::core_access::CoreAccess;
use hp2_core::core_class_specs::NativeRecordKind as Kind;
use std::{collections::{HashMap, HashSet}, ptr};

const LIMIT: usize = 16384;
const CPF_NATIVE: u32 = 0x1000;

/// An exact canonical native class identity and its allocation kind, resolved
/// by the caller before entering the read-only scan.
#[derive(Clone, Copy)]
pub struct NativeClass {
    pub address: *mut UClass,
    pub name: &'static str,
    pub kind: Kind,
}

pub struct Report {
    pub classes: Vec<String>,
    pub lines: Vec<String>,
    pub serializable: Vec<String>,
}

fn name(access: CoreAccess<'_>, index: i32) -> Result<String, String> {
    access.name_text(index).map_err(|e| e.to_string())?.into_iter()
        .map(|c| char::from_u32(c).ok_or_else(|| "invalid reflected name scalar".to_owned())).collect()
}

fn indexed(access: CoreAccess<'_>, table: &HashMap<usize, i32>, object: *mut UObject) -> Result<i32, String> {
    let index = *table.get(&(object as usize)).ok_or("null or unindexed metadata pointer")?;
    if access.object_at(index) != object { return Err("object table changed during scan".into()); }
    // SAFETY: table membership establishes a live canonical UObject header;
    // the caller guarantees object-table stability for the entire scan.
    if unsafe { ptr::addr_of!((*object).index).read_unaligned() } != index {
        return Err("object header index does not match table".into());
    }
    Ok(index)
}

fn is_property(kind: Kind) -> bool {
    matches!(kind, Kind::Property | Kind::ByteProperty | Kind::IntProperty | Kind::BoolProperty
        | Kind::FloatProperty | Kind::ObjectProperty | Kind::ClassProperty | Kind::NameProperty
        | Kind::StringProperty | Kind::FixedArrayProperty | Kind::ArrayProperty
        | Kind::MapProperty | Kind::StructProperty)
}
fn is_field(kind: Kind) -> bool {
    is_property(kind) || matches!(kind, Kind::Field | Kind::Struct | Kind::State
        | Kind::Class | Kind::Function | Kind::Enum | Kind::Const)
}

fn kind_of(access: CoreAccess<'_>, table: &HashMap<usize, i32>,
           native: &HashMap<usize, (&'static str, Kind)>, object: *mut UObject)
           -> Result<(&'static str, Kind), String> {
    indexed(access, table, object)?;
    // SAFETY: membership established the live canonical UObject header.
    let class = unsafe { ptr::addr_of!((*object).class).read_unaligned().cast::<UObject>() };
    indexed(access, table, class)?;
    native.get(&(class as usize)).copied().ok_or_else(|| "unknown exact native class".into())
}

/// Walk the actual `property_archive::Fields` children/next/super traversal.
///
/// Every visited property lacking CPF_NATIVE is returned in `serializable`;
/// the caller must reject a nonempty list. Version 79 excludes the obsolete
/// pre-v76 Rand branch, and Reference mode is nonpersistent. This scan is not a
/// physical write observer or a proof of other serialization/preload paths.
///
/// # Safety
/// The caller must supply exact class pointers and kinds resolved from this
/// Core owner's immutable native registration. Native class identity must imply
/// the complete declared allocation prefix, not a forged or replaced record.
/// All indexed records and the class registry must remain live and stable; no
/// callback, GC, load, object-table mutation, or concurrent access can occur
/// until return. The supplied root must be a live canonical class record.
pub unsafe fn scan(access: CoreAccess<'_>, root: *mut UClass, classes: &[NativeClass]) -> Result<Report, String> {
    if root.is_null() { return Err("null root class".into()); }
    let mut native = HashMap::new();
    for class in classes {
        if class.address.is_null() || native.insert(class.address as usize, (class.name, class.kind)).is_some() {
            return Err("null or duplicate native class identity".into());
        }
    }
    let mut table = HashMap::new();
    for index in 0..access.object_count() {
        let object = access.object_at(index);
        if !object.is_null() && table.insert(object as usize, index).is_some() {
            return Err("duplicate pointer in object table".into());
        }
    }
    let mut report = Report { classes: Vec::new(), lines: Vec::new(), serializable: Vec::new() };
    let mut owner = root.cast::<UObject>();
    let mut seen_classes = HashSet::new();
    let mut seen_fields = HashSet::new();
    while !owner.is_null() {
        if !seen_classes.insert(owner as usize) || seen_classes.len() > LIMIT { return Err("class chain duplicate/cycle/bound".into()); }
        let index = indexed(access, &table, owner)?;
        if kind_of(access, &table, &native, owner)?.1 != Kind::Class { return Err("super chain record is not exact native Class".into()); }
        // SAFETY: exact Core.Class identity supplies UField/UStruct prefixes.
        let (name_index, mut child, super_field, object_flags, class, linker, linker_index) = unsafe {
            (i32::from_ne_bytes(ptr::addr_of!((*owner).name).read_unaligned()),
             ptr::addr_of!((*owner.cast::<UStruct>()).children).read_unaligned().cast::<UObject>(),
             ptr::addr_of!((*owner.cast::<UField>()).super_field).read_unaligned().cast::<UObject>(),
             ptr::addr_of!((*owner).object_flags).read_unaligned(),
             ptr::addr_of!((*owner).class).read_unaligned(),
             ptr::addr_of!((*owner)._linker).read_unaligned(),
             ptr::addr_of!((*owner)._linker_index).read_unaligned())
        };
        if object_flags & (CORE_LINKER_CONSTANTS.need_load | CORE_LINKER_CONSTANTS.preloading) != 0 {
            return Err("class needs load or is preloading".into());
        }
        let owner_name = name(access, name_index)?;
        report.lines.push(format!("CLASS {owner_name} address={owner:p} index={index} class={class:p} flags=0x{object_flags:08x} linker={linker:p} linker_index={linker_index} children={child:p} super={super_field:p}"));
        report.classes.push(owner_name.clone());
        while !child.is_null() {
            if !seen_fields.insert(child as usize) || seen_fields.len() > LIMIT { return Err("field chain duplicate/cycle/bound".into()); }
            let index = indexed(access, &table, child)?;
            let (class_name, kind) = kind_of(access, &table, &native, child)?;
            if !is_field(kind) { return Err(format!("unsupported non-field native kind {kind:?}")); }
            // SAFETY: exact native record kind supplies its UField prefix.
            let (field_name, next, object_flags) = unsafe {
                (i32::from_ne_bytes(ptr::addr_of!((*child).name).read_unaligned()),
                 ptr::addr_of!((*child.cast::<UField>()).next).read_unaligned().cast::<UObject>(),
                 ptr::addr_of!((*child).object_flags).read_unaligned())
            };
            if object_flags & (CORE_LINKER_CONSTANTS.need_load | CORE_LINKER_CONSTANTS.preloading) != 0 {
                return Err("field needs load or is preloading".into());
            }
            let field_name = name(access, field_name)?;
            if is_property(kind) {
                // SAFETY: exact property kind supplies its UProperty prefix.
                let (flags, offset, size, dim) = unsafe {
                    let p = child.cast::<UProperty>();
                    (ptr::addr_of!((*p).property_flags).read_unaligned(),
                     ptr::addr_of!((*p).offset).read_unaligned(),
                     ptr::addr_of!((*p).element_size).read_unaligned(),
                     ptr::addr_of!((*p).array_dim).read_unaligned())
                };
                report.lines.push(format!("PROPERTY {owner_name}.{field_name} kind={class_name} index={index} object_flags=0x{object_flags:08x} flags=0x{flags:08x} offset={offset} element_size={size} dim={dim} next={next:p}"));
                if flags & CPF_NATIVE == 0 { report.serializable.push(format!("{owner_name}.{field_name} flags=0x{flags:08x} offset={offset}")); }
            } else {
                report.lines.push(format!("FIELD {owner_name}.{field_name} kind={class_name} index={index} flags=0x{object_flags:08x} next={next:p}"));
            }
            child = next;
        }
        owner = super_field;
    }
    Ok(report)
}
