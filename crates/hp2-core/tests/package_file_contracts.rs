use hp2_core::ansi::Tchar;
use hp2_core::package_file::*;
use std::collections::BTreeMap;
use std::ffi::c_void;

fn wide(value: &str) -> Vec<Tchar> {
    value.chars().map(|ch| ch as Tchar).chain([0]).collect()
}

unsafe fn text(mut value: *const c_void) -> String {
    let mut result = String::new();
    while unsafe { *value.cast::<Tchar>() } != 0 {
        result.push(char::from_u32(unsafe { *value.cast::<Tchar>() } as u32).unwrap());
        value = unsafe { value.cast::<Tchar>().add(1).cast() };
    }
    result
}

struct Host {
    language: Vec<Tchar>,
    cd: Vec<Tchar>,
    paths: Vec<Vec<Tchar>>,
    cache: Vec<Tchar>,
    cache_ext: Vec<Tchar>,
    guid: Vec<Tchar>,
    dll: Vec<Tchar>,
    separator: Vec<Tchar>,
    sizes: BTreeMap<String, i32>,
    listings: BTreeMap<String, Vec<Vec<Tchar>>>,
    events: Vec<String>,
    fail_at: Option<usize>,
    enumeration_drops: usize,
}

impl Host {
    fn new(paths: &[&str]) -> Self {
        Self {
            language: wide("int"),
            cd: wide(""),
            paths: paths.iter().map(|p| wide(p)).collect(),
            cache: wide("cache"),
            cache_ext: wide(".uxx"),
            guid: wide("0123456789ABCDEF"),
            dll: wide(".dll"),
            separator: wide("/"),
            sizes: BTreeMap::new(),
            listings: BTreeMap::new(),
            events: Vec::new(),
            fail_at: None,
            enumeration_drops: 0,
        }
    }

    fn event(&mut self, value: String) -> i32 {
        let index = self.events.len();
        self.events.push(value);
        if self.fail_at == Some(index) { 73 } else { 0 }
    }

    fn exists(&mut self, name: &str) {
        // Zero-length files are valid, not just positive sizes.
        self.sizes.insert(name.into(), 0);
    }

    fn listing(&mut self, spec: &str, names: &[&str]) {
        self.listings
            .insert(spec.into(), names.iter().map(|name| wide(name)).collect());
    }

    fn probes(&self) -> Vec<&str> {
        self.events
            .iter()
            .filter_map(|event| event.strip_prefix("size:"))
            .collect()
    }

    fn finds(&self) -> Vec<&str> {
        self.events
            .iter()
            .filter_map(|event| event.strip_prefix("find:"))
            .collect()
    }

    fn run(&mut self, input: &str, guid: bool) -> (Result<bool, i32>, String) {
        self.run_into(input, guid, 256)
    }

    /// Runs with a `capacity`-unit output followed by canaries that must survive.
    fn run_into(
        &mut self,
        input: &str,
        guid: bool,
        capacity: usize,
    ) -> (Result<bool, i32>, String) {
        const CANARY: Tchar = 0x5A5A;
        let input = wide(input);
        let mut output = vec![CANARY; capacity + 16];
        let sentinel = wide("untouched");
        output[..sentinel.len()].copy_from_slice(&sentinel);
        let result = unsafe {
            find_package_file(
                &SERVICES,
                (self as *mut Self).cast(),
                input.as_ptr(),
                guid,
                output.as_mut_ptr(),
                capacity,
            )
        };
        assert!(output[capacity..].iter().all(|&unit| unit == CANARY));
        (result, unsafe { text(output.as_ptr().cast()) })
    }
}

unsafe extern "C" fn get_string(
    context: *mut c_void,
    kind: i32,
    index: i32,
    output: *mut *const c_void,
) -> i32 {
    let host = unsafe { &mut *context.cast::<Host>() };
    let error = host.event(format!("string:{kind}:{index}"));
    if error != 0 {
        return error;
    }
    let value = match kind {
        LANGUAGE => &host.language,
        CD_PATH => &host.cd,
        SEARCH_PATH => &host.paths[index as usize],
        CACHE_PATH => &host.cache,
        CACHE_EXT => &host.cache_ext,
        GUID => &host.guid,
        DLL_EXT => &host.dll,
        SEPARATOR => &host.separator,
        _ => unreachable!(),
    };
    unsafe { *output = value.as_ptr().cast() };
    0
}

unsafe extern "C" fn count(context: *mut c_void, output: *mut i32) -> i32 {
    let host = unsafe { &mut *context.cast::<Host>() };
    let error = host.event("count".into());
    if error == 0 {
        unsafe { *output = host.paths.len() as i32 };
    }
    error
}

unsafe extern "C" fn compare(
    context: *mut c_void,
    left: *const c_void,
    right: *const c_void,
    output: *mut i32,
) -> i32 {
    let host = unsafe { &mut *context.cast::<Host>() };
    let (left, right) = unsafe { (text(left), text(right)) };
    let error = host.event(format!("compare:{left}:{right}"));
    if error == 0 {
        unsafe { *output = i32::from(!left.eq_ignore_ascii_case(&right)) };
    }
    error
}

unsafe extern "C" fn file_size(context: *mut c_void, path: *const c_void, output: *mut i32) -> i32 {
    let host = unsafe { &mut *context.cast::<Host>() };
    let path = unsafe { text(path) };
    let error = host.event(format!("size:{path}"));
    if error == 0 {
        unsafe { *output = *host.sizes.get(&path).unwrap_or(&-1) };
    }
    error
}

unsafe extern "C" fn enumerate(
    context: *mut c_void,
    spec: *const c_void,
    visitor: PackageFileVisitor,
    complete: PackageFileComplete,
    visitor_context: *mut c_void,
) -> i32 {
    let spec = unsafe { text(spec) };
    let host = unsafe { &mut *context.cast::<Host>() };
    let error = host.event(format!("find:{spec}"));
    if error != 0 {
        return error;
    }
    // Own the listing across reentrant visitor callbacks, just like native TArray.
    let files = host.listings.get(&spec).cloned().unwrap_or_default();
    for file in &files {
        let error = unsafe { visitor(visitor_context, file.as_ptr().cast()) };
        if error != 0 {
            unsafe {
                (*context.cast::<Host>()).enumeration_drops += 1;
            }
            return error;
        }
    }
    let error = unsafe { complete(visitor_context) };
    unsafe {
        (*context.cast::<Host>()).enumeration_drops += 1;
    }
    error
}

unsafe extern "C" fn touch(context: *mut c_void, path: *const c_void) -> i32 {
    unsafe { (&mut *context.cast::<Host>()).event(format!("touch:{}", text(path))) }
}

unsafe extern "C" fn diagnostic(
    context: *mut c_void,
    input: *const c_void,
    output: *const c_void,
) -> i32 {
    unsafe {
        (&mut *context.cast::<Host>()).event(format!("diagnostic:{}:{}", text(input), text(output)))
    }
}

unsafe extern "C" fn too_long(context: *mut c_void, path: *const c_void, capacity: i32) -> i32 {
    unsafe { (&mut *context.cast::<Host>()).event(format!("too_long:{}:{capacity}", text(path))) }
}

static SERVICES: Hp2PackageFileServices = Hp2PackageFileServices {
    string: get_string,
    path_count: count,
    compare,
    file_size,
    enumerate,
    touch,
    diagnostic,
    too_long,
};

#[test]
fn candidates_longer_than_the_old_scratch_buffers_are_probed_whole() {
    // A save directory under a long HOME: prefix alone exceeds the old TCHAR[256].
    let directory = format!("/{}/Save/", "h".repeat(300));
    let mut host = Host::new(&[&format!("{directory}*.usa")]);
    let found = format!("{directory}X.usa");
    host.exists(&found);
    assert_eq!(host.run_into("X", false, 400), (Ok(true), found.clone()));
    assert_eq!(host.probes(), ["X", &format!("{directory}X"), &found]);
    let mut host = Host::new(&[&format!("{directory}*.usa")]);
    host.listing(&format!("{directory}*.usa"), &["x.USA"]);
    let listed = format!("{directory}x.USA");
    host.exists(&listed);
    assert_eq!(host.run_into("X", false, 400), (Ok(true), listed));
}

#[test]
fn hits_that_do_not_fit_the_output_are_logged_misses_without_truncation() {
    let directory = format!("/{}/", "d".repeat(300));
    let found = format!("{directory}X.unr");
    // Exact pass: stop at the hit, report it, and leave an empty output.
    let mut host = Host::new(&[&format!("{directory}*.unr"), "later/*.unr"]);
    host.exists(&found);
    host.exists("later/X.unr");
    assert_eq!(host.run("X", false), (Ok(false), String::new()));
    assert_eq!(
        host.events.last().unwrap(),
        &format!("too_long:{found}:256")
    );
    assert!(!host.probes().contains(&"later/X.unr"));
    // Enumeration pass: no diagnostic or timestamp for an unreturnable hit.
    let mut host = Host::new(&[&format!("{directory}*.unr")]);
    host.listing(&format!("{directory}*.unr"), &["x.UNR"]);
    let listed = format!("{directory}x.UNR");
    host.exists(&listed);
    assert_eq!(host.run("X", false), (Ok(false), String::new()));
    assert_eq!(
        host.events.last().unwrap(),
        &format!("too_long:{listed}:256")
    );
    assert!(
        !host
            .events
            .iter()
            .any(|event| event.starts_with("diagnostic:"))
    );
    // A miss whose last candidate is too long also leaves an empty output.
    let mut host = Host::new(&[&format!("{directory}*.unr")]);
    assert_eq!(host.run("X", false), (Ok(false), String::new()));
    // The capacity counts the terminator: 16 units hold 15 code units, not 16.
    let mut host = Host::new(&["a/"]);
    host.exists("a/X");
    assert_eq!(host.run_into("X", false, 16), (Ok(true), "a/X".into()));
    let mut host = Host::new(&[]);
    host.exists("abcdefghijklmnop");
    assert_eq!(
        host.run_into("abcdefghijklmnop", false, 16),
        (Ok(false), String::new())
    );
    assert_eq!(host.events.last().unwrap(), "too_long:abcdefghijklmnop:16");
}

#[test]
fn library_rejection_precedes_language_files_and_output_mutation() {
    let mut host = Host::new(&["p/*.unr"]);
    host.exists("plugin.DLL");
    assert_eq!(
        host.run("plugin.DLL", true),
        (Ok(false), "untouched".into())
    );
    assert_eq!(host.events, ["string:6:0", "compare:.DLL:.dll"]);
    let mut host = Host::new(&[]);
    host.exists(".dll");
    assert_eq!(host.run(".dll", false), (Ok(true), ".dll".into()));
    assert_eq!(host.probes(), [".dll"]);
}

#[test]
fn direct_normalized_path_precedes_localization_and_configured_paths() {
    let mut host = Host::new(&["p/*.unr"]);
    host.language = wide("frt");
    host.exists("Maps//X.unr");
    host.exists("p/X.unr.frt_unr");
    assert_eq!(
        host.run("Maps\\/X.unr", false),
        (Ok(true), "Maps//X.unr".into())
    );
    assert_eq!(host.probes(), ["Maps//X.unr"]);
    assert!(!host.events.iter().any(|event| event == "count"));
}

#[test]
fn overlong_inputs_are_probed_whole_instead_of_matching_their_truncation() {
    let mut host = Host::new(&[]);
    let input = "a".repeat(260);
    host.exists(&"a".repeat(255));
    assert_eq!(host.run(&input, false).0, Ok(false));
    assert_eq!(host.probes()[0], input);
}

#[test]
fn variant_language_and_path_order_is_explicit() {
    let mut host = Host::new(&["p/*.unr", "q/*.unr"]);
    host.language = wide("frt");
    host.exists("p/Maps/X.unr");
    host.exists("q/X.frt_unr");
    assert_eq!(
        host.run("Maps\\X", false),
        (Ok(true), "p/Maps/X.unr".into())
    );
    assert_eq!(
        host.probes(),
        [
            "Maps/X",
            "p/Maps/X",
            "p/Maps/X.frt_unr",
            "p/X",
            "p/X.frt_unr",
            "p/Maps/X",
            "p/Maps/X.unr",
        ]
    );
    assert!(host.finds().is_empty());
}

#[test]
fn bare_variant_localized_hit_precedes_full_variant_default_extension() {
    let mut host = Host::new(&["p/*.unr"]);
    host.language = wide("frt");
    host.exists("p/X.frt_unr");
    host.exists("p/Maps/X.unr");
    assert_eq!(host.run("Maps\\X", false), (Ok(true), "p/X.frt_unr".into()));
    assert_eq!(
        host.probes(),
        [
            "Maps/X",
            "p/Maps/X",
            "p/Maps/X.frt_unr",
            "p/X",
            "p/X.frt_unr"
        ]
    );
}

#[test]
fn extensions_are_appended_even_when_input_already_has_one() {
    let mut host = Host::new(&["p/*.unr"]);
    host.language = wide("frt");
    host.exists("p/X.unr.unr");
    assert_eq!(host.run("X.unr", false), (Ok(true), "p/X.unr.unr".into()));
    assert_eq!(
        host.probes(),
        [
            "X.unr",
            "p/X.unr",
            "p/X.unr.frt_unr",
            "p/X.unr",
            "p/X.unr.unr"
        ]
    );
}

#[test]
fn missing_wildcard_has_no_extension_and_non_dot_suffix_keeps_ext_plus_one() {
    let mut host = Host::new(&["literal/", "odd/*abc"]);
    host.exists("odd/X.bc");
    assert_eq!(host.run("X", false), (Ok(true), "odd/X.bc".into()));
    assert_eq!(host.probes(), ["X", "literal/X", "odd/X", "odd/X.bc"]);
}

#[test]
fn cache_guid_lookup_follows_paths_and_touches_only_a_cache_hit() {
    let mut host = Host::new(&["p/*.unr"]);
    host.language = wide("frt");
    host.exists("cache/0123456789ABCDEF.frt_uxx");
    assert_eq!(
        host.run("X", true),
        (Ok(true), "cache/0123456789ABCDEF.frt_uxx".into())
    );
    assert_eq!(
        host.probes(),
        [
            "X",
            "p/X",
            "p/X.frt_unr",
            "p/X",
            "p/X.unr",
            "cache/0123456789ABCDEF",
            "cache/0123456789ABCDEF.frt_uxx",
        ]
    );
    assert_eq!(
        host.events.last().unwrap(),
        "touch:cache/0123456789ABCDEF.frt_uxx"
    );
    let mut host = Host::new(&["p/*.unr"]);
    host.exists("p/X");
    host.exists("cache/0123456789ABCDEF");
    assert_eq!(host.run("X", true), (Ok(true), "p/X".into()));
    assert!(!host.events.iter().any(|event| event.starts_with("touch:")));
}

#[test]
fn guid_without_extension_is_accepted_and_touched() {
    let mut host = Host::new(&[]);
    host.exists("cache/0123456789ABCDEF");
    assert_eq!(
        host.run("X", true),
        (Ok(true), "cache/0123456789ABCDEF".into())
    );
    assert_eq!(host.probes(), ["X", "cache/0123456789ABCDEF"]);
    assert_eq!(host.events.last().unwrap(), "touch:cache/0123456789ABCDEF");
}

#[test]
fn cd_exact_pass_skips_path_zero_but_fallback_includes_it() {
    let mut host = Host::new(&["p0/*.unr", "p1/*.unr"]);
    host.cd = wide("cd/");
    host.exists("cd/System/p0/X.unr");
    host.exists("cd/System/p1/X.unr");
    assert_eq!(
        host.run("X", false),
        (Ok(true), "cd/System/p1/X.unr".into())
    );
    assert_eq!(
        host.probes(),
        [
            "X",
            "p0/X",
            "p0/X.unr",
            "p1/X",
            "p1/X.unr",
            "cd/System/p1/X",
            "cd/System/p1/X.unr"
        ]
    );
    let mut host = Host::new(&["p0/*.unr", "p1/*.unr"]);
    host.cd = wide("cd/");
    host.exists("cd/System/p0/x.UNR");
    host.listing("cd/System/p0/*.unr", &["x.UNR"]);
    assert_eq!(
        host.run("X", false),
        (Ok(true), "cd/System/p0/x.UNR".into())
    );
    assert_eq!(host.finds(), ["p0/*.unr", "p1/*.unr", "cd/System/p0/*.unr"]);
}

#[test]
fn fallback_uses_bare_input_and_preserves_enumerated_spelling() {
    let mut host = Host::new(&["p/*.unr", "q/*.unr"]);
    host.language = wide("frt");
    host.listing("p/*.unr", &["unrelated.unr", "x.UnR"]);
    host.exists("p/x.UnR");
    assert_eq!(host.run("Maps\\X", false), (Ok(true), "p/x.UnR".into()));
    assert_eq!(host.finds(), ["p/*.unr"]);
    assert_eq!(
        host.events
            .iter()
            .filter(|event| event.starts_with("diagnostic:"))
            .collect::<Vec<_>>(),
        [&"diagnostic:Maps\\X:p/x.UnR".to_string()]
    );
    assert_eq!(host.enumeration_drops, 1);
}

#[test]
fn fallback_last_matching_size_wins_and_nonmatches_do_not_reset_it() {
    for (second_exists, trailing_nonmatch, expected, output) in [
        (true, false, true, "p/x.UNR"),
        (false, false, false, "p/x.UNR"),
        (false, true, true, "p/X.UnR"),
    ] {
        let mut host = Host::new(&["p/*.unr"]);
        host.exists("p/X.UnR");
        if second_exists {
            host.exists("p/x.UNR");
        }
        let second = if trailing_nonmatch {
            "unrelated.unr"
        } else {
            "x.UNR"
        };
        host.listing("p/*.unr", &["X.UnR", second]);
        assert_eq!(host.run("X", false), (Ok(expected), output.into()));
        assert_eq!(host.probes().last().copied(), Some(output));
    }
}

#[test]
fn fallback_cache_matches_package_name_not_guid_and_logs_before_touching() {
    let mut host = Host::new(&[]);
    host.listing("cache/*.uxx", &["0123456789ABCDEF.uxx", "x.UXX"]);
    // A stale GUID listing is not a name match; only the package-name entry is.
    host.exists("cache/x.UXX");
    assert_eq!(host.run("X", true), (Ok(true), "cache/x.UXX".into()));
    assert_eq!(
        host.probes(),
        [
            "X",
            "cache/0123456789ABCDEF",
            "cache/0123456789ABCDEF.uxx",
            "cache/x.UXX"
        ]
    );
    let effects: Vec<&str> = host
        .events
        .iter()
        .filter(|event| event.starts_with("diagnostic:") || event.starts_with("touch:"))
        .map(String::as_str)
        .collect();
    assert_eq!(effects, ["diagnostic:X:cache/x.UXX", "touch:cache/x.UXX"]);
}

#[test]
fn misses_preserve_the_last_native_output_mutation() {
    let mut host = Host::new(&["p/*.unr", "last/"]);
    assert_eq!(host.run("Maps\\X", false), (Ok(false), "last/X".into()));
    assert_eq!(host.finds(), ["p/*.unr", "last/*"]);
    let mut host = Host::new(&[]);
    assert_eq!(host.run("Maps\\X", false), (Ok(false), "Maps/X".into()));
    let mut host = Host::new(&[]);
    assert_eq!(
        host.run("X", true),
        (Ok(false), "cache/0123456789ABCDEF".into())
    );
}

fn error_host() -> Host {
    let mut host = Host::new(&["p/*.unr"]);
    host.language = wide("frt");
    host.cd = wide("cd/");
    host.listing("cache/*.uxx", &["unrelated.uxx", "x.UXX"]);
    host.exists("cache/x.UXX");
    host
}

#[test]
fn every_native_failure_stops_immediately_and_preserves_its_status() {
    let mut successful = error_host();
    assert_eq!(
        successful.run("Maps\\X", true),
        (Ok(true), "cache/x.UXX".into())
    );
    // Inject at every native boundary in this full path/CD/cache/fallback trace,
    // including visitor comparisons, diagnostic and final timestamp update.
    for fail_at in 0..successful.events.len() {
        let mut host = error_host();
        host.fail_at = Some(fail_at);
        let (result, _) = host.run("Maps\\X", true);
        assert_eq!(result, Err(73), "{}", successful.events[fail_at]);
        assert_eq!(
            host.events,
            successful.events[..=fail_at],
            "first error at {fail_at}"
        );
    }
}

#[test]
fn failed_size_diagnostic_and_timestamp_do_not_roll_back_output() {
    for target in [
        "size:cache/x.UXX",
        "diagnostic:Maps\\X:cache/x.UXX",
        "touch:cache/x.UXX",
    ] {
        let mut successful = error_host();
        successful.run("Maps\\X", true);
        let index = successful
            .events
            .iter()
            .position(|event| event == target)
            .unwrap();
        let mut host = error_host();
        host.fail_at = Some(index);
        assert_eq!(host.run("Maps\\X", true), (Err(73), "cache/x.UXX".into()));
        assert_eq!(host.enumeration_drops, 2);
    }
}
