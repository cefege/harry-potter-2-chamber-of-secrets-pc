//! First run from the macOS app: the player has not imported their copy of the game yet.
//!
//! Instead of an error, the game welcomes them, lets them pick their installed game folder (or an
//! archive of it) with the standard Finder chooser, imports it with the bundled `hp2_import`
//! (the same importer and options the old launcher used: `retail-only` into the folder the game
//! reads), explains a wrong choice in plain words, and then continues straight into the game.
//! Command-line runs and tests are unaffected: this only runs inside `*.app/Contents/MacOS`.
use std::{path::{Path, PathBuf}, process::Command};

const TITLE: &str = "Harry Potter and the Chamber of Secrets";

/// What the player picked.
enum Source {
    Folder(PathBuf),
    Archive(PathBuf),
}

/// The executable's folder when running from an app bundle.
pub(crate) fn bundle_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?.to_path_buf();
    dir.to_string_lossy().contains(".app/Contents/MacOS").then_some(dir)
}

/// Where the game looks for imported data (`hp2-bootstrap-paths`).
fn data_dir() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("HOME")?).join("Library/Application Support/Harry Potter 2/Data/Retail"))
}

/// Runs an AppleScript and returns its trimmed output, or `None` if it failed or was cancelled.
fn osascript(script: &str) -> Option<String> {
    let output = Command::new("/usr/bin/osascript").arg("-e").arg(script).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn notify(text: &str) {
    let _ = osascript(&format!("display notification {} with title {}", quoted(text), quoted(TITLE)));
}

fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

fn pick_folder() -> Option<PathBuf> {
    osascript("POSIX path of (choose folder with prompt \"Choose your installed Harry Potter and the Chamber of Secrets folder (the one containing Maps, System and Textures)\")")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
}

fn pick_archive() -> Option<PathBuf> {
    osascript("POSIX path of (choose file with prompt \"Choose a .zip, .7z or .tar archive of your installed game folder\")")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
}

/// A friendly sentence for an importer failure; the importer's own reason follows as a detail.
fn explain(source: &Source, stderr: &str) -> String {
    let detail = stderr.trim().trim_start_matches("hp2_import:").trim();
    let what = match source {
        Source::Folder(path) => format!("“{}” doesn’t look like the installed game.", path.display()),
        Source::Archive(path) => format!("“{}” doesn’t look like an archive of the installed game.", path.display()),
    };
    format!(
        "{what}\n\nChoose the folder where the game is installed: it contains the folders Maps, System and Textures. \
         The game CD or its installer will not work.\n\nDetails: {detail}"
    )
}

/// Imports `source` with the bundled importer. `Err` carries the importer's message.
fn import(dir: &Path, source: &Source, output: &Path) -> Result<(), String> {
    let (flag, path) = match source {
        Source::Folder(path) => ("--retail-root", path),
        Source::Archive(path) => ("--archive", path),
    };
    let result = Command::new(dir.join("hp2_import"))
        .arg(flag)
        .arg(path)
        .args(["--profile", "retail-only", "--output"])
        .arg(output)
        .output()
        .map_err(|e| format!("the importer could not start: {e}"))?;
    if result.status.success() { Ok(()) } else { Err(String::from_utf8_lossy(&result.stderr).into_owned()) }
}

/// Walks the player through importing their game. Returns whether data is now installed.
pub(crate) fn run() -> bool {
    let (Some(dir), Some(output)) = (bundle_dir(), data_dir()) else { return false };
    let welcome = "Welcome! To play, show the game where your copy of the original PC game is installed.\n\n\
                   It is copied once, into the game’s own folder, and you won’t be asked again.";
    let mut message = welcome.to_owned();
    loop {
        let choice = hp2_platform::message_box::choose(TITLE, &message, &["Choose Game Folder…", "Choose Archive…", "Quit"], 2);
        // `HP2_FIRST_RUN_SOURCE` stands in for the Finder chooser in automated checks.
        let scripted = std::env::var_os("HP2_FIRST_RUN_SOURCE").map(PathBuf::from);
        let source = match choice {
            Some(0) if scripted.is_some() => scripted.map(Source::Folder),
            Some(0) => pick_folder().map(Source::Folder),
            Some(1) => pick_archive().map(Source::Archive),
            _ => return false,
        };
        // Cancelled in the chooser: back to the welcome.
        let Some(source) = source else {
            message = welcome.to_owned();
            continue;
        };
        notify("Importing your game. This can take a minute…");
        match import(&dir, &source, &output) {
            Ok(()) => {
                notify("Your game is ready. Enjoy!");
                return true;
            }
            Err(error) => message = explain(&source, &error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_are_explained_in_plain_words_with_the_detail_last() {
        let text = explain(&Source::Folder(PathBuf::from("/Volumes/HP2")), "hp2_import: missing Old textures directory under /Volumes/HP2/Textures\n");
        assert!(text.starts_with("“/Volumes/HP2” doesn’t look like the installed game."));
        assert!(text.contains("Maps, System and Textures"));
        assert!(text.ends_with("Details: missing Old textures directory under /Volumes/HP2/Textures"));
    }

    #[test]
    fn applescript_strings_are_escaped() {
        assert_eq!(quoted(r#"say "hi" \ bye"#), r#""say \"hi\" \\ bye""#);
    }
}
