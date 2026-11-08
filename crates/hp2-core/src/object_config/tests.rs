use super::*;
use std::collections::BTreeMap;

#[derive(Default)]
struct Entry {
    class: bool,
    related: bool,
    loaded: Option<String>,
    edited: Option<String>,
}
struct State {
    source: usize,
    section: Option<Vec<(Vec<i32>, Vec<i32>)>>,
    values: BTreeMap<String, String>,
    objects: Vec<Entry>,
    selected: Option<usize>,
    read: bool,
    fail_key: Option<String>,
    grow: bool,
}
impl State {
    fn new(section: Option<&str>) -> Self {
        Self {
            source: 0,
            section: section.map(|s| {
                s.split('\0').filter_map(|entry| entry.split_once('='))
                    .map(|(key, value)| {
                        let wide = |text: &str| text.chars().map(|c| c as i32)
                            .chain(std::iter::once(0)).collect();
                        (wide(key), wide(value))
                    }).collect()
            }),
            values: BTreeMap::from([
                ("Value".into(), "old".into()),
                ("Keep".into(), "retained".into()),
            ]),
            objects: vec![
                Entry {
                    class: true,
                    related: true,
                    ..Entry::default()
                },
                Entry {
                    class: true,
                    ..Entry::default()
                },
                Entry {
                    related: true,
                    ..Entry::default()
                },
                Entry::default(),
            ],
            selected: None,
            read: false,
            fail_key: None,
            grow: false,
        }
    }
    fn run(&mut self) -> Result<()> {
        let services = Hp2ObjectConfigServices {
            context: (self as *mut State).cast(),
            call,
        };
        let args = Hp2ObjectConfigArgs {
            object: 0,
            object_class: 123,
            filename: 0,
            flags: 0,
            propagate: 0,
        };
        // Reset does not access the deferred-localization queue.
        Native(&services, std::ptr::null_mut()).reset(&args)
    }
}
unsafe fn text(address: usize) -> String {
    let mut p = address as *const i32;
    let mut result = String::new();
    // SAFETY: only called on live, initialized NUL-terminated section entries.
    unsafe {
        while *p != 0 {
            result.push(char::from_u32(*p as u32).expect("valid test text"));
            p = p.add(1);
        }
    }
    result
}
unsafe extern "C" fn call(
    context: *mut c_void,
    op: Hp2ObjectConfigLeaf,
    a: usize,
    b: usize,
    out: *mut usize,
) -> i32 {
    use Hp2ObjectConfigLeaf::*;
    // SAFETY: the test owns State throughout the synchronous kernel call.
    let s = unsafe { &mut *context.cast::<State>() };
    let result = match op {
        IsSystemConfig => usize::from(s.source == 0),
        IsUserConfig => usize::from(s.source == 1),
        ResetSource => {
            s.selected = Some(a);
            0
        }
        ResetReadSection => {
            s.read = true;
            usize::from(s.section.is_some())
        }
        ResetSectionCount => s.section.as_ref().unwrap().len(),
        ResetEntryKey => s.section.as_ref().unwrap()[a].0.as_ptr() as usize,
        ResetEntryValue => s.section.as_ref().unwrap()[a].1.as_ptr() as usize,
        ResetSetKey => {
            // SAFETY: kernel passes split NUL-terminated entries from its buffer.
            let (key, value) = unsafe { (text(a), text(b)) };
            if s.fail_key.as_ref() == Some(&key) {
                return 73;
            }
            s.values.insert(key, value);
            0
        }
        NextClassIndex | NextObjectIndex => {
            let start = (a as i32 + 1) as usize;
            (start..s.objects.len())
                .find(|&i| !matches!(op, NextClassIndex) || s.objects[i].class)
                .unwrap_or(s.objects.len())
        }
        ObjectCount => s.objects.len(),
        ObjectAt => a + 1,
        ChildOf => usize::from(s.objects[a - 1].class && s.objects[a - 1].related),
        IsA => usize::from(!s.objects[a - 1].class && s.objects[a - 1].related),
        ReloadDefault | ReloadObject => {
            s.objects[a - 1].loaded = s.values.get("Value").cloned();
            if s.grow && a == 1 {
                s.objects.push(Entry {
                    class: true,
                    related: true,
                    ..Entry::default()
                });
            }
            if s.grow && a == 3 {
                s.objects.push(Entry {
                    related: true,
                    ..Entry::default()
                });
            }
            0
        }
        PostEditChange => {
            s.objects[a - 1].edited = s.objects[a - 1].loaded.clone();
            0
        }
        _ => return 91,
    };
    // SAFETY: the kernel supplies a writable result slot.
    unsafe { out.write(result) };
    0
}

#[test]
fn reset_preserves_key_value_boundaries_and_visits_new_descendants() {
    let mut s = State::new(Some(
        "Value=reset=tail\0Equals=a=b=c\0Empty=\0=empty-key\0Value=final\0\0",
    ));
    s.grow = true;
    assert_eq!(s.run(), Ok(()));
    assert_eq!(
        s.values,
        BTreeMap::from([
            ("Value".into(), "final".into()),
            ("Keep".into(), "retained".into()),
            ("Empty".into(), "".into()),
            ("Equals".into(), "a=b=c".into()),
            ("".into(), "empty-key".into()),
        ])
    );
    assert_eq!(s.objects.len(), 6);
    for index in [0, 2, 4, 5] {
        assert_eq!(s.objects[index].loaded.as_deref(), Some("final"));
    }
    for index in [2, 5] {
        assert_eq!(s.objects[index].edited.as_deref(), Some("final"));
    }
    for index in [1, 3] {
        assert!(s.objects[index].loaded.is_none());
        assert!(s.objects[index].edited.is_none());
    }
    assert_eq!(s.selected, Some(0));
}

#[test]
fn missing_user_section_still_reloads_existing_values() {
    let mut s = State::new(None);
    s.source = 1;
    assert_eq!(s.run(), Ok(()));
    assert_eq!(s.selected, Some(1));
    assert!(s.read);
    assert_eq!(s.objects[0].loaded.as_deref(), Some("old"));
    assert_eq!(s.objects[2].edited.as_deref(), Some("old"));
    assert_eq!(s.values.get("Keep").map(String::as_str), Some("retained"));
}

#[test]
fn unsupported_config_is_unchanged() {
    let mut s = State::new(Some("Value=new\0\0"));
    s.source = 2;
    assert_eq!(s.run(), Ok(()));
    assert!(!s.read);
    assert_eq!(s.selected, None);
    assert_eq!(s.values.get("Value").map(String::as_str), Some("old"));
    assert!(
        s.objects
            .iter()
            .all(|e| e.loaded.is_none() && e.edited.is_none())
    );
}

#[test]
fn write_failure_keeps_prior_writes_but_does_not_reload_objects() {
    let mut s = State::new(Some("One=first\0Value=new\0\0"));
    s.fail_key = Some("Value".into());
    assert_eq!(s.run(), Err(73));
    assert_eq!(s.values.get("One").map(String::as_str), Some("first"));
    assert_eq!(s.values.get("Value").map(String::as_str), Some("old"));
    assert!(
        s.objects
            .iter()
            .all(|e| e.loaded.is_none() && e.edited.is_none())
    );
}

#[test]
fn oversized_admitted_value_is_preserved_without_admitting_following_keys() {
    let value = "x".repeat(40_000);
    let input = format!("Value={value}\0After=excluded\0\0");
    let mut s = State::new(Some(&input));
    assert_eq!(s.run(), Ok(()));
    assert_eq!(s.values["Value"], value);
    assert!(!s.values.contains_key("After"));
    assert_eq!(s.objects[2].loaded.as_deref(), Some(value.as_str()));
}

#[test]
fn key_at_historical_capacity_boundary_is_not_admitted() {
    let input = format!("{}=excluded\0\0", "k".repeat(32766));
    let mut s = State::new(Some(&input));
    assert_eq!(s.run(), Ok(()));
    assert_eq!(s.values.len(), 2);
    assert_eq!(s.objects[2].loaded.as_deref(), Some("old"));
}
