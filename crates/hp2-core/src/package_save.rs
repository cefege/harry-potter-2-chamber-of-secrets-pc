//! Shared package-save policy for native and Rust-owned package writers.
use hp2_abi::engine::generated::UObject;
use std::result::Result;

pub(crate) type NameIndex = i32;


/// Shared save state read by package-save phases.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SaveContext {
    pub outer: *mut UObject,
    pub base: *mut UObject,
    pub top_level_flags: u32,
    pub has_conform: bool,
    pub check: bool,
    pub slow: bool,
    pub tag_export: u32,
    pub tag_import: u32,
    pub export_name_flags: u32,
    pub public_flag: u32,
}

/// Operations required by the shared save policy; linker ownership stays in each adapter.
pub(crate) trait PackageSaveOps {
    type Error;

    fn context(&self) -> SaveContext;
    fn clear_save_tags(&mut self) -> Result<(), Self::Error>;
    fn tag_export(&mut self, object: *mut UObject) -> Result<(), Self::Error>;
    fn tag_import(&mut self, object: *mut UObject) -> Result<(), Self::Error>;
    fn object_count(&mut self) -> Result<usize, Self::Error>;
    fn object_at(&mut self, index: usize) -> Result<*mut UObject, Self::Error>;
    fn name_flags(&mut self, name: NameIndex) -> Result<u32, Self::Error>;
    fn object_flags(&mut self, object: *mut UObject) -> Result<u32, Self::Error>;
    fn object_outer(&mut self, object: *mut UObject) -> Result<*mut UObject, Self::Error>;
    fn object_class(&mut self, object: *mut UObject) -> Result<*mut UObject, Self::Error>;
    fn object_name(&mut self, object: *mut UObject) -> Result<NameIndex, Self::Error>;
    fn set_object_name_flags(&mut self, name: NameIndex, flags: u32) -> Result<(), Self::Error>;
    fn is_in_package(&mut self, object: *mut UObject, outer: *mut UObject) -> Result<bool, Self::Error>;
    fn report_private_import(&mut self, object: *mut UObject) -> Result<(), Self::Error>;
    fn report_saving_object(&mut self, object: *mut UObject) -> Result<(), Self::Error>;

    fn global_name_count(&mut self) -> Result<usize, Self::Error>;
    fn global_name_flags(&mut self, index: usize) -> Result<u32, Self::Error>;
    fn name_count(&mut self) -> Result<usize, Self::Error>;
    fn name_at(&mut self, index: usize) -> Result<NameIndex, Self::Error>;
    fn set_name_count(&mut self, count: usize) -> Result<(), Self::Error>;
    fn set_import_count(&mut self, count: usize) -> Result<(), Self::Error>;
    fn save_name(&mut self, name: NameIndex) -> Result<(), Self::Error>;
    fn set_name_usage(&mut self, name: NameIndex, usage: i32) -> Result<(), Self::Error>;
    fn name_usage(&mut self, name: NameIndex) -> Result<i32, Self::Error>;
    fn find_name(&mut self, name: NameIndex) -> Result<Option<usize>, Self::Error>;
    fn append_name_slot(&mut self) -> Result<usize, Self::Error>;
    fn swap_names(&mut self, left: usize, right: usize) -> Result<(), Self::Error>;
    fn set_name_at(&mut self, index: usize, name: NameIndex) -> Result<(), Self::Error>;
    fn sort_names_from(&mut self, index: usize) -> Result<(), Self::Error>;
    fn conform_name_count(&mut self) -> Result<usize, Self::Error>;
    fn conform_name_at(&mut self, index: usize) -> Result<NameIndex, Self::Error>;
    fn original_name_count(&mut self) -> Result<usize, Self::Error>;
    fn original_name_at(&mut self, index: usize) -> Result<NameIndex, Self::Error>;

    fn add_name(&mut self, name: NameIndex) -> Result<(), Self::Error>;
    fn add_import(&mut self, object: *mut UObject) -> Result<(), Self::Error>;
    fn add_export(&mut self, object: *mut UObject) -> Result<(), Self::Error>;
    fn sort_imports(&mut self) -> Result<(), Self::Error>;
    fn sort_exports(&mut self, conform_start: usize) -> Result<(), Self::Error>;
    fn export_count(&mut self) -> Result<usize, Self::Error>;
    fn export_at(&mut self, index: usize) -> Result<*mut UObject, Self::Error>;
    fn import_count(&mut self) -> Result<usize, Self::Error>;
    fn import_at(&mut self, index: usize) -> Result<*mut UObject, Self::Error>;
    fn set_object_usage(&mut self, object: *mut UObject, usage: i32) -> Result<(), Self::Error>;
    fn object_usage(&mut self, object: *mut UObject) -> Result<i32, Self::Error>;

    fn original_export_count(&mut self) -> Result<usize, Self::Error>;
    fn map_original_export_name(&mut self, index: usize) -> Result<(), Self::Error>;
    fn conform_export_count(&mut self) -> Result<usize, Self::Error>;
    fn find_conform_export(&mut self, index: usize) -> Result<Option<usize>, Self::Error>;
    fn conform_export_used(&mut self, index: usize) -> Result<bool, Self::Error>;
    fn set_conform_export_used(&mut self, index: usize) -> Result<(), Self::Error>;
    fn copy_original_export(&mut self, index: usize) -> Result<(), Self::Error>;
    fn add_empty_export(&mut self) -> Result<(), Self::Error>;
    fn check_last_export_matches_original(&mut self, index: usize) -> Result<bool, Self::Error>;
    fn original_export_present(&mut self, index: usize) -> Result<bool, Self::Error>;

    fn initialize_summary(&mut self) -> Result<(), Self::Error>;
    fn serialize_summary(&mut self) -> Result<(), Self::Error>;
    fn tell(&mut self) -> Result<u64, Self::Error>;
    fn seek(&mut self, offset: u64) -> Result<(), Self::Error>;
    fn set_name_offset(&mut self, offset: u64) -> Result<(), Self::Error>;
    fn set_import_offset(&mut self, offset: u64) -> Result<(), Self::Error>;
    fn set_export_offset(&mut self, offset: u64) -> Result<(), Self::Error>;
    fn closing_status(&mut self) -> Result<(), Self::Error>;
    fn update_generation_counts(&mut self) -> Result<(), Self::Error>;
    fn write_import(&mut self, index: usize) -> Result<(), Self::Error>;
    fn write_export_map(&mut self, index: usize) -> Result<(), Self::Error>;
    fn prepare_export(&mut self, index: usize, object: *mut UObject) -> Result<(), Self::Error>;
    fn serialize_object(&mut self, object: *mut UObject) -> Result<(), Self::Error>;
    fn serial_offset(&mut self, index: usize) -> Result<u64, Self::Error>;
    fn set_export_size(&mut self, index: usize, size: i32) -> Result<(), Self::Error>;
    fn finish_summary(&mut self) -> Result<(), Self::Error>;
    fn check(&mut self, condition: bool) -> Result<(), Self::Error>;
}

pub(crate) fn save_package<O: PackageSaveOps>(ops: &mut O) -> Result<(), O::Error> {
    let context = ops.context();
    if context.check { ops.check(!context.outer.is_null())?; }
    ops.clear_save_tags()?;
    tag_exports_with(
        ops,
        context.base,
        context.top_level_flags,
        context.outer,
        |ops| ops.object_count(),
        |ops, index| ops.object_at(index),
        |ops, object| ops.object_flags(object),
        |ops, object, outer| ops.is_in_package(object, outer),
        |ops, object| ops.tag_export(object),
    )?;
    run_save_phases_with(ops, |ops, phase| match phase {
        SavePhase::TagImports => tag_imports(ops),
        SavePhase::ExportNames => export_names(ops),
        SavePhase::InitializeSummary => ops.initialize_summary(),
        SavePhase::BuildNames => build_names(ops),
        SavePhase::SortNames => {
            if context.has_conform { sort_names(ops).map(|_| ()) }
            else { ops.sort_names_from(0) }
        }
        SavePhase::SaveNames => save_names(ops),
        SavePhase::BuildImports => {
            build_object_map(ops, true)?;
            let count = ops.import_count()?;
            ops.set_import_count(count)
        }
        SavePhase::SortImports => ops.sort_imports(),
        SavePhase::BuildExports => build_object_map(ops, false),
        SavePhase::SortExports => {
            let first = if context.has_conform { conform_exports(ops)? } else { 0 };
            ops.sort_exports(first)
        }
        SavePhase::Mappings => build_object_mappings(ops),
        SavePhase::SaveExports => save_exports(ops),
        SavePhase::SaveImports => save_imports(ops),
        SavePhase::SaveExportMap => save_export_map(ops),
        SavePhase::RewriteSummary => rewrite_summary_with(
            ops,
            |ops| ops.closing_status(),
            |ops| ops.update_generation_counts(),
            |ops| ops.seek(0),
            |ops| ops.serialize_summary(),
        ),
    })?;
    ops.finish_summary()
}
/// Tags the base export and each requested top-level object in the selected package.
pub(crate) fn tag_exports_with<S, E>(
    state: &mut S,
    base: *mut UObject,
    top_level_flags: u32,
    outer: *mut UObject,
    mut count: impl FnMut(&mut S) -> Result<usize, E>,
    mut object_at: impl FnMut(&mut S, usize) -> Result<*mut UObject, E>,
    mut object_flags: impl FnMut(&mut S, *mut UObject) -> Result<u32, E>,
    mut is_in_package: impl FnMut(&mut S, *mut UObject, *mut UObject) -> Result<bool, E>,
    mut tag_export: impl FnMut(&mut S, *mut UObject) -> Result<(), E>,
) -> Result<(), E> {
    if !base.is_null() { tag_export(state, base)?; }
    for index in 0..count(state)? {
        let object = object_at(state, index)?;
        if object.is_null() { continue; }
        if object_flags(state, object)? & top_level_flags != 0
            && is_in_package(state, object, outer)?
        {
            tag_export(state, object)?;
        }
    }
    Ok(())
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SavePhase {
    TagImports,
    ExportNames,
    InitializeSummary,
    BuildNames,
    SortNames,
    SaveNames,
    BuildImports,
    SortImports,
    BuildExports,
    SortExports,
    Mappings,
    SaveExports,
    SaveImports,
    SaveExportMap,
    RewriteSummary,
}

/// Authoritative phase order for package save, shared by every owner.
pub(crate) const SAVE_PHASE_ORDER: [SavePhase; 15] = [
    SavePhase::TagImports,
    SavePhase::ExportNames,
    SavePhase::InitializeSummary,
    SavePhase::BuildNames,
    SavePhase::SortNames,
    SavePhase::SaveNames,
    SavePhase::BuildImports,
    SavePhase::SortImports,
    SavePhase::BuildExports,
    SavePhase::SortExports,
    SavePhase::Mappings,
    SavePhase::SaveExports,
    SavePhase::SaveImports,
    SavePhase::SaveExportMap,
    SavePhase::RewriteSummary,
];

/// Runs the shared phase sequence with operations supplied by the concrete owner.
pub(crate) fn run_save_phases_with<S, E>(
    state: &mut S,
    mut run: impl FnMut(&mut S, SavePhase) -> Result<(), E>,
) -> Result<(), E> {
    for phase in SAVE_PHASE_ORDER { run(state, phase)?; }
    Ok(())
}

/// Tags imports referenced by objects marked for package export.
pub(crate) fn tag_imports<O: PackageSaveOps>(ops: &mut O) -> Result<(), O::Error> {
    let context = ops.context();
    tag_imports_with(
        ops,
        context.tag_export,
        |ops| ops.object_count(),
        |ops, index| ops.object_at(index),
        |ops, object| ops.object_flags(object),
        |ops, object| ops.tag_import(object),
    )
}

/// Tags each export object's references with owner-specific archive behavior.
pub(crate) fn tag_imports_with<S, E>(
    state: &mut S,
    tag_export: u32,
    mut count: impl FnMut(&mut S) -> Result<usize, E>,
    mut object_at: impl FnMut(&mut S, usize) -> Result<*mut UObject, E>,
    mut flags: impl FnMut(&mut S, *mut UObject) -> Result<u32, E>,
    mut visit_imports: impl FnMut(&mut S, *mut UObject) -> Result<(), E>,
) -> Result<(), E> {
    for index in 0..count(state)? {
        let object = object_at(state, index)?;
        if object.is_null() { continue; }
        if flags(state, object)? & tag_export != 0 { visit_imports(state, object)?; }
    }
    Ok(())
}

/// Marks names needed by exported and imported objects, outers, and classes.
pub(crate) fn export_names<O: PackageSaveOps>(ops: &mut O) -> Result<(), O::Error> {
    let context = ops.context();
    export_names_with(
        ops,
        context.tag_export,
        context.tag_import,
        context.export_name_flags,
        context.public_flag,
        context.check,
        |ops| ops.object_count(),
        |ops, index| ops.object_at(index),
        |ops, object| ops.object_flags(object),
        |ops, object| ops.object_name(object),
        |ops, object| ops.object_outer(object),
        |ops, object| ops.object_class(object),
        |ops, name, flags| ops.set_object_name_flags(name, flags),
        |ops, condition| ops.check(condition),
        |ops, object| ops.report_private_import(object),
        |ops, object| ops.report_saving_object(object),
    )
}

/// Applies the ordered name/class traversal used by every package-save adapter.
pub(crate) fn export_names_with<S, E>(
    state: &mut S,
    tag_export: u32,
    tag_import: u32,
    name_flags: u32,
    public_flag: u32,
    check: bool,
    mut count: impl FnMut(&mut S) -> Result<usize, E>,
    mut object_at: impl FnMut(&mut S, usize) -> Result<*mut UObject, E>,
    mut flags: impl FnMut(&mut S, *mut UObject) -> Result<u32, E>,
    mut name: impl FnMut(&mut S, *mut UObject) -> Result<NameIndex, E>,
    mut outer: impl FnMut(&mut S, *mut UObject) -> Result<*mut UObject, E>,
    mut class: impl FnMut(&mut S, *mut UObject) -> Result<*mut UObject, E>,
    mut set_name_flags: impl FnMut(&mut S, NameIndex, u32) -> Result<(), E>,
    mut check_value: impl FnMut(&mut S, bool) -> Result<(), E>,
    mut private_import: impl FnMut(&mut S, *mut UObject) -> Result<(), E>,
    mut saving_object: impl FnMut(&mut S, *mut UObject) -> Result<(), E>,
) -> Result<(), E> {
    for index in 0..count(state)? {
        let object = object_at(state, index)?;
        if object.is_null() { continue; }
        let object_flags = flags(state, object)?;
        if object_flags & (tag_export | tag_import) == 0 { continue; }
        let object_name = name(state, object)?;
        set_name_flags(state, object_name, name_flags)?;
        let object_outer = outer(state, object)?;
        if !object_outer.is_null() {
            let outer_name = name(state, object_outer)?;
            set_name_flags(state, outer_name, name_flags)?;
        }
        if object_flags & tag_import != 0 {
            let object_class = class(state, object)?;
            let class_name = name(state, object_class)?;
            set_name_flags(state, class_name, name_flags)?;
            let class_outer = outer(state, object_class)?;
            if check { check_value(state, !class_outer.is_null())?; }
            let class_outer_name = name(state, class_outer)?;
            set_name_flags(state, class_outer_name, name_flags)?;
            if object_flags & public_flag == 0 { private_import(state, object)?; }
        } else {
            saving_object(state, object)?;
        }
    }
    Ok(())
}

/// Adds tagged names to the package name map in canonical name traversal order.
pub(crate) fn build_names<O: PackageSaveOps>(ops: &mut O) -> Result<(), O::Error> {
    let context = ops.context();
    build_names_with(
        ops,
        context.tag_export,
        |ops| ops.tell(),
        |ops, offset| ops.set_name_offset(offset),
        |ops| ops.global_name_count(),
        |_ops, index| Ok(index as i32),
        |ops, name| ops.global_name_flags(name as usize),
        |ops, name| ops.add_name(name),
    )
}

/// Sets the name-table offset and appends tagged names in canonical table order.
pub(crate) fn build_names_with<S, E>(
    state: &mut S,
    tag_export: u32,
    mut tell: impl FnMut(&mut S) -> Result<u64, E>,
    mut set_offset: impl FnMut(&mut S, u64) -> Result<(), E>,
    mut count: impl FnMut(&mut S) -> Result<usize, E>,
    mut name_at: impl FnMut(&mut S, usize) -> Result<NameIndex, E>,
    mut name_flags: impl FnMut(&mut S, NameIndex) -> Result<u32, E>,
    mut add_name: impl FnMut(&mut S, NameIndex) -> Result<(), E>,
) -> Result<(), E> {
    let offset = tell(state)?;
    set_offset(state, offset)?;
    for index in 0..count(state)? {
        let name = name_at(state, index)?;
        if name_flags(state, name)? & tag_export != 0 { add_name(state, name)?; }
    }
    Ok(())
}

/// Builds the import or export map from tagged objects.
pub(crate) fn build_object_map<O: PackageSaveOps>(ops: &mut O, imports: bool) -> Result<(), O::Error> {
    let context = ops.context();
    build_object_map_with(
        ops,
        imports,
        context.tag_import,
        context.tag_export,
        |ops| ops.object_count(),
        |ops, index| ops.object_at(index),
        |ops, object| ops.object_flags(object),
        |ops, object| ops.add_import(object),
        |ops, object| ops.add_export(object),
    )
}
/// Publishes positive export and negative import indices through owner-supplied mapping leaves.
pub(crate) fn build_object_mappings_with<S, E>(
    state: &mut S,
    mut export_count: impl FnMut(&mut S) -> Result<usize, E>,
    mut export_at: impl FnMut(&mut S, usize) -> Result<*mut UObject, E>,
    mut import_count: impl FnMut(&mut S) -> Result<usize, E>,
    mut import_at: impl FnMut(&mut S, usize) -> Result<*mut UObject, E>,
    mut set_usage: impl FnMut(&mut S, *mut UObject, i32) -> Result<(), E>,
) -> Result<(), E> {
    for index in 0..export_count(state)? {
        let object = export_at(state, index)?;
        if !object.is_null() { set_usage(state, object, index as i32 + 1)?; }
    }
    for index in 0..import_count(state)? {
        let object = import_at(state, index)?;
        set_usage(state, object, -(index as i32) - 1)?;
    }
    Ok(())
}

/// Saves names and publishes their package-map indices.
pub(crate) fn save_names<O: PackageSaveOps>(ops: &mut O) -> Result<(), O::Error> {
    save_names_with(
        ops,
        |ops| ops.name_count(),
        |ops, index| ops.name_at(index),
        |ops, count| ops.set_name_count(count),
        |ops, name| ops.save_name(name),
        |ops, name, usage| ops.set_name_usage(name, usage),
    )
}

/// Runs name-table serialization against an adapter without owning its archive.
pub(crate) fn save_names_with<S, E>(
    state: &mut S,
    mut count: impl FnMut(&mut S) -> Result<usize, E>,
    mut name_at: impl FnMut(&mut S, usize) -> Result<NameIndex, E>,
    mut set_count: impl FnMut(&mut S, usize) -> Result<(), E>,
    mut write_name: impl FnMut(&mut S, NameIndex) -> Result<(), E>,
    mut set_usage: impl FnMut(&mut S, NameIndex, i32) -> Result<(), E>,
) -> Result<(), E> {
    let count = count(state)?;
    set_count(state, count)?;
    for index in 0..count {
        let name = name_at(state, index)?;
        write_name(state, name)?;
        set_usage(state, name, index as i32)?;
    }
    Ok(())
}
/// Builds import or export entries through adapter-supplied canonical lookups.
pub(crate) fn build_object_map_with<S, E>(
    state: &mut S,
    imports: bool,
    tag_import: u32,
    tag_export: u32,
    mut count: impl FnMut(&mut S) -> Result<usize, E>,
    mut object_at: impl FnMut(&mut S, usize) -> Result<*mut UObject, E>,
    mut flags: impl FnMut(&mut S, *mut UObject) -> Result<u32, E>,
    mut add_import: impl FnMut(&mut S, *mut UObject) -> Result<(), E>,
    mut add_export: impl FnMut(&mut S, *mut UObject) -> Result<(), E>,
) -> Result<(), E> {
    let flag = if imports { tag_import } else { tag_export };
    for index in 0..count(state)? {
        let object = object_at(state, index)?;
        if object.is_null() { continue; }
        if flags(state, object)? & flag != 0 {
            if imports { add_import(state, object)?; } else { add_export(state, object)?; }
        }
    }
    Ok(())
}

/// Applies original name order to the current map and returns its conform prefix length.
pub(crate) fn sort_names<O: PackageSaveOps>(ops: &mut O) -> Result<usize, O::Error> {
    let context = ops.context();
    let count = conform_name_prefix_with(
        ops,
        |ops| ops.conform_name_count(),
        |ops, index| ops.conform_name_at(index),
        |ops, name| ops.find_name(name),
        |ops, index| ops.name_at(index),
        |ops| ops.append_name_slot(),
        |ops, index, name| ops.set_name_at(index, name),
        |ops, left, right| ops.swap_names(left, right),
    )?;
    check_conform_names_with(
        ops,
        context.slow,
        context.check,
        |ops| ops.conform_name_count(),
        |ops, index| ops.conform_name_at(index),
        |ops, index| ops.name_at(index),
        |ops| ops.original_name_count(),
        |ops, index| ops.original_name_at(index),
        |ops, name| ops.find_name(name),
        |ops, condition| ops.check(condition),
    )?;
    ops.sort_names_from(count)?;
    Ok(count)
}

/// Applies the conform name-table prefix ordering shared with the native adapter.
pub(crate) fn conform_name_prefix_with<S, E>(
    state: &mut S,
    mut conform_count: impl FnMut(&mut S) -> Result<usize, E>,
    mut conform_at: impl FnMut(&mut S, usize) -> Result<NameIndex, E>,
    mut find_name: impl FnMut(&mut S, NameIndex) -> Result<Option<usize>, E>,
    mut name_at: impl FnMut(&mut S, usize) -> Result<NameIndex, E>,
    mut append_slot: impl FnMut(&mut S) -> Result<usize, E>,
    mut set_name: impl FnMut(&mut S, usize, NameIndex) -> Result<(), E>,
    mut swap_names: impl FnMut(&mut S, usize, usize) -> Result<(), E>,
) -> Result<usize, E> {
    let count = conform_count(state)?;
    for index in 0..count {
        let name = conform_at(state, index)?;
        let existing = find_name(state, name)?;
        if name == 0 || existing.is_none() {
            let last = append_slot(state)?;
            let moved = name_at(state, index)?;
            set_name(state, last, moved)?;
        } else if let Some(existing) = existing {
            swap_names(state, index, existing)?;
        }
        set_name(state, index, name)?;
    }
    Ok(count)
}
/// Preserves the legacy slow checks for conform-prefix equality and original-name presence.
pub(crate) fn check_conform_names_with<S, E>(
    state: &mut S,
    slow: bool,
    check: bool,
    mut conform_count: impl FnMut(&mut S) -> Result<usize, E>,
    mut conform_at: impl FnMut(&mut S, usize) -> Result<NameIndex, E>,
    mut name_at: impl FnMut(&mut S, usize) -> Result<NameIndex, E>,
    mut original_count: impl FnMut(&mut S) -> Result<usize, E>,
    mut original_at: impl FnMut(&mut S, usize) -> Result<NameIndex, E>,
    mut find_name: impl FnMut(&mut S, NameIndex) -> Result<Option<usize>, E>,
    mut check_value: impl FnMut(&mut S, bool) -> Result<(), E>,
) -> Result<(), E> {
    if slow {
        for index in 0..conform_count(state)? {
            if check {
                let got = name_at(state, index)?;
                let wanted = conform_at(state, index)?;
                check_value(state, got == wanted)?;
            }
        }
        for index in 0..original_count(state)? {
            if check {
                let name = original_at(state, index)?;
                let present = find_name(state, name)?.is_some();
                check_value(state, present)?;
            }
        }
    }
    Ok(())
}

/// Builds original export correspondence, conform ordering, and retained exports.
pub(crate) fn conform_exports<O: PackageSaveOps>(ops: &mut O) -> Result<usize, O::Error> {
    let context = ops.context();
    conform_exports_with(
        ops,
        context.check,
        context.slow,
        |ops| ops.original_export_count(),
        |ops, index| ops.map_original_export_name(index),
        |ops| ops.conform_export_count(),
        |ops, index| ops.find_conform_export(index),
        |ops, index| ops.copy_original_export(index),
        |ops, index| ops.check_last_export_matches_original(index),
        |ops, index| ops.set_conform_export_used(index),
        |ops| ops.add_empty_export(),
        |ops, index| ops.conform_export_used(index),
        |ops, index| ops.original_export_present(index),
        |ops, condition| ops.check(condition),
    )
}

/// Matches conform exports to current candidates, then appends unused candidates.
pub(crate) fn conform_exports_with<S, E>(
    state: &mut S,
    check: bool,
    slow: bool,
    mut original_count: impl FnMut(&mut S) -> Result<usize, E>,
    mut map_original_name: impl FnMut(&mut S, usize) -> Result<(), E>,
    mut conform_count: impl FnMut(&mut S) -> Result<usize, E>,
    mut find_conform: impl FnMut(&mut S, usize) -> Result<Option<usize>, E>,
    mut copy_original: impl FnMut(&mut S, usize) -> Result<(), E>,
    mut check_last_matches: impl FnMut(&mut S, usize) -> Result<bool, E>,
    mut set_used: impl FnMut(&mut S, usize) -> Result<(), E>,
    mut add_empty: impl FnMut(&mut S) -> Result<(), E>,
    mut is_used: impl FnMut(&mut S, usize) -> Result<bool, E>,
    mut original_present: impl FnMut(&mut S, usize) -> Result<bool, E>,
    mut check_value: impl FnMut(&mut S, bool) -> Result<(), E>,
) -> Result<usize, E> {
    let original_count = original_count(state)?;
    for index in 0..original_count { map_original_name(state, index)?; }
    let conform_count = conform_count(state)?;
    for index in 0..conform_count {
        if let Some(original) = find_conform(state, index)? {
            copy_original(state, original)?;
            if check {
                let matches = check_last_matches(state, original)?;
                check_value(state, matches)?;
            }
            set_used(state, original)?;
        } else {
            add_empty(state)?;
        }
    }
    for index in 0..original_count {
        if !is_used(state, index)? { copy_original(state, index)?; }
    }
    if slow {
        for index in 0..original_count {
            let present = original_present(state, index)?;
            if check { check_value(state, present)?; }
        }
    }
    Ok(conform_count)
}

/// Assigns package mapping indices to all exports followed by all imports.
pub(crate) fn build_object_mappings<O: PackageSaveOps>(ops: &mut O) -> Result<(), O::Error> {
    build_object_mappings_with(
        ops,
        |ops| ops.export_count(),
        |ops, index| ops.export_at(index),
        |ops| ops.import_count(),
        |ops, index| ops.import_at(index),
        |ops, object, usage| ops.set_object_usage(object, usage),
    )
}

/// Writes each export body and records its byte length.
pub(crate) fn save_exports<O: PackageSaveOps>(ops: &mut O) -> Result<(), O::Error> {
    save_exports_with(
        ops,
        |ops| ops.export_count(),
        |ops, index| ops.export_at(index),
        |ops, index, object| ops.prepare_export(index, object),
        |ops, object| ops.serialize_object(object),
        |ops| ops.tell(),
        |ops, index| ops.serial_offset(index),
        |ops, index, size| ops.set_export_size(index, size),
    )
}

/// Writes imports at the current archive position.
pub(crate) fn save_imports<O: PackageSaveOps>(ops: &mut O) -> Result<(), O::Error> {
    save_imports_with(
        ops,
        |ops| ops.tell(),
        |ops, offset| ops.set_import_offset(offset),
        |ops| ops.import_count(),
        |ops, index| ops.write_import(index),
    )
}

/// Writes imports using the adapter's native table and archive leaves.
pub(crate) fn save_imports_with<S, E>(
    state: &mut S,
    mut tell: impl FnMut(&mut S) -> Result<u64, E>,
    mut set_offset: impl FnMut(&mut S, u64) -> Result<(), E>,
    mut count: impl FnMut(&mut S) -> Result<usize, E>,
    mut write: impl FnMut(&mut S, usize) -> Result<(), E>,
) -> Result<(), E> {
    let offset = tell(state)?;
    set_offset(state, offset)?;
    for index in 0..count(state)? { write(state, index)?; }
    Ok(())
}

/// Writes export map records at the current archive position.
pub(crate) fn save_export_map<O: PackageSaveOps>(ops: &mut O) -> Result<(), O::Error> {
    save_export_map_with(
        ops,
        |ops| ops.tell(),
        |ops, offset| ops.set_export_offset(offset),
        |ops| ops.export_count(),
        |ops, index| ops.write_export_map(index),
    )
}

/// Writes export-map records using the adapter's native table and archive leaves.
pub(crate) fn save_export_map_with<S, E>(
    state: &mut S,
    mut tell: impl FnMut(&mut S) -> Result<u64, E>,
    mut set_offset: impl FnMut(&mut S, u64) -> Result<(), E>,
    mut count: impl FnMut(&mut S) -> Result<usize, E>,
    mut write: impl FnMut(&mut S, usize) -> Result<(), E>,
) -> Result<(), E> {
    let offset = tell(state)?;
    set_offset(state, offset)?;
    for index in 0..count(state)? { write(state, index)?; }
    Ok(())
}
/// Serializes each export and computes its serialized byte size through owner operations.
pub(crate) fn save_exports_with<S, E>(
    state: &mut S,
    mut count: impl FnMut(&mut S) -> Result<usize, E>,
    mut object_at: impl FnMut(&mut S, usize) -> Result<*mut UObject, E>,
    mut prepare: impl FnMut(&mut S, usize, *mut UObject) -> Result<(), E>,
    mut serialize: impl FnMut(&mut S, *mut UObject) -> Result<(), E>,
    mut tell: impl FnMut(&mut S) -> Result<u64, E>,
    mut serial_offset: impl FnMut(&mut S, usize) -> Result<u64, E>,
    mut set_size: impl FnMut(&mut S, usize, i32) -> Result<(), E>,
) -> Result<(), E> {
    for index in 0..count(state)? {
        let object = object_at(state, index)?;
        if object.is_null() { continue; }
        prepare(state, index, object)?;
        serialize(state, object)?;
        let end = tell(state)?;
        let start = serial_offset(state, index)?;
        set_size(state, index, (end as i32).wrapping_sub(start as i32))?;
    }
    Ok(())
}

/// Returns the legacy qsort difference: second mapped key minus first.
pub(crate) fn compare_qsort_usage<O: PackageSaveOps>(
    ops: &mut O,
    first: *mut UObject,
    second: *mut UObject,
) -> Result<i32, O::Error> {
    let first = ops.object_usage(first)?;
    let second = ops.object_usage(second)?;
    Ok(second.wrapping_sub(first))
}

/// Compares mapped names in native qsort operand order (second minus first).
pub(crate) fn compare_qsort_name_usage<O: PackageSaveOps>(
    ops: &mut O,
    first: NameIndex,
    second: NameIndex,
) -> Result<i32, O::Error> {
    let first = ops.name_usage(first)?;
    let second = ops.name_usage(second)?;
    Ok(second.wrapping_sub(first))
}

/// Applies native qsort's wrapping difference: second mapped key minus first.
pub(crate) const fn compare_qsort_mapped_values(first: i32, second: i32) -> i32 {
    second.wrapping_sub(first)
}

/// Runs summary finalization in native order before seeking back to rewrite it.
pub(crate) fn rewrite_summary_with<S, E>(
    state: &mut S,
    mut closing_status: impl FnMut(&mut S) -> Result<(), E>,
    mut update_generation_counts: impl FnMut(&mut S) -> Result<(), E>,
    mut seek_start: impl FnMut(&mut S) -> Result<(), E>,
    mut serialize: impl FnMut(&mut S) -> Result<(), E>,
) -> Result<(), E> {
    closing_status(state)?;
    update_generation_counts(state)?;
    seek_start(state)?;
    serialize(state)
}
