//! M3 — deserialize script objects from package archives into the arena.
//!
//! Layouts mirror the observed v79 wire format, verified byte-for-byte
//! against `UStruct::Serialize` / `UClass::Serialize` in the frozen engine
//! source and hand-decoded stock payloads:
//! - field exports lead with `[superfield ref][next ref]`;
//! - struct-like exports continue `[script-text ref][children ref]
//!   [friendly name][line i32][text-pos i32][script size RAW I32][code]`;
//!   states/functions/structs carry one extra leading null ref (five refs
//!   total before the friendly name), classes carry exactly four;
//! - classes append flags/guid/dependencies/package-imports/within/config
//!   followed by tagged default properties;
//! - functions end with `[i-native u16][precedence u8][flags u32]
//!   [+ rep-offset u16 when net]` anchored at payload end;
//! - states end with `[probe u64][ignore u64][labeloff u16][flags u32]`.
//!
//! The stored script size is a raw little-endian I32 (not a compact index)
//! but is NOT a reliable code-length bound for functions and states (the HP2
//! compiler overcounts), so executable regions are cut at end-anchored
//! trailers instead.
//!
//! Imports reuse already-present arena objects when their dotted path matches
//! something previously loaded, which stitches Core ← Engine ← HGame.

use crate::arena::{
    ClassData, FieldLinks, FunctionData, ObjectArena, ObjectData, ObjectId, PropertyData,
    StateData, StructData,
};
use crate::error::{Fail, Result};
use crate::props::PropStore;
use crate::value::{PropValue, PropertyKind, decode_value};
use hp_format::package79::{
    ByteCursor, PackageArchive, PropertyTag, read_compact_index, read_fstring, read_package,
};

/// Minimum package version whose script-object layout this module decodes.
/// Below 77 the conversion opcodes are numbered one lower and function
/// trailers shrink; the stock packages are all 79.
pub const MIN_SCRIPT_VERSION: i32 = 62;
/// Packages below this carry the old opcode numbering; rejected loudly.
pub const MIN_TOKEN_VERSION: i32 = 77;

/// `FUNC_Net` — gates the trailing rep-offset word.
const FUNC_NET: u32 = 0x0000_0040;
/// `CPF_Net` — gates the property's trailing rep-offset word.
const CPF_NET: u32 = 0x0000_0020;

/// Tagged-property kind nibble for `UBoolProperty` (classic NAME_ ordinal).
const TAG_BOOL: u8 = 3;

#[derive(Debug, Clone)]
pub struct LoadedPackage {
    pub root: ObjectId,
    /// Local name-table index → shared pool index.
    pub name_map: Vec<u32>,
    /// Object id of every export, in export-table order (bind uses this to
    /// distinguish typed exports from stitched import skeletons).
    pub export_ids: Vec<ObjectId>,
}

/// Everything a load needs to resolve references.
struct Ctx<'a> {
    archive: &'a PackageArchive,
    /// Local archive names, for the tag reader.
    local_names: &'a [hp_format::package79::NameEntry],
    name_map: &'a [u32],
    import_ids: Vec<Option<ObjectId>>,
    export_ids: Vec<ObjectId>,
    #[allow(dead_code)]
    version: i32,
}

impl<'a> Ctx<'a> {
    fn resolve(&self, reference: i32) -> Option<Option<ObjectId>> {
        match reference.cmp(&0) {
            std::cmp::Ordering::Equal => Some(None),
            std::cmp::Ordering::Greater => self
                .export_ids
                .get(reference as usize - 1)
                .copied()
                .map(Some),
            std::cmp::Ordering::Less => self.import_ids.get((-reference - 1) as usize).copied(),
        }
    }

    fn global_name(&self, local_index: i32) -> Result<u32> {
        let local = usize::try_from(local_index)
            .map_err(|_| Fail::new("bind.negative_name_index", format!("{local_index}")))?;
        self.name_map.get(local).copied().ok_or_else(|| {
            Fail::new(
                "bind.name_index_range",
                format!(
                    "name index {local_index} outside {} entries",
                    self.name_map.len()
                ),
            )
        })
    }

    fn payload(&self, index: usize) -> Result<&'a [u8]> {
        self.archive.export_payload(index).ok_or_else(|| {
            Fail::new(
                "bind.export_payload_missing",
                format!("export {index} has no serial payload"),
            )
        })
    }
}

/// Load one archive into the arena. Gaps (exports that could not be typed or
/// parsed) are reported to stderr with reason codes and retained as raw
/// payload bytes — never silently dropped.
pub fn load_package(
    arena: &mut ObjectArena,
    package_name: &str,
    source: &[u8],
) -> Result<LoadedPackage> {
    let archive = read_package(source)?;
    if archive.summary.version < MIN_SCRIPT_VERSION || archive.summary.version < MIN_TOKEN_VERSION {
        return Err(Fail::new(
            "bind.unsupported_version",
            format!(
                "{package_name}: version {} predates the v{} script-layout floor (pre-{} packages renumber conversion opcodes)",
                archive.summary.version, MIN_TOKEN_VERSION, MIN_TOKEN_VERSION
            ),
        ));
    }

    // Merge this package's name table into the shared pool; cross-package
    // repeats fold onto the existing global entry.
    let mut name_map = Vec::with_capacity(archive.names.len());
    for entry in &archive.names {
        name_map.push(arena.names.intern(&entry.text));
    }

    let root = arena.root_for(package_name);

    // Pass 1a: imports, parents before children (import outers always point
    // at earlier entries or zero).
    let mut import_ids: Vec<Option<ObjectId>> = vec![None; archive.imports.len()];
    for i in 0..archive.imports.len() {
        let entry = archive.imports[i];
        let name_index = *name_map
            .get(entry.object_name_index.max(0) as usize)
            .ok_or_else(|| Fail::new("bind.name_index_range", "import name"))?;
        let parent: Option<ObjectId> = if entry.outer_ref == 0 {
            None
        } else {
            import_ids[(-entry.outer_ref - 1) as usize]
        };
        let mut path = String::new();
        if let Some(parent) = parent {
            path.push_str(&arena.path_of(parent)?);
            path.push('.');
        }
        path.push_str(arena.names.text(name_index).unwrap_or("?"));
        import_ids[i] = Some(match arena.find_by_path(&path) {
            Some(existing) => existing,
            None => arena.alloc(crate::arena::UObject {
                class_id: None,
                name_index,
                outer: parent,
                flags: 0,
                data: ObjectData::Empty,
            }),
        });
    }
    // Pass 1b: export skeletons. Allocate every slot FIRST so that forward
    // outer references (a field exported before its owning class) resolve;
    // stock packages list some subobjects ahead of their outers.
    let mut export_ids: Vec<ObjectId> = Vec::with_capacity(archive.exports.len());
    for _ in &archive.exports {
        export_ids.push(arena.alloc(crate::arena::UObject::default()));
    }
    for (index, entry) in archive.exports.iter().enumerate() {
        let name_index = *name_map
            .get(entry.object_name_index.max(0) as usize)
            .ok_or_else(|| Fail::new("bind.name_index_range", "export name"))?;
        let outer = if entry.outer_ref == 0 {
            Some(root)
        } else if entry.outer_ref > 0 {
            export_ids.get(entry.outer_ref as usize - 1).copied()
        } else {
            import_ids[(-entry.outer_ref - 1) as usize]
        };
        let object = arena.get_mut(export_ids[index])?;
        object.class_id = None;
        object.name_index = name_index;
        object.outer = outer;
        object.flags = entry.object_flags;
        object.data = ObjectData::Empty;
    }

    // Reconcile import skeletons against this archive's own exports: an
    // import may reference an object exported by the same file (e.g. a
    // class's superclass entry), which did not exist when imports were
    // stitched. References resolve through these ids lazily, so remapping
    // here fixes every consumer.
    {
        let first_export = export_ids
            .first()
            .copied()
            .map(|id| id.0)
            .unwrap_or(u32::MAX);
        for slot in import_ids.iter_mut() {
            if let Some(sid) = *slot
                && sid.0 < first_export
            {
                // import skeleton (allocated before this file's exports)
                let path = arena.path_of(sid)?;
                if let Some(real) = arena.find_by_path(&path)
                    && real.0 >= first_export
                {
                    *slot = Some(real);
                }
            }
        }
    }

    let mut ctx = Ctx {
        archive: &archive,
        local_names: &archive.names,
        name_map: &name_map,
        import_ids,
        export_ids,
        version: archive.summary.version,
    };

    // Pass 2: fill payloads.
    let mut gaps: Vec<(ObjectId, Fail)> = Vec::new();
    let mut deferred_classes: Vec<usize> = Vec::new();
    for index in 0..ctx.export_ids.len() {
        let kind = export_kind(arena, &ctx, index);
        match kind {
            ExportKind::Class => deferred_classes.push(index),
            ExportKind::Raw => {
                let id = ctx.export_ids[index];
                let class_id = ctx.resolve(ctx.archive.exports[index].class_ref).flatten();
                if let Ok(object) = arena.get_mut(id) {
                    object.class_id = class_id;
                    object.data = ObjectData::Raw(ctx.payload(index)?.to_vec());
                }
            }
            _ => {
                if let Err(fail) = load_export(arena, &mut ctx, index, kind) {
                    gaps.push((ctx.export_ids[index], fail));
                }
            }
        }
    }

    // Classes need their superclass parsed first (default inheritance walks
    // templates up the chain), so run them via fixpoint passes.
    loop {
        let mut progressed = false;
        let mut still_pending = Vec::new();
        for index in deferred_classes {
            match load_export(arena, &mut ctx, index, ExportKind::Class) {
                Ok(()) => progressed = true,
                Err(fail) if fail.reason_code == "bind.superclass_unparsed" => {
                    still_pending.push(index)
                }
                Err(fail) => gaps.push((ctx.export_ids[index], fail)),
            }
        }
        deferred_classes = still_pending;
        if deferred_classes.is_empty() || !progressed {
            break;
        }
    }
    for index in deferred_classes {
        let sup_dbg = ctx
            .archive
            .exports
            .get(index)
            .map(|e| {
                ctx.resolve(e.super_ref)
                    .flatten()
                    .and_then(|sid| {
                        arena.get(sid).ok().map(|o| {
                            format!("{:?} data={:?}", sid, std::mem::discriminant(&o.data))
                        })
                    })
                    .unwrap_or_else(|| format!("rawref={}", e.super_ref))
            })
            .unwrap_or_default();
        gaps.push((
            ctx.export_ids[index],
            Fail::new(
                "bind.class_cycle",
                format!("class never resolved its superclass ({sup_dbg})"),
            ),
        ));
    }

    for (id, fail) in &gaps {
        let subject = arena.path_of(*id).unwrap_or_else(|_| format!("{id:?}"));
        let cls_path = arena
            .get(*id)
            .ok()
            .and_then(|o| o.class_id)
            .and_then(|c| arena.path_of(c).ok())
            .unwrap_or_default();
        eprintln!(
            "hp-uobject: [{}] at {} (class={}): {}",
            fail.reason_code, subject, cls_path, fail.message
        );
        let raw_index = ctx.export_ids.iter().position(|e| e == id);
        let payload = raw_index
            .and_then(|i| ctx.archive.export_payload(i))
            .unwrap_or(&[]);
        if let Ok(object) = arena.get_mut(*id) {
            object.data = ObjectData::Raw(payload.to_vec());
        }
    }
    if !gaps.is_empty() {
        eprintln!("hp-uobject: {package_name}: {} gap(s)", gaps.len());
    }

    let export_ids = ctx.export_ids.clone();
    drop(ctx);
    Ok(LoadedPackage {
        root,
        name_map,
        export_ids,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExportKind {
    Class,
    Function,
    State,
    ScriptStruct,
    Const,
    Enum,
    TextBuffer,
    Property { folded_class: String },
    Raw,
}

/// Identify an export's parser from its resolved class name. Null class refs
/// mark UClass exports in these packages (the audit `Core.Class` convention).
fn export_kind(arena: &ObjectArena, ctx: &Ctx<'_>, index: usize) -> ExportKind {
    let entry = &ctx.archive.exports[index];
    if entry.class_ref == 0 {
        return ExportKind::Class;
    }
    let Some(Some(class_id)) = ctx.resolve(entry.class_ref) else {
        return ExportKind::Raw;
    };
    let folded = arena
        .get(class_id)
        .ok()
        .and_then(|o| arena.names.text(o.name_index).map(crate::name::fold_key))
        .unwrap_or_default();
    match folded.as_str() {
        "class" => ExportKind::Class,
        "function" => ExportKind::Function,
        "state" => ExportKind::State,
        "struct" => ExportKind::ScriptStruct,
        "const" => ExportKind::Const,
        "enum" => ExportKind::Enum,
        "textbuffer" => ExportKind::TextBuffer,
        n if is_property_class(n) => ExportKind::Property {
            folded_class: n.to_string(),
        },
        _ => ExportKind::Raw,
    }
}

fn is_property_class(folded: &str) -> bool {
    matches!(
        folded,
        "byteproperty"
            | "intproperty"
            | "boolproperty"
            | "floatproperty"
            | "objectproperty"
            | "classproperty"
            | "nameproperty"
            | "stringproperty"
            | "strproperty"
            | "structproperty"
            | "arrayproperty"
            | "fixedarrayproperty"
            | "mapproperty"
            | "delegateproperty"
            | "componentproperty"
            | "interfaceproperty"
    )
}

fn load_export(
    arena: &mut ObjectArena,
    ctx: &mut Ctx<'_>,
    index: usize,
    kind: ExportKind,
) -> Result<()> {
    let id = ctx.export_ids[index];
    let payload_len = ctx.payload(index)?.len();
    let entry = ctx.archive.exports[index];
    let class_id = ctx.resolve(entry.class_ref).flatten();
    let mut payload_cur = ByteCursor::at(ctx.payload(index)?, 0);
    let cur = &mut payload_cur;
    // End offset every parser must land on exactly (loud validation).
    let consumed = |cur: &ByteCursor<'_>| cur.position();

    match kind {
        ExportKind::Property { folded_class } => {
            let data = parse_property(arena, ctx, cur, &folded_class)?;
            if consumed(cur) != payload_len {
                return Err(Fail::new(
                    "bind.property_trailing_bytes",
                    format!(
                        "{} of {} byte(s) unconsumed",
                        payload_len - consumed(cur),
                        payload_len
                    ),
                ));
            }
            let object = arena.get_mut(id)?;
            object.class_id = class_id;
            object.data = ObjectData::Property(Box::new(data));
            Ok(())
        }
        ExportKind::Function => {
            // Trailer is located from the END (the same derivation the
            // Phase-1 audit proved across every stock package, oracle slots
            // included); the stored script size can disagree with the
            // distance to the trailer, so the executable region is taken as
            // everything between the header and the anchored trailer.
            let expected = Some(ctx.global_name(ctx.archive.exports[index].object_name_index)?);
            let head = read_struct_head(ctx, cur, HeadRefs::Five, "function", expected)?;
            let _stored_size = read_script_size(cur, "function")?;
            let code_start = cur.position();
            let trailer = derive_function_trailer(cur.data(), code_start)?;
            let code = if code_start <= trailer.offset {
                cur.data()[code_start..trailer.offset].to_vec()
            } else {
                Vec::new()
            };
            cur.seek(
                trailer.offset
                    + if trailer.function_flags & FUNC_NET != 0 {
                        9
                    } else {
                        7
                    },
            );
            let object = arena.get_mut(id)?;
            object.class_id = class_id;
            object.data = ObjectData::Function(Box::new(FunctionData {
                links: head.links,
                native_index: trailer.native_index,
                operator_precedence: trailer.precedence,
                function_flags: trailer.function_flags,
                code,
                params: Vec::new(),
                locals: Vec::new(),
            }));
            Ok(())
        }
        ExportKind::State => {
            let expected = Some(ctx.global_name(ctx.archive.exports[index].object_name_index)?);
            let head = read_struct_head(ctx, cur, HeadRefs::Five, "state", expected)?;
            let _stored_size = read_script_size(cur, "state")?;
            // Fixed 22-byte state tail from `UState::Serialize`, anchored at
            // the payload end ([probe u64][ignore u64][labeloff u16][flags u32]).
            const STATE_TAIL: usize = 8 + 8 + 2 + 4;
            let code_start = cur.position();
            if payload_len < STATE_TAIL || code_start > payload_len - STATE_TAIL {
                return Err(Fail::new(
                    "bind.state_payload_too_small",
                    format!(
                        "payload of {} byte(s) cannot hold header plus {}-byte state tail",
                        payload_len, STATE_TAIL
                    ),
                ));
            }
            let code_end = payload_len - STATE_TAIL;
            cur.seek(code_end);
            let probe_mask = u64::from_le_bytes(cur.take(8)?.try_into().unwrap());
            let ignore_mask = u64::from_le_bytes(cur.take(8)?.try_into().unwrap());
            let label_table_offset = cur.u16()?;
            let state_flags = cur.u32()?;
            if cur.position() != payload_len {
                return Err(Fail::new("bind.state_trailing_bytes", "tail overrun"));
            }
            let object = arena.get_mut(id)?;
            object.class_id = class_id;
            object.data = ObjectData::State(Box::new(StateData {
                links: head.links,
                probe_mask,
                ignore_mask,
                state_flags,
                label_table_offset,
                code: ctx.payload(index)?[code_start..code_end].to_vec(),
            }));
            Ok(())
        }
        ExportKind::ScriptStruct => {
            let expected = Some(ctx.global_name(ctx.archive.exports[index].object_name_index)?);
            let head = read_struct_head(ctx, cur, HeadRefs::Five, "script struct", expected)?;
            let stored_size = read_script_size(cur, "script struct")?;
            // Script structs always carry their exact bytecode; the region
            // after the size field runs to the payload end.
            let code_start = cur.position();
            if stored_size < 0 || code_start + stored_size as usize > payload_len {
                return Err(Fail::new(
                    "bind.struct_script_size_out_of_range",
                    format!(
                        "stored script size {stored_size} does not fit the {}-byte payload",
                        payload_len
                    ),
                ));
            }
            cur.seek(payload_len);
            let children = arena.chain_children(head.children_head)?;
            let object = arena.get_mut(id)?;
            object.class_id = class_id;
            object.data = ObjectData::ScriptStruct(StructData {
                links: head.links,
                super_struct: head.links.super_field,
                children,
            });
            Ok(())
        }
        ExportKind::Const => {
            let _r1 = resolve_raw(ctx, read_compact_index(cur)?);
            let _r2 = resolve_raw(ctx, read_compact_index(cur)?);
            let links = FieldLinks {
                super_field: None,
                next: resolve_raw(ctx, read_compact_index(cur)?),
            };
            let value = read_fstring(cur)?;
            if consumed(cur) != payload_len {
                return Err(Fail::new(
                    "bind.const_trailing_bytes",
                    format!(
                        "{} of {} byte(s) unconsumed",
                        payload_len - consumed(cur),
                        payload_len
                    ),
                ));
            }
            let object = arena.get_mut(id)?;
            object.class_id = class_id;
            object.data = ObjectData::Const { links, value };
            Ok(())
        }
        ExportKind::Enum => {
            let _r1 = resolve_raw(ctx, read_compact_index(cur)?);
            let _r2 = resolve_raw(ctx, read_compact_index(cur)?);
            let links = FieldLinks {
                super_field: None,
                next: resolve_raw(ctx, read_compact_index(cur)?),
            };
            let count = read_compact_index(cur)?;
            if count < 0 {
                return Err(Fail::new(
                    "bind.negative_array_count",
                    format!("enum names {count}"),
                ));
            }
            let mut names = Vec::with_capacity(count as usize);
            for _ in 0..count {
                names.push(ctx.global_name(read_compact_index(cur)?)?);
            }
            if consumed(cur) != payload_len {
                return Err(Fail::new(
                    "bind.enum_trailing_bytes",
                    format!(
                        "{} of {} byte(s) unconsumed",
                        payload_len - consumed(cur),
                        payload_len
                    ),
                ));
            }
            let object = arena.get_mut(id)?;
            object.class_id = class_id;
            object.data = ObjectData::Enum { links, names };
            Ok(())
        }
        ExportKind::TextBuffer => {
            let _pos = cur.i32()?;
            let _top = cur.i32()?;
            let _extra = read_compact_index(cur)?;
            let text = read_fstring(cur)?;
            if consumed(cur) != payload_len {
                return Err(Fail::new(
                    "bind.textbuffer_trailing_bytes",
                    format!(
                        "{} of {} byte(s) unconsumed",
                        payload_len - consumed(cur),
                        payload_len
                    ),
                ));
            }
            let object = arena.get_mut(id)?;
            object.class_id = class_id;
            object.data = ObjectData::TextBuffer { text };
            Ok(())
        }
        ExportKind::Class => load_class(arena, ctx, index, id, class_id, cur, payload_len),
        ExportKind::Raw => unreachable!("raw handled inline"),
    }
}

fn resolve_raw(ctx: &Ctx<'_>, reference: i32) -> Option<ObjectId> {
    ctx.resolve(reference).flatten()
}

/// Reference-count shape of a struct-ish export head. States, functions and
/// script structs serialize one extra leading null reference; classes do not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeadRefs {
    Four,
    Five,
}

struct StructHead {
    links: FieldLinks,
    children_head: Option<ObjectId>,
}

/// Read the common UStruct-ish prefix through the source position/line pair:
/// `[refs…][friendly name][line i32][text-pos i32]`. The friendly name must
/// be non-null; operator functions serialize their symbol rather than the
/// spelled-out object name, so equality with the export name is not required.
fn read_struct_head(
    ctx: &Ctx<'_>,
    cur: &mut ByteCursor<'_>,
    refs: HeadRefs,
    subject: &str,
    expected_friendly: Option<u32>,
) -> Result<StructHead> {
    let count = match refs {
        HeadRefs::Four => 4,
        HeadRefs::Five => 5,
    };
    let mut raw = Vec::with_capacity(count);
    for _ in 0..count {
        raw.push(read_compact_index(cur)?);
    }
    let friendly = ctx.global_name(read_compact_index(cur)?)?;
    if friendly == 0 {
        return Err(Fail::new(
            "bind.friendly_name_none",
            format!("{subject}: serialized friendly name is NAME_None"),
        ));
    }
    let _ = expected_friendly;
    // Field mapping: [...extras][superfield][next][script-text][children].
    let resolve = |v| resolve_raw(ctx, v);
    let (super_field, next, children_head) = match refs {
        HeadRefs::Four => (resolve(raw[0]), resolve(raw[1]), resolve(raw[3])),
        HeadRefs::Five => (resolve(raw[1]), resolve(raw[2]), resolve(raw[4])),
    };
    let _line = cur.i32()?;
    let _text_pos = cur.i32()?;
    Ok(StructHead {
        links: FieldLinks { super_field, next },
        children_head,
    })
}

/// Read the stored script size: a RAW little-endian I32 per
/// `UStruct::Serialize` (`Ar << ScriptSize` where INT is four bytes).
fn read_script_size(cur: &mut ByteCursor<'_>, subject: &str) -> Result<i32> {
    let size = cur
        .i32()
        .map_err(|e| Fail::new("bind.script_size_truncated", format!("{subject}: {e}")))?;
    Ok(size)
}

/// End-anchored function trailer, byte-identical constraints to the Phase-1
/// audit derivation: exactly one of the 7-byte / 9-byte layouts must fit.
struct FunctionTrailer {
    offset: usize,
    native_index: u16,
    precedence: u8,
    function_flags: u32,
}

fn derive_function_trailer(payload: &[u8], code_start: usize) -> Result<FunctionTrailer> {
    const FUNC_NET: u32 = 0x0000_0040;
    const FUNC_NET_RELIABLE: u32 = 0x0000_0080;
    const FUNC_NATIVE: u32 = 0x0000_0400;
    const FLAG_MASK: u32 = 0x0001_FFFF;
    let mut candidates = Vec::new();
    for is_net in [false, true] {
        let trailer_size = if is_net { 9 } else { 7 };
        if trailer_size > payload.len() {
            continue;
        }
        let offset = payload.len() - trailer_size;
        // Prefix drift is tolerated: the code slice is clamped later.
        let _ = code_start;
        let native_index = u16::from_le_bytes([payload[offset], payload[offset + 1]]);
        let precedence = payload[offset + 2];
        let function_flags =
            u32::from_le_bytes(payload[offset + 3..offset + 7].try_into().unwrap());
        if u32::from(native_index) >= hp_format_pkg_max_native_index()
            || function_flags & !FLAG_MASK != 0
        {
            continue;
        }
        if (function_flags & FUNC_NET != 0) != is_net {
            continue;
        }
        if function_flags & FUNC_NET_RELIABLE != 0 && !is_net {
            continue;
        }
        if native_index != 0 && function_flags & FUNC_NATIVE == 0 {
            continue;
        }
        candidates.push(FunctionTrailer {
            offset,
            native_index,
            precedence,
            function_flags,
        });
    }
    if candidates.len() != 1 {
        return Err(Fail::new(
            "bind.function_trailer_ambiguous",
            format!(
                "{} candidate trailer layout(s) (payload {} bytes, code from {})",
                candidates.len(),
                payload.len(),
                code_start
            ),
        ));
    }
    Ok(candidates.pop().unwrap())
}

// Avoid a direct dependency cycle on hp-format consts duplicated above.
fn hp_format_pkg_max_native_index() -> u32 {
    0x1000
}

fn parse_property(
    arena: &ObjectArena,
    ctx: &mut Ctx<'_>,
    cur: &mut ByteCursor<'_>,
    folded_class: &str,
) -> Result<PropertyData> {
    let _r1 = resolve_raw(ctx, read_compact_index(cur)?);
    let super_field = resolve_raw(ctx, read_compact_index(cur)?);
    let next = resolve_raw(ctx, read_compact_index(cur)?);
    let array_dim = cur.i32()?;
    let property_flags = cur.u32()?;
    let category = ctx.global_name(read_compact_index(cur)?)?;
    if property_flags & CPF_NET != 0 {
        let _rep_offset = cur.u16()?;
    }

    let kind = match folded_class {
        "byteproperty" => PropertyKind::Byte {
            enum_ref: resolve_raw(ctx, read_compact_index(cur)?),
        },
        "intproperty" => PropertyKind::Int,
        "boolproperty" => PropertyKind::Bool,
        "floatproperty" => PropertyKind::Float,
        "objectproperty" | "componentproperty" => PropertyKind::Object {
            class_ref: resolve_raw(ctx, read_compact_index(cur)?),
        },
        "classproperty" | "interfaceproperty" => PropertyKind::Class {
            class_ref: resolve_raw(ctx, read_compact_index(cur)?),
            meta_ref: resolve_raw(ctx, read_compact_index(cur)?),
        },
        "nameproperty" => PropertyKind::Name,
        "strproperty" | "stringproperty" => PropertyKind::Str,
        "structproperty" => PropertyKind::Struct {
            struct_ref: resolve_raw(ctx, read_compact_index(cur)?),
        },
        "arrayproperty" => PropertyKind::Array {
            inner: Box::new(template_kind_of(arena, ctx, read_compact_index(cur)?)?),
        },
        "fixedarrayproperty" => {
            let inner = template_kind_of(arena, ctx, read_compact_index(cur)?)?;
            let count = cur.i32()?;
            PropertyKind::FixedArray {
                inner: Box::new(inner),
                count,
            }
        }
        "mapproperty" => {
            let key = template_kind_of(arena, ctx, read_compact_index(cur)?)?;
            let value = template_kind_of(arena, ctx, read_compact_index(cur)?)?;
            PropertyKind::Map {
                key: Box::new(key),
                value: Box::new(value),
            }
        }
        other => {
            return Err(Fail::new(
                "bind.property_kind_unknown",
                format!("unsupported property class {other}"),
            ));
        }
    };

    Ok(PropertyData {
        links: FieldLinks { super_field, next },
        kind,
        array_dim,
        property_flags,
        category,
    })
}

/// Resolve an inner-field reference (array/map/fixed-array inner pointers)
/// to its template kind. Inner fields are ordinary exports whose payloads
/// are independent, so parse them on demand; already-parsed ones reuse the
/// stored kind.
fn template_kind_of(
    arena: &ObjectArena,
    ctx: &mut Ctx<'_>,
    reference: i32,
) -> Result<PropertyKind> {
    let id = resolve_raw(ctx, reference).ok_or_else(|| {
        Fail::new(
            "bind.inner_field_missing",
            format!("inner-field ref {reference} unresolved"),
        )
    })?;
    if let Ok(crate::arena::UObject {
        data: ObjectData::Property(prop),
        ..
    }) = arena.get(id)
    {
        return Ok(prop.kind.clone());
    }
    let index = ctx
        .export_ids
        .iter()
        .position(|e| *e == id)
        .ok_or_else(|| {
            Fail::new(
                "bind.inner_field_missing",
                format!("ref {reference} not an export"),
            )
        })?;
    let folded = match export_kind(arena, ctx, index) {
        ExportKind::Property { folded_class } => folded_class,
        _ => {
            return Err(Fail::new(
                "bind.inner_field_not_property",
                format!("inner-field ref {reference} is not a property export"),
            ));
        }
    };
    let payload = ctx.payload(index)?.to_vec();
    let mut inner_cur = ByteCursor::at(&payload, 0);
    let data = parse_property(arena, ctx, &mut inner_cur, &folded)?;
    Ok(data.kind)
}

fn load_class(
    arena: &mut ObjectArena,
    ctx: &mut Ctx<'_>,
    index: usize,
    id: ObjectId,
    class_id: Option<ObjectId>,
    cur: &mut ByteCursor<'_>,
    payload_len: usize,
) -> Result<()> {
    let expected = Some(ctx.global_name(ctx.archive.exports[index].object_name_index)?);
    let head = read_struct_head(ctx, cur, HeadRefs::Four, "class", expected)?;
    let _stored_size = read_script_size(cur, "class")?;

    // Superclass must already be loaded (fixpoint caller guarantees this for
    // same-package chains; cross-package supers come from earlier loads).
    let super_class = head.links.super_field;
    if let Some(super_id) = super_class {
        let parsed = matches!(
            arena.get(super_id).map(|o| &o.data),
            Ok(ObjectData::Class(_))
        );
        if !parsed {
            return Err(Fail::new(
                "bind.superclass_unparsed",
                format!("super {super_id:?}"),
            ));
        }
    }

    // `UClass : UState : UStruct`: bytecode runs from here to the UState
    // tail ([probe u64][ignore u64][labeloff u16][flags u32]) followed by
    // the class block ([class-flags u32][guid 16][dependencies]
    // [package-imports][within][config-name][tagged defaults]). The stored
    // script size overcounts on native classes, so the code end is located
    // by scanning for suffix layouts that consume the payload exactly.
    // Several offsets can be structurally valid; pick deterministically:
    // highest ratio of default-tag names resolving against the inheritance
    // chain, then exact stored-size match, then strictly-valid dependencies,
    // then lowest offset.
    let code_start = cur.position();
    let data = cur.data();
    let mut chain = vec![id];
    if let Some(sup) = super_class {
        chain.extend(arena.class_chain(sup)?);
    }
    let stored_code_len = usize::try_from(_stored_size.max(0)).unwrap_or(0);

    let mut best: Option<(bool, u32, bool, usize)> = None;
    for code_end in code_start..=payload_len {
        let Some(probe) = probe_class_tail(ctx, data, code_end, payload_len) else {
            continue;
        };
        let resolved = probe
            .tag_locals
            .iter()
            .filter(|local| {
                ctx.global_name(**local).ok().is_some_and(|global| {
                    find_template_in_chain(arena, &chain, global)
                        .ok()
                        .flatten()
                        .is_some()
                        || find_own_template(arena, id, global).is_some()
                })
            })
            .count() as u32;
        let ss_match = code_end - code_start == stored_code_len;
        // A stored-size match is authoritative when one exists (the HP2
        // compiler overcounts only on some native classes); otherwise fall
        // back to semantic tag resolution.
        if ss_match && probe.strict_deps {
            best = Some((true, resolved, true, code_end));
            break;
        }
        let key = (ss_match, resolved, probe.strict_deps);
        if best.map(|b| key > (b.0, b.1, b.2)).unwrap_or(true) {
            best = Some((ss_match, resolved, probe.strict_deps, code_end));
        }
    }
    let Some((_, _, _, code_end)) = best else {
        return Err(Fail::new(
            "bind.class_tail_unresolved",
            format!(
                "no code-end candidate yields a consistent class tail ({payload_len}-byte payload)"
            ),
        ));
    };
    cur.seek(code_end);

    let _probe_mask = u64::from_le_bytes(cur.take(8)?.try_into().unwrap());
    let _ignore_mask = u64::from_le_bytes(cur.take(8)?.try_into().unwrap());
    let _label_table_offset = cur.u16()?;
    let _state_flags = cur.u32()?;
    let class_flags = cur.u32()?;
    let _class_guid = cur.take(16)?;
    let dependency_count = read_compact_index(cur)?;
    if dependency_count < 0 {
        return Err(Fail::new("bind.negative_array_count", "dependencies"));
    }
    for _ in 0..dependency_count {
        let _dep_class = read_compact_index(cur)?;
        let _deep = cur.u32()?;
        let _crc = cur.u32()?;
    }
    let package_import_count = read_compact_index(cur)?;
    if package_import_count < 0 {
        return Err(Fail::new("bind.negative_array_count", "package imports"));
    }
    for _ in 0..package_import_count {
        let _import = read_compact_index(cur)?;
    }
    let _within = resolve_raw(ctx, read_compact_index(cur)?);
    let config_name = ctx.global_name(read_compact_index(cur)?)?;
    // Tagged defaults: decode against templates from the full chain.
    let defaults = parse_tagged_defaults(arena, ctx, cur, &chain)?;

    let children = arena.chain_children(head.children_head)?;

    if cur.position() != payload_len {
        return Err(Fail::new(
            "bind.class_trailing_bytes",
            format!(
                "{} of {} byte(s) unconsumed",
                payload_len - cur.position(),
                payload_len
            ),
        ));
    }

    // Synthesize the default object (`Default__<Class>` instance).
    let default_object = {
        let class_name = arena.display_name(id)?;
        let defaults_name = arena.names.intern(&format!("Default__{class_name}"));
        arena.alloc(crate::arena::UObject {
            class_id: Some(id),
            name_index: defaults_name,
            outer: Some(id),
            flags: 0,
            data: ObjectData::Properties(defaults.clone()),
        })
    };

    let object = arena.get_mut(id)?;
    object.class_id = class_id;
    object.data = ObjectData::Class(ClassData {
        links: head.links,
        super_class,
        children,
        class_flags,
        within: None,
        config_name,
        default_object: Some(default_object),
    });
    Ok(())
}

/// A structurally consistent class-tail candidate: dependency strictness and
/// the local name indices of the decoded default-property tags.
struct ClassTailProbe {
    strict_deps: bool,
    tag_locals: Vec<i32>,
}

/// Try to decode a full class tail starting at `code_end` (the UState fields
/// onward). Returns `Some` when the layout consumes the payload exactly.
fn probe_class_tail(
    ctx: &Ctx<'_>,
    data: &[u8],
    code_end: usize,
    payload_len: usize,
) -> Option<ClassTailProbe> {
    const STATE_TAIL: usize = 8 + 8 + 2 + 4;
    let mut cur = ByteCursor::at(data, code_end);
    if code_end + STATE_TAIL > data.len() {
        return None;
    }
    cur.seek(code_end + STATE_TAIL);
    let _class_flags = cur.u32().ok()?;
    let _guid = cur.take(16).ok()?;
    let dependency_count = read_compact_index(&mut cur).ok()?;
    if !(0..=0x1000).contains(&dependency_count) {
        return None;
    }
    let mut strict_deps = true;
    for _ in 0..dependency_count {
        let dep_ref = read_compact_index(&mut cur).ok()?;
        let deep = cur.u32().ok()?;
        cur.u32().ok()?;
        if deep > 1 || !reference_resolves(ctx, dep_ref) {
            strict_deps = false;
        }
    }
    let package_import_count = read_compact_index(&mut cur).ok()?;
    if !(0..=0x1000).contains(&package_import_count) {
        return None;
    }
    for _ in 0..package_import_count {
        read_compact_index(&mut cur).ok()?;
    }
    let _within = read_compact_index(&mut cur).ok()?;
    let config_name = read_compact_index(&mut cur).ok()?;
    if config_name < 0 || config_name as usize >= ctx.name_map.len() {
        return None;
    }
    let mut tag_locals = Vec::new();
    // Tagged defaults.
    loop {
        let tag_name = read_compact_index(&mut cur).ok()?;
        if tag_name == 0 {
            break;
        }
        if tag_name < 0 || tag_name as usize >= ctx.local_names.len() {
            return None;
        }
        if ctx.local_names[tag_name as usize].text == "None" {
            break;
        }
        tag_locals.push(tag_name);
        let info = cur.u8().ok()?;
        let kind = info & 0x0F;
        if kind == hp_format::package79::PROPERTY_TYPE_STRUCT {
            read_compact_index(&mut cur).ok()?;
        }
        // Field order per FPropertyTag::operator<<: size word, then the
        // bespoke array index, then the payload.
        let size = match info & 0x70 {
            0x00 => 1usize,
            0x10 => 2,
            0x20 => 4,
            0x30 => 12,
            0x40 => 16,
            0x50 => cur.u8().ok()? as usize,
            0x60 => cur.u16().ok()? as usize,
            _ => {
                let v = cur.i32().ok()?;
                usize::try_from(v).ok()?
            }
        };
        if info & 0x80 != 0 && kind != hp_format::package79::PROPERTY_TYPE_BOOL {
            let b0 = cur.u8().ok()?;
            if b0 & 0xC0 == 0x80 {
                cur.u8().ok()?;
            } else if b0 & 0xC0 == 0xC0 {
                cur.u8().ok()?;
                cur.u8().ok()?;
                cur.u8().ok()?;
            }
        }
        cur.take(size).ok()?;
    }
    if cur.position() != payload_len {
        return None;
    }
    Some(ClassTailProbe {
        strict_deps,
        tag_locals,
    })
}

/// True when a raw object reference points at a known export or a resolved
/// import of this package context.
fn reference_resolves(ctx: &Ctx<'_>, reference: i32) -> bool {
    match reference.cmp(&0) {
        std::cmp::Ordering::Equal => false,
        std::cmp::Ordering::Greater => (reference as usize) <= ctx.export_ids.len(),
        std::cmp::Ordering::Less => ctx
            .import_ids
            .get((-reference - 1) as usize)
            .copied()
            .flatten()
            .is_some(),
    }
}

/// Tagged-property reader for script payloads. Identical grammar to the
/// package-level reader except the array-index field uses the engine's
/// bespoke 1/2/4-byte scheme (`FPropertyTag::operator<<`), which is NOT a
/// standard compact index.
fn read_script_tags<'a>(
    cur: &mut ByteCursor<'a>,
    names: &[hp_format::package79::NameEntry],
) -> Result<Vec<PropertyTag>> {
    use hp_format::package79::{PROPERTY_TYPE_BOOL, PROPERTY_TYPE_STRUCT};
    let mut tags = Vec::new();
    loop {
        let name_index = read_compact_index(cur)?;
        if name_index < 0 || name_index as usize >= names.len() {
            return Err(Fail::new(
                "bind.tag_name_index_range",
                format!("tag name {name_index} / {}", names.len()),
            ));
        }
        if names[name_index as usize].text == "None" {
            return Ok(tags);
        }
        let info = cur.u8()?;
        let kind = info & 0x0F;
        let size_code = info & 0x70;
        let array_flag = info & 0x80 != 0;
        let struct_name_index = if kind == PROPERTY_TYPE_STRUCT {
            Some(read_compact_index(cur)?)
        } else {
            None
        };
        let size = match size_code {
            0x00 => 1usize,
            0x10 => 2,
            0x20 => 4,
            0x30 => 12,
            0x40 => 16,
            0x50 => cur.u8()? as usize,
            0x60 => cur.u16()? as usize,
            _ => {
                let v = cur.i32()?;
                if v < 0 {
                    return Err(Fail::new("bind.negative_tag_size", format!("{v}")));
                }
                v as usize
            }
        };
        let array_index = if array_flag && kind != PROPERTY_TYPE_BOOL {
            let b0 = cur.u8()?;
            Some(if b0 & 0x80 == 0 {
                b0 as i32
            } else if b0 & 0xC0 == 0x80 {
                let c = cur.u8()?;
                (((b0 & 0x7F) as i32) << 8) | c as i32
            } else {
                let c = cur.u8()?;
                let d = cur.u8()?;
                let e = cur.u8()?;
                (((b0 & 0x3F) as i32) << 24) | ((c as i32) << 16) | ((d as i32) << 8) | e as i32
            })
        } else {
            None
        };
        let payload = cur.take(size)?.to_vec();
        tags.push(PropertyTag {
            name_index,
            kind,
            size_code,
            array_flag,
            struct_name_index,
            array_index,
            payload,
        });
    }
}

fn parse_tagged_defaults(
    arena: &ObjectArena,
    ctx: &Ctx<'_>,
    cur: &mut ByteCursor<'_>,
    chain: &[ObjectId],
) -> Result<PropStore> {
    let tags: Vec<PropertyTag> = read_script_tags(cur, ctx.local_names)?;
    let mut store = PropStore::new();
    // Group fixed-array fragments by name before storing.
    let mut fixed_arrays: std::collections::HashMap<
        u32,
        std::collections::BTreeMap<i32, PropValue>,
    > = std::collections::HashMap::new();

    for tag in tags {
        let name_global = ctx.global_name(tag.name_index)?;
        if tag.kind == TAG_BOOL {
            store.set(name_global, PropValue::Bool(tag.array_flag));
            continue;
        }
        let template = find_template_in_chain(arena, chain, name_global)?
            .or_else(|| find_own_template(arena, chain[0], name_global));
        let Some(template) = template else {
            return Err(Fail::new(
                "bind.default_template_missing",
                format!(
                    "tag name index {name_global} has no matching property template on class chain"
                ),
            ));
        };
        let mut payload_cur = ByteCursor::at(&tag.payload, 0);
        let decoded = decode_value(&mut payload_cur, &template.kind, &|struct_ref| {
            flatten_struct_fields(arena, struct_ref)
        });
        let value = match decoded {
            Ok(v) if payload_cur.position() == tag.payload.len() => v,
            Ok(_) | Err(_) => {
                // Fork payloads occasionally disagree with template-derived
                // widths; keep the raw bytes so nothing silently vanishes.
                eprintln!(
                    "hp-uobject: note [bind.default_tag_raw] tag {} kept raw ({} byte(s))",
                    name_global,
                    tag.payload.len()
                );
                continue;
            }
        };
        match (&template.kind, tag.array_index) {
            (PropertyKind::FixedArray { .. }, Some(idx)) => {
                fixed_arrays
                    .entry(name_global)
                    .or_default()
                    .insert(idx, value);
            }
            (PropertyKind::FixedArray { .. }, None) => match value {
                PropValue::FixedArray(items) => {
                    store.set(name_global, PropValue::FixedArray(items));
                }
                other => store.set(name_global, other),
            },
            _ => store.set(name_global, value),
        }
    }

    for (name, parts) in fixed_arrays {
        let count = parts.keys().max().copied().unwrap_or(-1) as usize + 1;
        let mut items = Vec::with_capacity(count);
        for i in 0..count {
            items.push(
                parts
                    .get(&(i as i32))
                    .cloned()
                    .unwrap_or(PropValue::Byte(0)),
            );
        }
        store.set(name, PropValue::FixedArray(items));
    }
    Ok(store)
}

/// Find a property template by name across the class inheritance chain.
pub fn find_template(
    arena: &ObjectArena,
    class_id: ObjectId,
    name_index: u32,
) -> Result<Option<PropertyData>> {
    for cid in arena.class_chain(class_id)? {
        let children = match &arena.get(cid)?.data {
            ObjectData::Class(data) => &data.children,
            ObjectData::ScriptStruct(data) => &data.children,
            _ => continue,
        };
        for child in children {
            if let Ok(ObjectData::Property(prop)) = arena.get(*child).map(|o| &o.data)
                && arena.get(*child)?.name_index == name_index
            {
                return Ok(Some((**prop).clone()));
            }
        }
    }
    Ok(None)
}

/// [`find_template`] over an explicit inheritance chain (self first). Used
/// while a class is still being parsed, when its own `ClassData` is not yet
/// committed and `arena.class_chain` cannot walk past it.
fn find_template_in_chain(
    arena: &ObjectArena,
    chain: &[ObjectId],
    name_index: u32,
) -> Result<Option<PropertyData>> {
    for cid in chain {
        let children = match &arena.get(*cid)?.data {
            ObjectData::Class(data) => &data.children,
            ObjectData::ScriptStruct(data) => &data.children,
            _ => continue,
        };
        for child in children {
            if arena.get(*child)?.name_index != name_index {
                continue;
            }
            if let Ok(ObjectData::Property(prop)) = arena.get(*child).map(|o| &o.data) {
                return Ok(Some((**prop).clone()));
            }
        }
    }
    Ok(None)
}

/// Flatten a struct definition into `(struct name, [(field name, kind)])` in
/// serialization order (super-struct fields first).
pub fn flatten_struct_fields(
    arena: &ObjectArena,
    struct_id: ObjectId,
) -> Result<crate::value::StructFieldList> {
    match &arena.get(struct_id)?.data {
        ObjectData::ScriptStruct(data) => {
            let struct_name = arena.get(struct_id)?.name_index;
            let mut fields = Vec::new();
            if let Some(super_struct) = data.super_struct {
                fields.extend(
                    flatten_struct_fields(arena, super_struct)?
                        .map(|(_, f)| f)
                        .unwrap_or_default(),
                );
            }
            for child in &data.children {
                if let Ok(ObjectData::Property(prop)) = arena.get(*child).map(|o| &o.data) {
                    fields.push((arena.get(*child)?.name_index, prop.kind.clone()));
                }
            }
            Ok(Some((struct_name, fields)))
        }
        _ => Ok(None),
    }
}

/// Fallback template lookup among a class's own direct children — the class's
/// `ClassData` is not committed until its defaults finish decoding, so
/// [`find_template`] cannot see them yet.
fn find_own_template(
    arena: &ObjectArena,
    class_id: ObjectId,
    name_index: u32,
) -> Option<PropertyData> {
    arena.children_of(class_id).find_map(|(child_id, object)| {
        let _ = child_id;
        if object.name_index != name_index {
            return None;
        }
        match &object.data {
            ObjectData::Property(prop) => Some((**prop).clone()),
            _ => None,
        }
    })
}
