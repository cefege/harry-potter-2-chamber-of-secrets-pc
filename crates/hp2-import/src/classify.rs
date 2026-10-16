//! Replaces `HP2ImportInternal.h`'s `Debris`/`Prohibited`: path
//! classification shared by every collection step, mirroring the oracle's
//! `is_debris`/`is_prohibited_path`
//! (`Build/prepare_retail_data.py:301-321`).

use std::path::Path;

use crate::lower::lower;

const DEBRIS_NAMES: [&str; 6] = [
    ".ds_store",
    "desktop.ini",
    "mssccprj.scc",
    "thumbs.db",
    "vssver.scc",
    "ws_ftp.log",
];

const PROHIBITED_EXTENSIONS: [&str; 9] = [
    ".bat", ".cmd", ".com", ".dll", ".drv", ".exe", ".lib", ".ocx", ".sys",
];

/// Matches `fs::path::extension()`'s string (including the leading `.`,
/// or empty when there is none), lowercased via [`lower`]. Rust's
/// `Path::extension()` omits the dot and uses the same "no dot" special
/// case for a name with no extension or a dotfile like `.gitignore`.
fn lower_extension(path: &Path) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => lower(&format!(".{ext}")),
        None => String::new(),
    }
}

fn lower_file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(lower)
        .unwrap_or_default()
}

/// Replaces `Debris(const fs::path&)`.
pub fn is_debris(path: &Path) -> bool {
    let name = lower_file_name(path);
    let ext = lower_extension(path);
    DEBRIS_NAMES.contains(&name.as_str())
        || ext == ".scc"
        || name.starts_with("unins")
        || name.ends_with(".bak")
        || name.ends_with(".old")
        || name.ends_with(".orig")
        || name.ends_with(".tmp")
        || name.ends_with('~')
}

/// Replaces `Prohibited(const std::string&)`. Takes a path-shaped string
/// (a destination path like `"System/foo.exe"`, not necessarily a real
/// filesystem path), matching the C++ overload that operates on
/// `std::string` rather than `fs::path`.
pub fn is_prohibited(path: &str) -> bool {
    let lowered = lower(path);
    let as_path = Path::new(path);
    let ext = lower_extension(as_path);
    if PROHIBITED_EXTENSIONS.contains(&ext.as_str()) || is_debris(as_path) {
        return true;
    }
    lowered
        .split('/')
        .any(|part| matches!(part, ".git" | ".svn" | "cvs" | "__pycache__"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debris_matches_known_names_case_insensitively() {
        assert!(is_debris(Path::new("Thumbs.db")));
        assert!(is_debris(Path::new(".DS_Store")));
        assert!(is_debris(Path::new("Desktop.ini")));
        assert!(!is_debris(Path::new("Thumbs.dbx")));
    }

    #[test]
    fn debris_matches_scc_extension_and_suffix_patterns() {
        assert!(is_debris(Path::new("project.SCC")));
        assert!(is_debris(Path::new("uninstall.exe")));
        assert!(is_debris(Path::new("backup.BAK")));
        assert!(is_debris(Path::new("old.OLD")));
        assert!(is_debris(Path::new("thing.orig")));
        assert!(is_debris(Path::new("scratch.tmp")));
        assert!(is_debris(Path::new("file~")));
        assert!(!is_debris(Path::new("normal.txt")));
    }

    #[test]
    fn debris_handles_dotfiles_and_extensionless_names_like_cpp_extension() {
        // ".gitignore" has no extension under both fs::path::extension()
        // and Rust's Path::extension() (leading-dot special case) -- must
        // not be treated as having extension ".gitignore".
        assert!(!is_debris(Path::new(".gitignore")));
        assert!(!is_debris(Path::new("README")));
    }

    #[test]
    fn prohibited_matches_dangerous_extensions() {
        for ext in [
            "bat", "cmd", "com", "dll", "drv", "exe", "lib", "ocx", "sys",
        ] {
            assert!(is_prohibited(&format!("System/payload.{ext}")));
            assert!(is_prohibited(&format!(
                "System/PAYLOAD.{}",
                ext.to_uppercase()
            )));
        }
        assert!(!is_prohibited("System/Default.ini"));
    }

    #[test]
    fn prohibited_matches_vcs_and_cache_path_components() {
        assert!(is_prohibited("Textures/.git/config"));
        assert!(is_prohibited("Textures/.SVN/entries"));
        assert!(is_prohibited("a/cvs/b"));
        assert!(is_prohibited("a/__pycache__/b.pyc"));
        assert!(!is_prohibited("Textures/gitignore/foo.utx"));
    }

    #[test]
    fn prohibited_also_catches_debris_via_the_string_overload() {
        assert!(is_prohibited("System/Thumbs.db"));
    }
}
