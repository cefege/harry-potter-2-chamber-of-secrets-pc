//! Canonical ownership and operations for compiler-probed FURL records.

use crate::url::{self, Op, UrlArgs, UrlOps};
use hp2_abi::engine::generated::{FURL, TArrayRaw};
use hp2_core::{
    array_storage::{self, Hp2ArraySlots},
    core_access::{CoreAccess, RuntimeError, RuntimeResult},
    name_owner::FindName,
    object_archive::ObjectArchive,
};
use std::{borrow::Cow, cell::RefCell, mem::MaybeUninit, ptr, rc::Rc};
const DEFAULT_KEYS: [&[u32]; 9] = [
    &[80, 114, 111, 116, 111, 99, 111, 108],
    &[80, 114, 111, 116, 111, 99, 111, 108, 68, 101, 115, 99, 114, 105, 112, 116, 105, 111, 110],
    &[78, 97, 109, 101], &[77, 97, 112], &[76, 111, 99, 97, 108, 77, 97, 112],
    &[72, 111, 115, 116], &[80, 111, 114, 116, 97, 108], &[77, 97, 112, 69, 120, 116],
    &[83, 97, 118, 101, 69, 120, 116],
];
const URL_SECTION: &[u32] = &[85, 82, 76];

/// Configured native URL defaults loaded from Core's real configuration cache.
#[derive(Clone, Debug)]
pub struct UrlDefaults {
    fields: [Vec<u32>; 9],
    port: i32,
}

impl Default for UrlDefaults {
    fn default() -> Self {
        // Native statics are empty FStrings/port zero before Engine::Init.
        Self { fields: std::array::from_fn(|_| Vec::new()), port: 0 }
    }
}

impl UrlDefaults {
    fn text(&self, id: usize) -> &[u32] {
        if self.fields[id].is_empty() { &EMPTY } else { &self.fields[id] }
    }
    /// Returns the configured `FURL::DefaultName` TCHAR string.
    pub fn default_name(&self) -> &[u32] {
        self.text(2)
    }

    /// Returns the configured `FURL::DefaultMapExt` TCHAR string.
    pub fn default_map_ext(&self) -> &[u32] {
        self.text(7)
    }
    /// Returns the configured `FURL::DefaultSaveExt` TCHAR string.
    pub fn default_save_ext(&self) -> &[u32] {
        self.text(8)
    }


    /// Loads FURL statics from the canonical URL configuration section.
    pub fn load(access: CoreAccess<'_>) -> RuntimeResult<Self> {
        let backend = CoreUrlOps::new(
            access,
            Cow::Owned(Self::default()),
        );
        if unsafe { url::run_with(&backend, 0, &UrlArgs::default()) }.is_err() {
            return Err(backend.errors.borrow_mut().take().unwrap_or_else(|| RuntimeError::new("Unable to load FURL defaults")));
        }
        Ok(backend.defaults.into_inner().into_owned())
    }
}

/// Constructs, updates, serializes, and destroys canonical FURL storage.
pub struct UrlOwner;

impl UrlOwner {
    /// Returns this owner's stable current FURL static state.
    pub fn current_defaults(access: CoreAccess<'_>) -> RuntimeResult<Rc<UrlDefaults>> {
        let state = access.owner_context::<crate::core_owner::EngineState>()
            .ok_or_else(|| RuntimeError::new("URL operation has no canonical Engine owner"))?;
        Ok(Rc::clone(&state.url_defaults.borrow()))
    }

    /// Performs native FURL::StaticInit at genuine Engine initialization.
    pub fn static_init(access: CoreAccess<'_>) -> RuntimeResult<()> {
        let defaults = UrlDefaults::load(access)?;
        let state = access.owner_context::<crate::core_owner::EngineState>()
            .ok_or_else(|| RuntimeError::new("URL initialization has no canonical Engine owner"))?;
        *state.url_defaults.borrow_mut() = Rc::new(defaults);
        Ok(())
    }

    /// Native StaticExit empties the strings but leaves DefaultPort unchanged.
    pub fn static_exit(access: CoreAccess<'_>) -> RuntimeResult<()> {
        let state = access.owner_context::<crate::core_owner::EngineState>()
            .ok_or_else(|| RuntimeError::new("URL shutdown has no canonical Engine owner"))?;
        let port = state.url_defaults.borrow().port;
        *state.url_defaults.borrow_mut() = Rc::new(UrlDefaults { port, ..UrlDefaults::default() });
        Ok(())
    }

    /// Constructs the native no-argument FURL default state from loaded statics.
    ///
    /// # Safety
    /// `url` points to writable, uninitialized compiler-probed FURL storage.
    pub unsafe fn construct_default(
        access: CoreAccess<'_>,
        defaults: &UrlDefaults,
        url: *mut FURL,
    ) -> RuntimeResult<()> {
        unsafe { Self::construct_local_filename(access, defaults, url, None) }
    }

    /// Constructs `FURL(LocalFilename)` without interpreting URL delimiters.
    ///
    /// # Safety
    /// `url` is writable uninitialized FURL storage; filename units, if present,
    /// are valid for this synchronous construction.
    pub unsafe fn construct_local_filename(
        access: CoreAccess<'_>,
        defaults: &UrlDefaults,
        url: *mut FURL,
        filename: Option<&[u32]>,
    ) -> RuntimeResult<()> {
        if url.is_null() { return Err(RuntimeError::new("Null FURL")); }
        unsafe { ptr::write_bytes(url.cast::<u8>(), 0, std::mem::size_of::<FURL>()); }
        let fields = unsafe { field_ptrs(url) };
        let text = [defaults.text(0), defaults.text(5), filename.unwrap_or_else(|| defaults.text(3)), defaults.text(6)];
        for (field, source) in fields.into_iter().zip(text) {
            if let Err(error) = unsafe { access.assign_fstring(field, source) } {
                let _ = unsafe { Self::destroy(access, url) };
                return Err(error);
            }
        }
        unsafe {
            (*url).port = defaults.port;
            (*url).valid = 1;
        }
        Ok(())
    }

    /// Deep-copies the native URL members without parsing or normalizing options.
    ///
    /// # Safety
    /// Destination is uninitialized storage disjoint from the live source URL.
    pub unsafe fn copy_construct(
        access: CoreAccess<'_>, destination: *mut FURL, source: *const FURL,
    ) -> RuntimeResult<()> {
        if destination.is_null() || source.is_null() {
            return Err(RuntimeError::new("Null FURL copy argument"));
        }
        // SAFETY: zero initializes every owned header before fallible copying.
        unsafe { destination.write(std::mem::zeroed()); }
        let result = (|| {
            for (to, from) in field_ptrs(destination).into_iter().zip(field_ptrs(source.cast_mut())) {
                unsafe { copy_string(to, from)?; }
            }
            let options = unsafe { (*source).op };
            let target = unsafe { ptr::addr_of_mut!((*destination).op) };
            if options.num < 0 || options.max < options.num || (options.num > 0 && options.data.is_null()) {
                return Err(RuntimeError::new("Invalid FURL option storage"));
            }
            unsafe {
                array_storage::set(array_slots(target), options.num,
                    std::mem::size_of::<TArrayRaw>() as i32, ptr::null(),
                    array_storage::system_allocator())
                    .map_err(|_| RuntimeError::new("Unable to copy URL option array"))?;
                let data = (*target).data.cast::<TArrayRaw>();
                if options.num > 0 { ptr::write_bytes(data, 0, options.num as usize); }
                for index in 0..options.num as usize {
                    copy_string(data.add(index), options.data.cast::<TArrayRaw>().add(index))?;
                }
                (*destination).port = (*source).port;
                (*destination).valid = (*source).valid;
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = unsafe { Self::destroy(access, destination) };
        }
        result
    }

    /// Assigns a URL by deep member copy, including duplicate and invalid options.
    ///
    /// # Safety
    /// Both URLs are initialized and live; source may equal destination.
    pub unsafe fn assign_copy(
        access: CoreAccess<'_>, destination: *mut FURL, source: *const FURL,
    ) -> RuntimeResult<()> {
        if ptr::eq(destination, source) { return Ok(()); }
        let mut copied = MaybeUninit::<FURL>::uninit();
        unsafe { Self::copy_construct(access, copied.as_mut_ptr(), source)?; }
        let mut old = unsafe { ptr::replace(destination, copied.assume_init()) };
        unsafe { Self::destroy(access, &mut old) }
    }

    /// Replaces a live URL using native `FURL(base, text, travel)` semantics.
    /// Invalid text produces Valid=0, not an exception.
    ///
    /// # Safety
    /// URL/base and NUL-terminated text are live; travel is ETravelType.
    /// Base may alias URL: construction finishes before the old URL is freed.
    pub unsafe fn parse_update(
        access: CoreAccess<'_>,
        defaults: &UrlDefaults,
        url: *mut FURL,
        base: *mut FURL,
        text: *const u32,
        travel: i32,
    ) -> RuntimeResult<i32> {
        if url.is_null() || text.is_null() { return Err(RuntimeError::new("Invalid FURL parse arguments")); }
        let mut parsed = MaybeUninit::<FURL>::uninit();
        unsafe { Self::construct_default(access, defaults, parsed.as_mut_ptr())?; }
        let backend = CoreUrlOps::new(access, Cow::Borrowed(defaults));
        let args = UrlArgs { url: parsed.as_mut_ptr() as usize, other: base as usize, text: text as usize, mode: travel as usize, ..UrlArgs::default() };
        if let Err(error) = backend.run_policy(2, &args) {
            let _ = unsafe { Self::destroy(access, parsed.as_mut_ptr()) };
            return Err(error);
        }
        let parsed = unsafe { parsed.assume_init() };
        let valid = parsed.valid;
        let mut old = unsafe { ptr::replace(url, parsed) };
        unsafe { Self::destroy(access, &mut old)?; }
        Ok(valid)
    }

    /// Applies native case-insensitive option-key replacement.
    ///
    /// # Safety
    /// URL is initialized and text is live and NUL-terminated.
    pub unsafe fn add_option(access: CoreAccess<'_>, defaults: &UrlDefaults, url: *mut FURL, text: *const u32) -> RuntimeResult<()> {
        CoreUrlOps::new(access, Cow::Borrowed(defaults)).run_policy(
            6, &UrlArgs { url: url as usize, text: text as usize, ..UrlArgs::default() },
        )?;
        Ok(())
    }

    /// Returns the live suffix after a native case-insensitive option prefix.
    ///
    /// # Safety
    /// Inputs are live; match_text is NUL-terminated. Result belongs to URL
    /// (or default), invalidated by its mutation/destruction.
    pub unsafe fn get_option(access: CoreAccess<'_>, defaults: &UrlDefaults, url: *const FURL, match_text: *const u32, default: *const u32) -> RuntimeResult<*const u32> {
        CoreUrlOps::new(access, Cow::Borrowed(defaults)).run_policy(
            10, &UrlArgs { url: url as usize, text: match_text as usize, extra: default as usize, ..UrlArgs::default() },
        ).map(|value| value as *const u32)
    }

    /// Checks a whole native option, case-insensitively.
    ///
    /// # Safety
    /// URL is initialized and text is live and NUL-terminated.
    pub unsafe fn has_option(access: CoreAccess<'_>, defaults: &UrlDefaults, url: *const FURL, text: *const u32) -> RuntimeResult<bool> {
        CoreUrlOps::new(access, Cow::Borrowed(defaults)).run_policy(
            9, &UrlArgs { url: url as usize, text: text as usize, ..UrlArgs::default() },
        ).map(|value| value != 0)
    }

    /// Formats directly into owned Core FString storage.
    ///
    /// # Safety
    /// URL is initialized. Caller destroys the result with destroy_owned_fstring.
    pub unsafe fn string(access: CoreAccess<'_>, defaults: &UrlDefaults, url: *const FURL, fully_qualified: bool) -> RuntimeResult<TArrayRaw> {
        let mut output = TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 };
        let result = CoreUrlOps::new(access, Cow::Borrowed(defaults)).run_policy(
            3, &UrlArgs { url: url as usize, output: ptr::addr_of_mut!(output) as usize, mode: usize::from(fully_qualified), ..UrlArgs::default() },
        );
        if let Err(error) = result {
            let _ = unsafe { access.destroy_owned_fstring(&mut output) };
            return Err(error);
        }
        Ok(output)
    }

    /// Checks whether the protocol matches the native internal protocol.
    ///
    /// # Safety
    /// `url` identifies a live initialized canonical FURL.
    pub unsafe fn is_internal(access: CoreAccess<'_>, defaults: &UrlDefaults, url: *const FURL) -> RuntimeResult<bool> {
        CoreUrlOps::new(access, Cow::Borrowed(defaults)).run_policy(
            4, &UrlArgs { url: url as usize, ..UrlArgs::default() },
        ).map(|value| value != 0)
    }

    /// Checks for an internal URL with no remote host.
    ///
    /// # Safety
    /// `url` identifies a live initialized canonical FURL.
    pub unsafe fn is_local_internal(access: CoreAccess<'_>, defaults: &UrlDefaults, url: *const FURL) -> RuntimeResult<bool> {
        CoreUrlOps::new(access, Cow::Borrowed(defaults)).run_policy(
            5, &UrlArgs { url: url as usize, ..UrlArgs::default() },
        ).map(|value| value != 0)
    }

    /// Compares URLs using native field, port and ordered-option equality.
    ///
    /// # Safety
    /// Both pointers identify live initialized canonical FURLs.
    pub unsafe fn equal(access: CoreAccess<'_>, defaults: &UrlDefaults, url: *const FURL, other: *const FURL) -> RuntimeResult<bool> {
        CoreUrlOps::new(access, Cow::Borrowed(defaults)).run_policy(
            11, &UrlArgs { url: url as usize, other: other as usize, ..UrlArgs::default() },
        ).map(|value| value != 0)
    }

    /// Adds options from a canonical configuration section.
    ///
    /// # Safety
    /// URL is initialized; section/optional filename are NUL-terminated.
    pub unsafe fn load_config(access: CoreAccess<'_>, defaults: &UrlDefaults, url: *mut FURL, section: *const u32, filename: *const u32) -> RuntimeResult<()> {
        CoreUrlOps::new(access, Cow::Borrowed(defaults)).run_policy(
            7, &UrlArgs { url: url as usize, text: section as usize, extra: filename as usize, ..UrlArgs::default() },
        )?;
        Ok(())
    }

    /// Saves native key-matching options into the canonical configuration cache.
    ///
    /// # Safety
    /// URL is initialized; section/key and optional filename are NUL-terminated.
    pub unsafe fn save_config(access: CoreAccess<'_>, defaults: &UrlDefaults, url: *const FURL, section: *const u32, key: *const u32, filename: *const u32) -> RuntimeResult<()> {
        CoreUrlOps::new(access, Cow::Borrowed(defaults)).run_policy(
            8, &UrlArgs { url: url as usize, text: section as usize, extra: key as usize, output: filename as usize, ..UrlArgs::default() },
        )?;
        Ok(())
    }

    /// Serializes FURL members in native `operator<<` order.
    ///
    /// # Safety
    /// `url` identifies an initialized canonical FURL.
    pub unsafe fn serialize(
        access: CoreAccess<'_>,
        archive: &ObjectArchive<'_>,
        url: *mut FURL,
    ) -> RuntimeResult<()> {
        if url.is_null() { return Err(RuntimeError::new("Null FURL")); }
        unsafe {
            access.serialize_owned_fstring(archive, ptr::addr_of_mut!((*url).protocol))?;
            access.serialize_owned_fstring(archive, ptr::addr_of_mut!((*url).host))?;
            access.serialize_owned_fstring(archive, ptr::addr_of_mut!((*url).map))?;
            access.serialize_owned_fstring(archive, ptr::addr_of_mut!((*url).portal))?;
            access.serialize_owned_fstring_array(archive, ptr::addr_of_mut!((*url).op))?;
            archive.serialize_i32(&mut (*url).port)?;
            archive.serialize_i32(&mut (*url).valid)?;
        }
        Ok(())
    }

    /// Destroys all paired FString and option-array allocations without allocating.
    ///
    /// # Safety
    /// `url` identifies an initialized canonical FURL and is not used afterwards.
    pub unsafe fn destroy(access: CoreAccess<'_>, url: *mut FURL) -> RuntimeResult<()> {
        if url.is_null() { return Err(RuntimeError::new("Null FURL")); }
        let mut first = None;
        if let Err(error) = unsafe { access.destroy_owned_fstring_array(ptr::addr_of_mut!((*url).op)) } {
            first = Some(error);
        }
        if let Err(error) = unsafe { access.destroy_owned_fstring(ptr::addr_of_mut!((*url).portal)) } {
            if first.is_none() { first = Some(error); }
        }
        for field in [
            unsafe { ptr::addr_of_mut!((*url).map) },
            unsafe { ptr::addr_of_mut!((*url).host) },
            unsafe { ptr::addr_of_mut!((*url).protocol) },
        ] {
            if let Err(error) = unsafe { access.destroy_owned_fstring(field) } {
                if first.is_none() { first = Some(error); }
            }
        }
        match first { Some(error) => Err(error), None => Ok(()) }
    }
}

fn field_ptrs(url: *mut FURL) -> [*mut TArrayRaw; 4] {
    unsafe { [
        ptr::addr_of_mut!((*url).protocol), ptr::addr_of_mut!((*url).host),
        ptr::addr_of_mut!((*url).map), ptr::addr_of_mut!((*url).portal),
    ] }
}

struct CoreUrlOps<'a> {
    access: CoreAccess<'a>,
    defaults: RefCell<Cow<'a, UrlDefaults>>,
    scratch: RefCell<Vec<u32>>,
    errors: RefCell<Option<RuntimeError>>,
}

impl<'a> CoreUrlOps<'a> {
    fn new(access: CoreAccess<'a>, defaults: Cow<'a, UrlDefaults>) -> Self {
        Self { access, defaults: RefCell::new(defaults), scratch: RefCell::new(Vec::new()), errors: RefCell::new(None) }
    }

    unsafe fn op(&self, code: u32, a: usize, b: usize, c: usize, d: usize) -> RuntimeResult<usize> {
        let op = match code {
            0 => Op::StaticText, 1 => Op::StaticSet, 2 => Op::DefaultPort, 3 => Op::SetDefaultPort,
            4 => Op::ConfigGet, 5 => Op::ConfigSet, 6 => Op::Icmp, 7 => Op::Nicmp, 8 => Op::Cmp,
            9 => Op::Atoi, 10 => Op::Len, 11 => Op::Chr, 12 => Op::Char, 13 => Op::SetChar,
            14 => Op::Field, 15 => Op::SetField, 16 => Op::Port, 17 => Op::SetPort, 18 => Op::Count,
            19 => Op::Option, 20 => Op::Push, 21 => Op::SetOption, 22 => Op::Reset, 23 => Op::SetValid,
            24 => Op::Append, 25 => Op::AppendPort, 26 => Op::LogDefault, 27 => Op::Internal,
            28 => Op::AddOption, 29 => Op::GetOption, 30 => Op::Literal, 31 => Op::CopyField,
            32 => Op::CopyOption, 33 => Op::FieldBool, 34 => Op::FieldLen, 35 => Op::StaticLen,
            36 => Op::CopyStaticField, 37 => Op::CheckBase, 38 => Op::MatchLength,
            _ => return Err(RuntimeError::new("Invalid URL operation")),
        };
        let result = match op {
            Op::StaticText => self.static_text(a)?,
            Op::StaticSet => { self.static_set(a, b)?; 0 },
            Op::DefaultPort => self.defaults.borrow().port as usize,
            Op::SetDefaultPort => { self.defaults.borrow_mut().to_mut().port = a as i32; 0 },
            Op::ConfigGet => self.config_get(a)?,
            Op::ConfigSet => { self.config_set(a, b, c, d)?; 0 },
            Op::Icmp => (unsafe { cmp_ci(a as *const u32, b as *const u32) }) as usize,
            Op::Nicmp => (unsafe { cmp_n_ci(a as *const u32, b as *const u32, c) }) as usize,
            Op::Cmp => (unsafe { hp2_core::ansi::strcmp((a as *const u32).cast(), (b as *const u32).cast()) }) as usize,
            Op::Atoi => (unsafe { atoi_ptr(a as *const u32)? }) as usize,
            Op::Len => unsafe { text_len(a as *const u32) },
            Op::Chr => unsafe { find_char(a as *const u32, b as u8 as u32) },
            Op::Char => unsafe { *(a as *const u32) as usize },
            Op::SetChar => { unsafe { *(a as *mut u32) = b as u32 }; 0 },
            Op::Field => unsafe { fstring_data(field(a as *mut FURL, b)?) },
            Op::SetField => { unsafe { self.set_field(a as *mut FURL, b, c as *const u32)?; } 0 },
            Op::Port => unsafe { (*((a as *const FURL))).port as usize },
            Op::SetPort => { unsafe { (*((a as *mut FURL))).port = b as i32 }; 0 },
            Op::Count => {
                let count = unsafe { (*((a as *const FURL))).op.num };
                if count < 0 { return Err(RuntimeError::new("Invalid URL option count")); }
                count as usize
            },
            Op::Option => unsafe { self.option(a as *mut FURL, b)? },
            Op::Push => { unsafe { self.push_option(a as *mut FURL, b as *const u32)?; } 0 },
            Op::SetOption => { unsafe { self.set_option(a as *mut FURL, b, c as *const u32)?; } 0 },
            Op::Reset => { unsafe { self.reset(a as *mut FURL)?; } 0 },
            Op::SetValid => { unsafe { (*((a as *mut FURL))).valid = b as i32 }; 0 },
            Op::Append => { unsafe { self.append(a as *mut TArrayRaw, b as *const u32)?; } 0 },
            Op::AppendPort => { unsafe { self.append_port(a as *mut TArrayRaw, b as *const FURL)?; } 0 },
            Op::LogDefault => { self.log_default(a as *const u32)?; 0 },
            Op::Internal => usize::from(unsafe { self.is_internal(a as *const FURL)? }),
            Op::AddOption => { unsafe { self.add_option(a as *mut FURL, b as *const u32)?; } 0 },
            Op::GetOption => unsafe { self.get_option(a as *const FURL, b as *const u32, c as *const u32)? },
            Op::Literal => LITERALS.get(a).ok_or_else(|| RuntimeError::new("Invalid URL literal"))?.as_ptr() as usize,
            Op::CopyField => { unsafe { self.copy_field(a as *mut FURL, b as *const FURL, c)?; } 0 },
            Op::CopyOption => { unsafe { self.copy_option(a as *mut FURL, b as *const FURL, c)?; } 0 },
            Op::FieldBool => usize::from(unsafe { text_len(fstring_data(field(a as *mut FURL, b)?) as *const u32) } != 0),
            Op::FieldLen => unsafe { text_len(fstring_data(field(a as *mut FURL, b)?) as *const u32) },
            Op::StaticLen => unsafe { text_len(self.static_text(a)? as *const u32) },
            Op::CopyStaticField => { unsafe { self.copy_static_field(a as *mut FURL, b, c)?; } 0 },
            Op::CheckBase => if a == 0 { return Err(RuntimeError::new("Null FURL base")); } else { 0 },
            Op::MatchLength => unsafe { (a as *const u32).offset_from(b as *const u32) as usize + 1 },
        };
        Ok(result)
    }

    fn static_text(&self, id: usize) -> RuntimeResult<usize> {
        let defaults = self.defaults.borrow();
        if id >= defaults.fields.len() { return Err(RuntimeError::new("Invalid URL static")); }
        Ok(defaults.text(id).as_ptr() as usize)
    }
    fn static_set(&self, id: usize, source: usize) -> RuntimeResult<()> {
        let mut defaults = self.defaults.borrow_mut();
        let Some(field) = defaults.to_mut().fields.get_mut(id) else { return Err(RuntimeError::new("Invalid URL static")); };
        if source == 0 || unsafe { *(source as *const u32) } == 0 {
            field.clear();
        } else {
            *field = unsafe { read_text(source as *const u32) };
        }
        Ok(())
    }
    fn config_get(&self, id: usize) -> RuntimeResult<usize> {
        let value = if id < 9 {
            self.access.config_get_units(URL_SECTION, DEFAULT_KEYS[id])?
        } else if id == 9 {
            self.access.config_get_units(URL_SECTION, &[80,111,114,116])?
        } else {
            return Err(RuntimeError::new("Invalid URL config key"));
        };
        let Some(value) = value.filter(|text| text.first().is_some_and(|unit| *unit != 0)) else {
            return Ok(EMPTY.as_ptr() as usize);
        };
        let mut scratch = self.scratch.borrow_mut();
        *scratch = terminated(value);
        Ok(scratch.as_ptr() as usize)
    }
    fn config_set(&self, section: usize, key: usize, value: usize, filename: usize) -> RuntimeResult<()> {
        let section = unsafe { read_text(section as *const u32) };
        let key = unsafe { read_text(key as *const u32) };
        let value = unsafe { read_text(value as *const u32) };
        let filename = if filename == 0 { None } else { Some(unsafe { read_text(filename as *const u32) }) };
        self.access.config_set_units_from(filename.as_deref(), &section, &key, &value)
    }
    unsafe fn set_field(&self, url: *mut FURL, id: usize, text: *const u32) -> RuntimeResult<()> {
        let destination = unsafe { field(url, id)? };
        let text = unsafe { read_text(text) };
        unsafe { self.access.assign_fstring(destination, &terminated(text)) }
    }
    unsafe fn option(&self, url: *mut FURL, index: usize) -> RuntimeResult<usize> {
        let array = unsafe { ptr::addr_of!((*url).op).read_unaligned() };
        if index >= array.num.max(0) as usize || array.data.is_null() { return Err(RuntimeError::new("URL option index out of range")); }
        Ok(unsafe { fstring_data(array.data.cast::<TArrayRaw>().add(index)) })
    }
    unsafe fn push_option(&self, url: *mut FURL, text: *const u32) -> RuntimeResult<()> {
        let slots = unsafe { array_slots(ptr::addr_of_mut!((*url).op)) };
        let old_num = unsafe { ptr::addr_of!((*url).op.num).read_unaligned() };
        let old_max = unsafe { ptr::addr_of!((*url).op.max).read_unaligned() };
        let index = unsafe {
            array_storage::add(
                slots,
                1,
                std::mem::size_of::<TArrayRaw>() as i32,
                ptr::null(),
                array_storage::system_allocator(),
            )
        };
        let index = match index {
            Ok(index) => index,
            Err(_) => {
                unsafe {
                    ptr::addr_of_mut!((*url).op.num).write_unaligned(old_num);
                    ptr::addr_of_mut!((*url).op.max).write_unaligned(old_max);
                }
                return Err(RuntimeError::new("Unable to grow URL options"));
            }
        };
        let item = unsafe { (*url).op.data.cast::<TArrayRaw>().add(index as usize) };
        unsafe { item.write_unaligned(TArrayRaw { data: ptr::null_mut(), num: 0, max: 0 }); }
        let value = unsafe { read_text(text) };
        unsafe { self.access.assign_fstring(item, &terminated(value)) }
    }
    unsafe fn set_option(&self, url: *mut FURL, index: usize, text: *const u32) -> RuntimeResult<()> {
        let array = unsafe { ptr::addr_of_mut!((*url).op) };
        let raw = unsafe { array.read_unaligned() };
        if index >= raw.num.max(0) as usize || raw.data.is_null() { return Err(RuntimeError::new("URL option index out of range")); }
        let value = unsafe { read_text(text) };
        unsafe { self.access.assign_fstring(raw.data.cast::<TArrayRaw>().add(index), &terminated(value)) }
    }
    unsafe fn reset(&self, url: *mut FURL) -> RuntimeResult<()> {
        let defaults = self.defaults.borrow();
        for (index, source) in [0, 5, 3, 6].into_iter().enumerate() {
            let field = unsafe { field(url, [0, 1, 2, 3][index])? };
            unsafe { self.access.assign_fstring(field, defaults.text(source))?; }
        }
        unsafe { self.access.destroy_owned_fstring_array(ptr::addr_of_mut!((*url).op))?; }
        unsafe { (*url).port = defaults.port; (*url).valid = 1; }
        Ok(())
    }
    unsafe fn append(&self, output: *mut TArrayRaw, text: *const u32) -> RuntimeResult<()> {
        let mut value = unsafe { read_fstring(output) };
        value.extend(unsafe { read_text(text) }.into_iter().take_while(|&u| u != 0));
        unsafe { self.access.assign_fstring(output, &terminated(value)) }
    }
    unsafe fn append_port(&self, output: *mut TArrayRaw, url: *const FURL) -> RuntimeResult<()> {
        let port = unsafe { (*url).port };
        let mut value = unsafe { read_fstring(output) };
        value.extend(port.to_string().bytes().map(u32::from));
        unsafe { self.access.assign_fstring(output, &terminated(value)) }
    }
    fn log_default(&self, text: *const u32) -> RuntimeResult<()> {
        let category = self.access.intern_str("Log", FindName::Add)?;
        self.access.emit_log(category, &unsafe { read_text(text) })
    }
    unsafe fn is_internal(&self, url: *const FURL) -> RuntimeResult<bool> {
        let protocol = unsafe { fstring_data(ptr::addr_of!((*url).protocol) as *mut TArrayRaw) } as *const u32;
        let defaults = self.defaults.borrow();
        let default = defaults.text(0).as_ptr();
        Ok(unsafe { hp2_core::ansi::stricmp(protocol.cast(), default.cast()) == 0 })
    }
    fn run_policy(&self, entry: u32, args: &UrlArgs) -> RuntimeResult<usize> {
        unsafe { url::run_with(self, entry, args) }.map_err(|_| {
            self.errors.borrow_mut().take().unwrap_or_else(|| RuntimeError::new("FURL policy failed"))
        })
    }
    unsafe fn add_option(&self, url: *mut FURL, text: *const u32) -> RuntimeResult<()> {
        self.run_policy(6, &UrlArgs { url: url as usize, text: text as usize, ..UrlArgs::default() })?;
        Ok(())
    }
    unsafe fn get_option(&self, url: *const FURL, match_text: *const u32, default: *const u32) -> RuntimeResult<usize> {
        self.run_policy(10, &UrlArgs {
            url: url as usize, text: match_text as usize, extra: default as usize, ..UrlArgs::default()
        })
    }
    unsafe fn copy_field(&self, to: *mut FURL, from: *const FURL, id: usize) -> RuntimeResult<()> {
        let text = unsafe { fstring_data(field(from as *mut FURL, id)?) } as *const u32;
        unsafe { self.set_field(to, id, text) }
    }
    unsafe fn copy_option(&self, to: *mut FURL, from: *const FURL, index: usize) -> RuntimeResult<()> {
        let text = unsafe { self.option(from as *mut FURL, index)? as *const u32 };
        unsafe { self.push_option(to, text) }
    }
    unsafe fn copy_static_field(&self, url: *mut FURL, field_id: usize, static_id: usize) -> RuntimeResult<()> {
        let defaults = self.defaults.borrow();
        if static_id >= defaults.fields.len() { return Err(RuntimeError::new("Invalid URL static")); }
        let destination = unsafe { field(url, field_id)? };
        unsafe { self.access.assign_fstring(destination, defaults.text(static_id)) }
    }
}

impl UrlOps for CoreUrlOps<'_> {
    unsafe fn call(&self, code: u32, a: usize, b: usize, c: usize, d: usize) -> Result<usize, i32> {
        match unsafe { self.op(code, a, b, c, d) } {
            Ok(value) => Ok(value),
            Err(error) => { *self.errors.borrow_mut() = Some(error); Err(1) }
        }
    }

    unsafe fn scope(&self, entry: u32, args: &UrlArgs) -> Result<usize, i32> {
        match unsafe { self.scope_impl(entry, args) } {
            Ok(value) => Ok(value),
            Err(error) => { *self.errors.borrow_mut() = Some(error); Err(1) }
        }
}
}

impl CoreUrlOps<'_> {
    fn resume_policy(&self, entry: u32, args: &UrlArgs, buffer: *mut u32) -> RuntimeResult<usize> {
        unsafe { url::resume_with(self, entry, args, buffer as usize) }.map_err(|_| {
            self.errors.borrow_mut().take().unwrap_or_else(|| RuntimeError::new("URL continuation failed"))
        })
    }

    unsafe fn scope_impl(&self, entry: u32, args: &UrlArgs) -> RuntimeResult<usize> {
        match entry {
            1 | 3 | 4 => {
                let source = match entry {
                    1 => args.text,
                    3 => unsafe { self.option(args.url as *mut FURL, args.other)? },
                    _ => unsafe { self.option(args.other as *mut FURL, args.text)? },
                } as *const u32;
                // Only written text and its terminal NUL are read by policy.
                let mut buffer = MaybeUninit::<[u32; 1024]>::uninit();
                let pointer = buffer.as_mut_ptr().cast::<u32>();
                let maximum = if entry == 4 { 99 } else { 1023 };
                let mut length = 0;
                while length < maximum {
                    let unit = unsafe { source.add(length).read() };
                    if unit == 0 || (entry == 4 && unit == b'=' as u32) { break; }
                    unsafe { pointer.add(length).write(unit); }
                    length += 1;
                }
                unsafe { pointer.add(length).write(0); }
                self.resume_policy(entry, args, pointer)
            }
            2 => {
                let section = unsafe { read_text(args.text as *const u32) };
                let filename = if args.extra == 0 { None } else { Some(unsafe { read_text(args.extra as *const u32) }) };
                let pairs = self.access.config_section_units(&section, filename.as_deref())?;
                let mut buffer = Vec::new();
                for (key, value) in pairs {
                    let key = &key[..key.iter().position(|&unit| unit == 0).unwrap_or(key.len())];
                    // Preserve GConfig::GetSection's key-only size predicate.
                    if buffer.len() + key.len() + 1 >= 32767 { break; }
                    buffer.extend_from_slice(key);
                    buffer.push(b'=' as u32);
                    buffer.extend(value.into_iter().take_while(|&unit| unit != 0));
                    buffer.push(0);
                }
                buffer.push(0);
                self.resume_policy(entry, args, buffer.as_mut_ptr())
            }
            _ => Err(RuntimeError::new("Invalid URL scope")),
        }
    }
}

const LITERALS: [&[u32]; 26] = [
    &[0], &[80,114,111,116,111,99,111,108,0], &[80,114,111,116,111,99,111,108,68,101,115,99,114,105,112,116,105,111,110,0],
    &[78,97,109,101,0], &[77,97,112,0], &[76,111,99,97,108,77,97,112,0], &[72,111,115,116,0], &[80,111,114,116,97,108,0],
    &[77,97,112,69,120,116,0], &[83,97,118,101,69,120,116,0], &[58,0], &[47,47,0], &[47,0], &[63,0], &[80,85,83,72,0],
    &[80,79,80,0], &[80,69,69,82,0], &[76,79,65,68,0], &[78,97,109,101,61,0], &[84,101,97,109,61,0], &[67,108,97,115,115,61,0],
    &[83,107,105,110,61,0], &[70,97,99,101,61,0], &[86,111,105,99,101,61,0], &[79,118,101,114,114,105,100,101,67,108,97,115,115,61,0], &[35,0],
];

unsafe fn field(url: *mut FURL, id: usize) -> RuntimeResult<*mut TArrayRaw> {
    if url.is_null() { return Err(RuntimeError::new("Null FURL")); }
    Ok(unsafe { match id { 0 => ptr::addr_of_mut!((*url).protocol), 1 => ptr::addr_of_mut!((*url).host), 2 => ptr::addr_of_mut!((*url).map), 3 => ptr::addr_of_mut!((*url).portal), _ => return Err(RuntimeError::new("Invalid FURL field")) } })
}
unsafe fn fstring_data(value: *mut TArrayRaw) -> usize {
    let raw = unsafe { value.read_unaligned() };
    if raw.num == 0 { EMPTY.as_ptr() as usize } else { raw.data as usize }
}
unsafe fn read_fstring(value: *mut TArrayRaw) -> Vec<u32> {
    let raw = unsafe { value.read_unaligned() };
    if raw.num <= 1 || raw.data.is_null() { Vec::new() }
    else { unsafe { std::slice::from_raw_parts(raw.data.cast::<u32>(), raw.num as usize - 1) }.to_vec() }
}
unsafe fn read_text(value: *const u32) -> Vec<u32> {
    if value.is_null() { return vec![0]; }
    let mut result = Vec::new();
    let mut cursor = value;
    loop {
        let unit = unsafe { cursor.read() };
        result.push(unit);
        if unit == 0 { return result; }
        cursor = unsafe { cursor.add(1) };
    }
}
fn terminated(mut value: Vec<u32>) -> Vec<u32> {
    value.truncate(value.iter().position(|&u| u == 0).unwrap_or(value.len()));
    value.push(0);
    value
}
unsafe fn text_len(value: *const u32) -> usize {
    if value.is_null() { return 0; }
    let mut size = 0;
    while unsafe { value.add(size).read() } != 0 { size += 1; }
    size
}
unsafe fn find_char(value: *const u32, ch: u32) -> usize {
    if value.is_null() { return 0; }
    let mut at = value;
    loop {
        let unit = unsafe { at.read() };
        if unit == ch { return at as usize; }
        if unit == 0 { return 0; }
        at = unsafe { at.add(1) };
    }
}
unsafe fn cmp_ci(a: *const u32, b: *const u32) -> i32 {
    unsafe { hp2_core::ansi::stricmp(a.cast(), b.cast()) }
}
unsafe fn cmp_ci_n(a: *const u32, b: *const u32, n: usize) -> i32 {
    unsafe { hp2_core::ansi::strnicmp(a.cast(), b.cast(), i32::try_from(n).unwrap_or(i32::MAX)) }
}
unsafe fn cmp_n_ci(a: *const u32, b: *const u32, n: usize) -> i32 {
    unsafe { cmp_ci_n(a, b, n) }
}
unsafe fn atoi_ptr(value: *const u32) -> RuntimeResult<i32> {
    unsafe { hp2_core::ansi::atoi_checked(value.cast()) }
        .map_err(|_| RuntimeError::new("URL integer is outside the native INT range"))
}
unsafe fn array_slots(array: *mut TArrayRaw) -> Hp2ArraySlots {
    Hp2ArraySlots { data: unsafe { ptr::addr_of_mut!((*array).data) }, num: unsafe { ptr::addr_of_mut!((*array).num) }, max: unsafe { ptr::addr_of_mut!((*array).max) } }
}
static EMPTY: [u32; 1] = [0];

/// Copies the counted native FString storage, preserving an empty zero-count header.
///
/// # Safety
/// Source and destination are disjoint initialized FString headers with live storage.
unsafe fn copy_string(destination: *mut TArrayRaw, source: *const TArrayRaw) -> RuntimeResult<()> {
    let source = unsafe { source.read_unaligned() };
    if source.num < 0 || source.max < source.num || (source.num > 0 && source.data.is_null()) {
        return Err(RuntimeError::new("Invalid FString in FURL copy"));
    }
    unsafe {
        array_storage::set(array_slots(destination), source.num,
            std::mem::size_of::<u32>() as i32, ptr::null(),
            array_storage::system_allocator())
            .map_err(|_| RuntimeError::new("Unable to copy FURL FString"))?;
        if source.num > 0 {
            ptr::copy_nonoverlapping(source.data.cast::<u32>(),
                (*destination).data.cast::<u32>(), source.num as usize);
        }
    }
    Ok(())
}
