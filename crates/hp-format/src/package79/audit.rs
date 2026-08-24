//! Audit-parity support: object-path resolution, UFunction terminal-field
//! derivation, and the `--data-root` audit document assembled by
//! `Build/package79_reference.py::generate_audit`.

use std::collections::{BTreeMap, HashSet};

use super::archive::PackageArchive;
use super::error::{PackageError, PackageResult, layout};
use super::json::Json;
use super::tables::{ExportEntry, ImportEntry, NameEntry};

/// Upper bound on derived native ordinals.
pub const MAX_NATIVE_INDEX: u32 = 0x1000;
pub const FUNCTION_FLAG_NET: u32 = 0x0000_0040;
pub const FUNCTION_FLAG_NET_RELIABLE: u32 = 0x0000_0080;
pub const FUNCTION_FLAG_NATIVE: u32 = 0x0000_0400;
pub const FUNCTION_FLAG_MASK: u32 = 0x0001_FFFF;

/// Resolves import/export references to dotted object paths exactly like
/// `Build/package79_reference.py::ObjectPathResolver`.
///
/// Archives from [`read_package`](super::read_package) are acyclic; hand-built
/// tables get a cycle reported as [`PackageError::CycleInOuterRefs`] instead
/// of unbounded recursion.
pub struct ObjectPaths<'a> {
    names: &'a [NameEntry],
    imports: &'a [ImportEntry],
    exports: &'a [ExportEntry],
    package_name: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Node {
    Import(usize),
    Export(usize),
}

impl<'a> ObjectPaths<'a> {
    pub fn new(
        package_name: impl Into<String>,
        names: &'a [NameEntry],
        imports: &'a [ImportEntry],
        exports: &'a [ExportEntry],
    ) -> Self {
        Self {
            names,
            imports,
            exports,
            package_name: package_name.into(),
        }
    }

    /// Path for an object reference (`0` → None, positive → export-1,
    /// negative → import+1).
    pub fn ref_path(&self, reference: i32) -> PackageResult<Option<String>> {
        match reference {
            0 => Ok(None),
            r if r > 0 => self
                .node_path(Node::Export(r as usize - 1), &mut HashSet::new())
                .map(Some),
            r => self
                .node_path(Node::Import((-r) as usize - 1), &mut HashSet::new())
                .map(Some),
        }
    }

    /// Full dotted path of import `index`.
    pub fn import_path(&self, index: usize) -> PackageResult<String> {
        self.node_path(Node::Import(index), &mut HashSet::new())
    }

    /// Full dotted path of export `index` (rooted at the package name).
    pub fn export_path(&self, index: usize) -> PackageResult<String> {
        self.node_path(Node::Export(index), &mut HashSet::new())
    }

    fn node_path(&self, node: Node, seen: &mut HashSet<Node>) -> PackageResult<String> {
        if !seen.insert(node) {
            return Err(PackageError::CycleInOuterRefs);
        }
        let (outer_ref, name_index) = match node {
            Node::Import(index) => {
                let entry = self.imports.get(index).ok_or_else(|| {
                    layout(format!("import index {index} is outside the import table"))
                })?;
                (entry.outer_ref, entry.object_name_index)
            }
            Node::Export(index) => {
                let entry = self.exports.get(index).ok_or_else(|| {
                    layout(format!("export index {index} is outside the export table"))
                })?;
                (entry.outer_ref, entry.object_name_index)
            }
        };
        let name = self
            .names
            .get(name_index as usize)
            .ok_or(PackageError::BadNameIndex {
                index: name_index,
                count: self.names.len(),
            })?;
        let outer = match self.ref_path(outer_ref)? {
            Some(path) => Some(path),
            None if matches!(node, Node::Export(_)) => Some(self.package_name.clone()),
            None => None,
        };
        Ok(match outer {
            Some(prefix) => format!("{prefix}.{}", name.text),
            None => name.text.clone(),
        })
    }
}

/// Terminal fields derived from a Function export's serial span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionTerminal {
    pub native_index: u16,
    pub operator_precedence: u8,
    pub function_flags: u32,
}

/// Derive a Function export's terminal fields (`iNative`, `OperPrecedence`,
/// `FunctionFlags`, plus `RepOffset` only when FUNC_Net) from the exact end
/// boundary of its serial span.
///
/// `data` is the whole original file; trailer offsets are absolute.
pub fn derive_function_terminal(
    data: &[u8],
    entry: &ExportEntry,
) -> PackageResult<FunctionTerminal> {
    let Some(offset) = entry.serial_offset else {
        return Err(layout("Function export has no serial payload"));
    };
    if entry.serial_size < 7 {
        return Err(layout(
            "Function export is too small for its terminal fields",
        ));
    }
    let payload_offset = offset as usize;
    let payload_end = payload_offset + entry.serial_size as usize;
    if payload_end > data.len() {
        return Err(layout("Function export serial span exceeds the input"));
    }

    let mut candidates = Vec::new();
    for is_net in [false, true] {
        let trailer_size = if is_net { 9 } else { 7 };
        let trailer_offset = match payload_end.checked_sub(trailer_size) {
            Some(value) if value >= payload_offset => value,
            _ => continue,
        };
        let native_index = u16::from_le_bytes([data[trailer_offset], data[trailer_offset + 1]]);
        let operator_precedence = data[trailer_offset + 2];
        let function_flags = u32::from_le_bytes([
            data[trailer_offset + 3],
            data[trailer_offset + 4],
            data[trailer_offset + 5],
            data[trailer_offset + 6],
        ]);
        if u32::from(native_index) >= MAX_NATIVE_INDEX || function_flags & !FUNCTION_FLAG_MASK != 0
        {
            continue;
        }
        if (function_flags & FUNCTION_FLAG_NET != 0) != is_net {
            continue;
        }
        if function_flags & FUNCTION_FLAG_NET_RELIABLE != 0 && !is_net {
            continue;
        }
        if native_index != 0 && function_flags & FUNCTION_FLAG_NATIVE == 0 {
            continue;
        }
        candidates.push(FunctionTerminal {
            native_index,
            operator_precedence,
            function_flags,
        });
    }

    if candidates.len() != 1 {
        return Err(PackageError::AmbiguousFunctionTerminal {
            candidates: candidates.len(),
        });
    }
    Ok(candidates.remove(0))
}

/// Class-path string used for an export's `class_ref` (audit convention:
/// zero maps to `"Core.Class"`).
fn class_path(paths: &ObjectPaths<'_>, entry: &ExportEntry) -> PackageResult<Option<String>> {
    if entry.class_ref == 0 {
        Ok(Some("Core.Class".to_string()))
    } else {
        paths.ref_path(entry.class_ref)
    }
}

/// Per-package projection emitted into `--data-root` audits
/// (`PackageReader.audit_entry`).
///
/// `source` must be the same byte slice the archive was parsed from; function
/// terminals read trailer fields out of it by absolute offset.
pub fn audit_entry(
    relative_path: &str,
    source: &[u8],
    archive: &PackageArchive,
) -> PackageResult<Json> {
    let stem = std::path::Path::new(relative_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    let paths = ObjectPaths::new(stem, &archive.names, &archive.imports, &archive.exports);

    let mut exports_summary = Vec::with_capacity(archive.exports.len());
    let mut native_functions = Vec::new();
    for (index, entry) in archive.exports.iter().enumerate() {
        let object_path = paths.export_path(index)?;
        let resolved_class_path = class_path(&paths, entry)?;
        exports_summary.push(Json::obj(vec![
            (
                "class_path",
                resolved_class_path
                    .clone()
                    .map(Json::Str)
                    .unwrap_or(Json::Null),
            ),
            ("object_path", Json::str(object_path.clone())),
        ]));
        let is_function = resolved_class_path
            .is_some_and(|path| path == "Core.Function" || path.ends_with(".Function"));
        if is_function {
            let terminal = derive_function_terminal(source, entry)?;
            if terminal.function_flags & FUNCTION_FLAG_NATIVE != 0 {
                native_functions.push(Json::obj(vec![
                    ("native_index", Json::Int(i64::from(terminal.native_index))),
                    ("object_path", Json::str(object_path)),
                ]));
            }
        }
    }

    let summary = &archive.summary;
    Ok(Json::obj(vec![
        ("export_count", Json::Int(i64::from(summary.export_count))),
        ("exports", Json::Arr(exports_summary)),
        ("import_count", Json::Int(i64::from(summary.import_count))),
        ("licensee", Json::Int(i64::from(summary.licensee))),
        ("name_count", Json::Int(i64::from(summary.name_count))),
        ("native_functions", Json::Arr(native_functions)),
        ("package_flags", Json::Int(i64::from(summary.package_flags))),
        ("path", Json::str(relative_path)),
        ("version", Json::Int(i64::from(summary.version))),
    ]))
}

/// One walked package: its repo-relative POSIX path, original bytes, and
/// parsed archive.
pub struct AuditedPackage {
    pub relative_path: String,
    pub source: Vec<u8>,
    pub archive: PackageArchive,
}

impl AuditedPackage {
    /// Format profile key used by the audit census.
    fn profile_key(&self) -> String {
        format!(
            "v{}/licensee{}/flags{}",
            self.archive.summary.version,
            self.archive.summary.licensee,
            self.archive.summary.package_flags
        )
    }
}

/// Assemble the top-level audit document (`generate_audit` minus filesystem
/// traversal). `packages` MUST be sorted by relative path, as must
/// `non_package_files`; both orders are produced by the reference walker and
/// preserved here.
pub fn assemble_audit_document(
    data_root_display: &str,
    mut packages: Vec<AuditedPackage>,
    non_package_files: Vec<String>,
) -> PackageResult<Json> {
    let mut profiles: BTreeMap<String, i64> = BTreeMap::new();
    let mut package_entries = Vec::with_capacity(packages.len());
    for walked in &mut packages {
        package_entries.push(
            audit_entry(&walked.relative_path, &walked.source, &walked.archive).map_err(
                |error| {
                    layout(format!(
                        "{}: deriving audit entry: {error}",
                        walked.relative_path
                    ))
                },
            )?,
        );
        *profiles.entry(walked.profile_key()).or_insert(0) += 1;
        // Source bytes are no longer needed once the entry is rendered.
        walked.source.clear();
    }

    let file_count = packages.len() + non_package_files.len();
    Ok(Json::obj(vec![
        (
            "accepted_versions",
            Json::obj(vec![
                (
                    "max",
                    Json::Int(i64::from(super::cursor::PACKAGE_MAX_VERSION)),
                ),
                (
                    "min",
                    Json::Int(i64::from(super::cursor::PACKAGE_MIN_VERSION)),
                ),
            ]),
        ),
        (
            "census",
            Json::obj(vec![
                ("file_count", Json::Int(file_count as i64)),
                (
                    "format_profiles",
                    Json::Obj(
                        profiles
                            .into_iter()
                            .map(|(k, v)| (k, Json::Int(v)))
                            .collect(),
                    ),
                ),
                (
                    "non_package_count",
                    Json::Int(non_package_files.len() as i64),
                ),
                ("package_count", Json::Int(packages.len() as i64)),
            ]),
        ),
        ("data_root", Json::str(data_root_display)),
        ("format", Json::str("hp1-ue1-package-audit")),
        (
            "non_package_files",
            Json::Arr(non_package_files.into_iter().map(Json::Str).collect()),
        ),
        ("packages", Json::Arr(package_entries)),
        ("schema_version", Json::Int(1)),
    ]))
}
