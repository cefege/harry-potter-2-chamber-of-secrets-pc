//! Audit every UE1 package under a data root, emitting the same canonical
//! JSON document as `python3 Build/package79_reference.py --data-root ROOT`.
//!
//! Usage: `cargo run -q -p hp-format --example p79audit -- --data-root <DIR>
//! [--repo-root <DIR>]`
//!
//! The JSON goes to stdout; `--repo-root` (default: the current directory) is
//! used only to render the `data_root` field repo-relative when possible.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use hp_format::package79::{AuditedPackage, PackageArchive, assemble_audit_document, read_package};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("p79audit: error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut data_root: Option<PathBuf> = None;
    let mut repo_root: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--data-root" => {
                data_root = Some(PathBuf::from(
                    args.next().ok_or("--data-root needs a value")?,
                ))
            }
            "--repo-root" => {
                repo_root = Some(PathBuf::from(
                    args.next().ok_or("--repo-root needs a value")?,
                ))
            }
            other => return Err(format!("unrecognized argument: {other}")),
        }
    }
    let data_root = data_root.ok_or("missing --data-root")?;
    let repo_root =
        repo_root.unwrap_or_else(|| std::env::current_dir().expect("current directory"));

    let root = data_root
        .canonicalize()
        .map_err(|error| format!("{}: cannot resolve data root: {error}", data_root.display()))?;
    if !root.is_dir() {
        return Err(format!(
            "{}: data root is not a directory",
            data_root.display()
        ));
    }

    let mut files: Vec<String> = Vec::new();
    collect_files(&root, &root, &mut files)?;
    files.sort();

    let mut packages: Vec<AuditedPackage> = Vec::new();
    let mut non_package_files: Vec<String> = Vec::new();
    for relative_path in &files {
        let source = std::fs::read(root.join(relative_path))
            .map_err(|error| format!("{relative_path}: cannot read file: {error}"))?;
        if source.len() < 4
            || u32::from_le_bytes([source[0], source[1], source[2], source[3]])
                != hp_format::package79::PACKAGE_TAG
        {
            non_package_files.push(relative_path.clone());
            continue;
        }
        let archive: PackageArchive =
            read_package(&source).map_err(|error| format!("{relative_path}: {error}"))?;
        packages.push(AuditedPackage {
            relative_path: relative_path.clone(),
            source,
            archive,
        });
    }

    let data_root_display = root
        .strip_prefix(&repo_root)
        .ok()
        .and_then(|relative| relative.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| root.to_string_lossy().into_owned());

    let document = assemble_audit_document(&data_root_display, packages, non_package_files)
        .map_err(|error| error.to_string())?;
    print!("{}", document.to_canonical());
    Ok(())
}

/// Depth-first collection of every regular file's repo-relative POSIX path.
///
/// Mirrors the reference walker's exclusions: top-level `*.json` provenance
/// manifests directly inside the root and any `crash-report.json` are build
/// artifacts/debris, not game data.
fn collect_files(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), String> {
    let mut entries: BTreeMap<String, PathBuf> = BTreeMap::new();
    let read_dir = dir
        .read_dir()
        .map_err(|error| format!("{}: cannot list directory: {error}", dir.display()))?;
    for entry in read_dir {
        let entry = entry
            .map_err(|error| format!("{}: cannot read directory entry: {error}", dir.display()))?;
        entries.insert(
            entry.file_name().to_string_lossy().into_owned(),
            entry.path(),
        );
    }

    for (name, path) in entries {
        let file_type = path
            .symlink_metadata()
            .map_err(|error| format!("{}: cannot stat: {error}", path.display()))?
            .file_type();
        if file_type.is_dir() {
            collect_files(root, &path, out)?;
            continue;
        }
        let is_top_level_json =
            Path::new(&name).extension() == Some(std::ffi::OsStr::new("json")) && dir == root;
        if is_top_level_json || name == "crash-report.json" {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .expect("walked paths are under root")
            .components()
            .collect::<PathBuf>()
            .to_string_lossy()
            .replace('\\', "/");
        out.push(relative);
    }
    Ok(())
}
