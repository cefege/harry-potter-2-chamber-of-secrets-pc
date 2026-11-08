use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;

use hp2_import::output::{Options, run_import};
use hp2_import::plan::LinkMode;
use hp2_ini::Profile;

fn value(args: &mut impl Iterator<Item = OsString>, name: &str) -> Result<OsString, Vec<u8>> {
    args.next()
        .ok_or_else(|| format!("argument {name}: expected one argument").into_bytes())
}

fn path(value: OsString, error: &'static str) -> Result<String, Vec<u8>> {
    value.into_string().map_err(|_| error.as_bytes().to_vec())
}

fn parse(mut args: impl Iterator<Item = OsString>) -> Result<Options, Vec<u8>> {
    let mut retail_root = OsString::new();
    let mut archive = OsString::new();
    let mut prototype_root = OsString::new();
    let mut output = OsString::new();
    let mut retail_seen = false;
    let mut archive_seen = false;
    let mut profile = Profile::Safe;
    let mut link_mode = LinkMode::Auto;
    let mut check_only = false;

    // On the supported Unix platforms these are argv's original bytes.
    // Parse before UTF-8 conversion: native argument errors preserve raw bytes,
    // and overwritten path values never reached the FFI's conversion boundary.
    while let Some(arg) = args.next() {
        match arg.as_encoded_bytes() {
            b"--prototype-root" => prototype_root = value(&mut args, "--prototype-root")?,
            b"--retail-root" => {
                retail_root = value(&mut args, "--retail-root")?;
                retail_seen = true;
            }
            b"--archive" => {
                archive = value(&mut args, "--archive")?;
                archive_seen = true;
            }
            b"--output" => output = value(&mut args, "--output")?,
            b"--profile" => {
                let v = value(&mut args, "--profile")?;
                profile = match v.as_encoded_bytes() {
                    b"safe" => Profile::Safe,
                    b"full" => Profile::Full,
                    b"retail-only" => Profile::RetailOnly,
                    _ => {
                        return Err([
                            b"argument --profile: invalid choice: '".as_slice(),
                            v.as_encoded_bytes(),
                            b"' (choose from 'safe', 'full', 'retail-only')",
                        ]
                        .concat());
                    }
                };
            }
            b"--link-mode" => {
                let v = value(&mut args, "--link-mode")?;
                link_mode = match v.as_encoded_bytes() {
                    b"auto" => LinkMode::Auto,
                    b"copy" => LinkMode::Copy,
                    b"hardlink" => LinkMode::Hardlink,
                    _ => {
                        return Err([
                            b"argument --link-mode: invalid choice: '".as_slice(),
                            v.as_encoded_bytes(),
                            b"' (choose from 'auto', 'copy', 'hardlink')",
                        ]
                        .concat());
                    }
                };
            }
            b"--check" => check_only = true,
            _ => {
                return Err([b"unrecognized arguments: ".as_slice(), arg.as_encoded_bytes()].concat());
            }
        }
    }
    // Presence, not nonemptiness, is the native parser's XOR condition.
    if retail_seen == archive_seen {
        return Err(b"one of the arguments --retail-root --archive is required".to_vec());
    }
    if output.is_empty() {
        return Err(b"the following arguments are required: --output".to_vec());
    }
    // Keep the deleted hp2_rs_import_run's former UTF-8 checks in their
    // original order, independent of the order in which the flags appeared
    // in argv (also matched by hp2-launcher's import_session::run_retail_import).
    Ok(Options {
        retail_root: path(retail_root, "retail root path is not valid UTF-8")?,
        archive: path(archive, "archive path is not valid UTF-8")?,
        prototype_root: path(prototype_root, "prototype root path is not valid UTF-8")?,
        output: path(output, "output path is not valid UTF-8")?,
        profile,
        link_mode,
        check_only,
    })
}

fn main() -> ExitCode {
    match parse(std::env::args_os().skip(1))
        .and_then(|options| run_import(&options, None).map_err(String::into_bytes))
    {
        Ok(log) => {
            let _ = io::stdout().lock().write_all(log.as_bytes());
            ExitCode::SUCCESS
        }
        Err(error) => {
            let mut stderr = io::stderr().lock();
            let _ = stderr.write_all(b"hp2_import: ");
            let _ = stderr.write_all(&error);
            let _ = stderr.write_all(b"\n");
            ExitCode::from(2)
        }
    }
}
