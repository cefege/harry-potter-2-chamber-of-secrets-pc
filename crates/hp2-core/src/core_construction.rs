//! Canonical Rust construction, typed property lifetime, and class registration.

use crate::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use crate::core_class_specs::NativeRecordKind;
use crate::object_construction::{self, Hp2ObjectAllocationRequest};
use crate::object_owner::{ClassData, OwnedClassRecord, OwnedObject};
use crate::property_value::Hp2ValueOperation;
use hp2_abi::engine::generated::{UClass, UObject, UProperty, UStruct};
use hp2_abi::render_span::FMemMark;
use std::alloc::Layout;
use std::ffi::c_void;
use std::ptr;

/// A class-constructed UObject in the Core scratch arena, outside the object table.
///
/// The lifetime is bounded by a GMem mark. Call [`finish`](Self::finish) to
/// observe cleanup errors; dropping is a best-effort fallback.
pub struct TemporaryObject<'a> {
    access: CoreAccess<'a>,
    class: *mut UClass,
    object: *mut UObject,
    mark: FMemMark,
    active: bool,
    initialized: bool,
}


impl<'a> CoreAccess<'a> {
    /// Creates an unregistered, class-sized temporary through canonical class
    /// initialization and construction. Its storage remains live until finish/drop.
    ///
    /// # Safety
    /// `class` must be a live canonical class whose constructor and reflected
    /// property links remain valid for the temporary's lifetime.
    pub unsafe fn temporary_object(self, class: *mut UClass) -> RuntimeResult<TemporaryObject<'a>> {
        if class.is_null() {
            return Err(RuntimeError::new("Cannot construct a temporary without a class"));
        }
        let metadata = unsafe { self.class_metadata(class)? };
        let (size, alignment) = unsafe {
            ((*class).properties_size, (*class.cast::<UStruct>()).properties_align)
        };
        if size < std::mem::size_of::<UObject>() as i32
            || alignment <= 0
            || !(alignment as u32).is_power_of_two()
        {
            return Err(RuntimeError::new("Invalid temporary object layout"));
        }
        // Source New<BYTE>(GMem, ...) uses DEFAULT_ALIGNMENT=16.
        let alignment = alignment.max(16);
        Layout::from_size_align(size as usize, alignment as usize)
            .map_err(|_| RuntimeError::new("Invalid temporary object layout"))?;

        let memory = self.scratch_memory();
        let mark = unsafe { memory.mark() };
        let object = match unsafe { memory.push_bytes(size, alignment) } {
            Ok(storage) => storage.cast::<UObject>(),
            Err(error) => {
                unsafe { memory.pop(&mark) };
                return Err(error);
            }
        };
        let mut temporary = TemporaryObject {
            access: self,
            class,
            object,
            mark,
            active: true,
            initialized: true,
        };

        let initialize = unsafe {
            crate::object_properties::init_class_default_object_with(
                object.cast(),
                class.cast(),
                |object, class| {
                    crate::object_bootstrap::initialize_header(
                        object,
                        class,
                        ptr::null_mut(),
                        0,
                        0,
                        crate::object_bootstrap::Hp2ObjectHeaderKind::ClassDefault,
                    );
                    Ok::<(), RuntimeError>(())
                },
                |class| (*class.cast::<UClass>()).properties_size,
                |class| (*class.cast::<UClass>()).super_field,
                |data, size, superclass| {
                    self.init_properties(data, size, superclass.cast(), ptr::null(), 0)
                },
            )
        };
        if let Err(error) = initialize {
            let _ = temporary.finish();
            return Err(error);
        }



        // Copy operation metadata before invoking constructor code: callbacks
        // may reenter Core and mutate records, but no class-record borrow survives.
        if let Err(error) = unsafe { (metadata.ops.construct)(self, object) } {
            let _ = temporary.finish();
            return Err(error);
        }
        Ok(temporary)
    }
}

impl TemporaryObject<'_> {
    /// Returns the live temporary object pointer.
    pub fn as_ptr(&self) -> *mut UObject {
        self.object
    }

    /// Runs temporary-only property/base cleanup and restores its GMem mark.
    /// No object-table, root, logging, or conditional-destroy operation occurs.
    pub fn finish(mut self) -> RuntimeResult<()> {
        let result = self.cleanup();
        if self.active {
            unsafe { self.access.scratch_memory().pop(&self.mark) };
            self.active = false;
        }
        result
    }

    fn cleanup(&mut self) -> RuntimeResult<()> {
        if self.object.is_null() {
            return Ok(());
        }
        let mut first_error = None;
        if self.initialized {
            if let Err(error) = unsafe { self.access.exit_properties(self.object.cast(), self.class) } {
                first_error = Some(error);
            }
        }
        let state_frame = unsafe { ptr::addr_of_mut!((*self.object).state_frame).read() };
        if !state_frame.is_null() {
            unsafe { crate::vm_frame::free_state(state_frame.cast()) };
            unsafe { ptr::addr_of_mut!((*self.object).state_frame).write(ptr::null_mut()) };
        }
        self.object = ptr::null_mut();
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl Drop for TemporaryObject<'_> {
    fn drop(&mut self) {
        let _ = self.cleanup();
        if self.active {
            unsafe { self.access.scratch_memory().pop(&self.mark) };
            self.active = false;
        }
    }
}

impl<'a> CoreAccess<'a> {
    /// Initializes reflected storage using byte defaults for scalar data and typed
    /// property copies for every dynamic field in the constructor chain.
    ///
    /// # Safety
    /// `data` spans `size`; optional defaults span their byte count. The defaults
    /// class and its linked property records, when present, are live.
    pub(crate) unsafe fn init_properties(
        self,
        data: *mut u8,
        size: i32,
        class: *mut UClass,
        defaults: *const u8,
        defaults_count: i32,
    ) -> RuntimeResult<()> {
        if data.is_null() || defaults_count < 0 || defaults_count > size {
            return Err(RuntimeError::new("Invalid property initialization storage"));
        }
        let (defaults, defaults_count) = if defaults.is_null() && !class.is_null() {
            unsafe { ((*class).defaults.data.cast::<u8>().cast_const(), (*class).defaults.num) }
        } else {
            (defaults, defaults_count)
        };
        let header_size = std::mem::size_of::<UObject>() as i32;
        if size < header_size || defaults_count > size
            || (!defaults.is_null() && defaults_count < header_size)
        {
            return Err(RuntimeError::new("Invalid property initialization size"));
        }
        unsafe {
            crate::object_properties::init_typed_properties(
                data,
                size,
                defaults,
                defaults_count,
                std::mem::size_of::<UObject>() as i32,
                || if class.is_null() { ptr::null_mut() } else { (*class.cast::<UStruct>()).constructor_link.cast() },
                |property| (*property.cast::<UProperty>()).offset,
                |property| {
                    let property = property.cast::<UProperty>();
                    (*property).element_size.saturating_mul((*property).array_dim)
                },
                |property| (*property.cast::<UProperty>()).constructor_link_next.cast(),
                |property, destination, source| {
                    self.property_value(
                        Hp2ValueOperation::ValueCopyComplete,
                        property.cast(),
                        destination,
                        source,
                        0,
                        None,
                    ).map(|_| ())
                },
                RuntimeError::new,
            )
        }
    }

    /// Destroys each constructed property value in reflected constructor order.
    ///
    /// # Safety
    /// `data` spans the complete properties storage; `class` and its constructor
    /// links remain live until the traversal finishes.
    pub(crate) unsafe fn exit_properties(
        self,
        data: *mut u8,
        class: *mut UClass,
    ) -> RuntimeResult<()> {
        if data.is_null() || class.is_null() {
            return Err(RuntimeError::new("Invalid property destruction storage"));
        }
        let first_error = std::cell::RefCell::new(None);
        unsafe {
            crate::object_properties::exit_typed_properties(
                data,
                || (*class.cast::<UStruct>()).constructor_link.cast(),
                |property| (*property.cast::<UProperty>()).offset,
                |property| (*property.cast::<UProperty>()).constructor_link_next.cast(),
                |property, slot| {
                    let offset = (*property.cast::<UProperty>()).offset;
                    if offset < 0 || offset >= (*class).properties_size {
                        if first_error.borrow().is_none() {
                            *first_error.borrow_mut() =
                                Some(RuntimeError::new("Invalid reflected property offset"));
                        }
                    } else if offset != 0 {
                        if let Err(error) = self.property_value(
                            Hp2ValueOperation::ValueDestroy,
                            property.cast(),
                            slot,
                            ptr::null_mut(),
                            0,
                            None,
                        ) {
                            if first_error.borrow().is_none() {
                                *first_error.borrow_mut() = Some(error);
                            }
                        }
                    }
                    Ok(())
                },
                RuntimeError::new,
            )?;
        }
        match first_error.into_inner() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    /// Constructs and publishes a canonical Rust-owned object through shared
    /// allocation and constructor policy.
    ///
    /// # Safety
    /// Class, outer, and template pointers must remain canonical across callbacks.
    pub unsafe fn construct_object(
        self,
        class: *mut UClass,
        outer: *mut UObject,
        name: i32,
        flags: u32,
        template: *mut UObject,
    ) -> RuntimeResult<*mut UObject> {
        unsafe {
            self.construct_object_internal(
                class, outer, name, flags, template,
                None::<fn(CoreAccess<'a>, *mut UObject) -> RuntimeResult<()>>,
            )
        }
    }

    /// Constructs an object with a source-specific constructor instead of the class thunk.
    ///
    /// Allocation, typed defaults, publication, and rollback remain the same as
    /// [`construct_object`]; the supplied constructor replaces only the final
    /// native class constructor invocation.
    ///
    /// # Safety
    /// Class, outer, and template remain canonical; the constructor initializes
    /// all native members exactly once and does not unwind across this call.
    pub unsafe fn construct_object_with_constructor<F>(
        self,
        class: *mut UClass,
        outer: *mut UObject,
        name: i32,
        flags: u32,
        template: *mut UObject,
        constructor: F,
    ) -> RuntimeResult<*mut UObject>
    where
        F: FnOnce(CoreAccess<'a>, *mut UObject) -> RuntimeResult<()>,
    {
        unsafe { self.construct_object_internal(class, outer, name, flags, template, Some(constructor)) }
    }

    unsafe fn construct_object_internal<F>(
        self,
        class: *mut UClass,
        outer: *mut UObject,
        name: i32,
        flags: u32,
        template: *mut UObject,
        constructor: Option<F>,
    ) -> RuntimeResult<*mut UObject>
    where
        F: FnOnce(CoreAccess<'a>, *mut UObject) -> RuntimeResult<()>,
    {
        if self.object_creation_closed() {
            return Err(RuntimeError::new("Cannot construct Core objects after pre-exit begins"));
        }
        if class.is_null() {
            return Err(RuntimeError::new("Cannot construct an object without a class"));
        }
        let request = Hp2ObjectAllocationRequest {
            in_class: class.cast(), outer: outer.cast(), in_template: template.cast(),
            supplied: ptr::null_mut(), flags,
        };
        let ops = DirectConstruction {
            access: self, class, name: std::cell::Cell::new(name), outer, flags,
            staged: std::cell::RefCell::new(None),
            constructor: std::cell::RefCell::new(constructor),
        };
        unsafe {
            object_construction::construct_with_tag_ops(&ops, &request, |ops, request, next| {
                next(ops, request)
            })
        }.map(|object| object.cast())
    }

    /// Resets a class's typed default storage before its registration pass.
    ///
    /// # Safety
    /// `class` identifies a live Rust-owned class record.
    pub(crate) unsafe fn reset_class_defaults(self, class: *mut UClass) -> RuntimeResult<()> {
        if class.is_null() { return Err(RuntimeError::new("Invalid class defaults")); }
        let defaults = unsafe { ptr::addr_of_mut!((*class).defaults) };
        let data = unsafe { (*defaults).data.cast::<u8>() };
        let count = unsafe { (*defaults).num };
        let mut first_error = None;
        if !data.is_null() && count > 0 {
            if let Err(error) = unsafe { self.exit_properties(data, class) } { first_error = Some(error); }
        }
        unsafe { crate::array_storage::release_owned(
            ptr::addr_of_mut!((*defaults).data),
            ptr::addr_of_mut!((*defaults).num),
            ptr::addr_of_mut!((*defaults).max),
        ) };
        if let Some(error) = first_error { return Err(error); }
        let size = unsafe { (*class).properties_size };
        if size < std::mem::size_of::<UObject>() as i32 {
            return Err(RuntimeError::new("Invalid class defaults size"));
        }
        let layout = Layout::array::<u8>(size as usize)
            .map_err(|_| RuntimeError::new("Invalid class defaults size"))?;
        let allocation = unsafe { crate::unix_alloc::system_malloc_layout(layout) };
        unsafe {
            (*defaults).data = allocation.cast();
            (*defaults).num = size;
            (*defaults).max = size;
        }
        Ok(())
    }

    /// Returns the class-owned defaults buffer; it has no object-table entry.
    ///
    /// # Safety
    /// `class` is a live class whose defaults have been reset and initialized.
    pub unsafe fn default_object(self, class: *mut UClass) -> RuntimeResult<*mut UObject> {
        if class.is_null() { return Err(RuntimeError::new("Invalid class default object")); }
        let defaults = unsafe { &(*class).defaults };
        if defaults.num != unsafe { (*class).properties_size } || defaults.data.is_null() {
            return Err(RuntimeError::new("Invalid class default storage"));
        }
        Ok(defaults.data.cast())
    }

    /// Initializes the class-owned buffer from superclass defaults.
    ///
    /// # Safety
    /// The class owns a properties-sized buffer allocated by reset_class_defaults.
    pub(crate) unsafe fn initialize_default(self, class: *mut UClass) -> RuntimeResult<()> {
        let default = unsafe { self.default_object(class)? };
        unsafe {
            crate::object_properties::init_class_default_object_with(
                default.cast(),
                class.cast(),
                |object, class| {
                    // The native vptr slot is inert in the Rust-owned record.
                    object.cast::<*mut c_void>().write(ptr::null_mut());
                    crate::object_bootstrap::initialize_header(
                        object, class, ptr::null_mut(), 0, 0,
                        crate::object_bootstrap::Hp2ObjectHeaderKind::ClassDefault,
                    );
                    Ok(())
                },
                |class| (*class.cast::<UClass>()).properties_size,
                |class| (*class.cast::<UClass>()).super_field,
                |data, size, superclass| {
                    self.init_properties(data, size, superclass.cast(), ptr::null(), 0)
                },
            )
        }
    }
    /// Registers class metadata and its defaults through the shared Core policy.
    ///
    /// # Safety
    /// `class` is live canonical state whose registration hooks may reenter Core.
    pub(crate) unsafe fn register_class(self, class: *mut UClass) -> RuntimeResult<()> {
        if class.is_null() { return Err(RuntimeError::new("Invalid class registration")); }
        let ops = DirectClassRegistration { access: self };
        unsafe {
            crate::class_metadata::register_class_with(
                class.cast(),
                CLASS_INHERIT_MASK,
                &ops,
            )
        }
    }

    ///
    /// # Safety
    /// Both records are live canonical class allocations; the class tail is initialized.
    pub(crate) unsafe fn inherit_class_metadata(
        self,
        class: *mut UClass,
        superclass: *mut UClass,
    ) -> RuntimeResult<()> {
        let current = unsafe { self.class_metadata(class)? };
        if let Some(spec) = current.native_spec {
            let object = class.cast::<UObject>();
            let outer = unsafe { (*object).outer.cast::<UObject>() };
            if !outer.is_null()
                && i32::from_ne_bytes(unsafe { (*object).name })
                    == self.intern_str(spec.short_name, crate::name_owner::FindName::Find)?
                && i32::from_ne_bytes(unsafe { (*outer).name })
                    == self.intern_str(spec.package, crate::name_owner::FindName::Find)?
            {
                return Ok(());
            }
        }
        let parent = unsafe { self.class_metadata(superclass)? };
        unsafe { OwnedClassRecord::replace_metadata(class.cast(), parent) };
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct DirectReplacementSnapshot {
    class: *mut UClass,
    class_state: Option<(*mut c_void, u32, *mut c_void, i32, i32)>,
    class_data: Option<ClassData>,
    property: *mut UProperty,
    property_state: Option<(i32, i32)>,
    bool_property: *mut hp2_abi::engine::generated::UBoolProperty,
    bit_mask: Option<u32>,
}
struct DirectConstruction<'a, F> {
    access: CoreAccess<'a>,
    class: *mut UClass,
    name: std::cell::Cell<i32>,
    outer: *mut UObject,
    flags: u32,
    staged: std::cell::RefCell<Option<OwnedObject>>,
    constructor: std::cell::RefCell<Option<F>>,
}

impl<'a, F> object_construction::ObjectAllocationOps for DirectConstruction<'a, F>
where
    F: FnOnce(CoreAccess<'a>, *mut UObject) -> RuntimeResult<()>,
{
    type Error = RuntimeError;
    type ReplacementSnapshot = DirectReplacementSnapshot;
    fn assert_error(&self) -> RuntimeResult<()> { Ok(()) }
    fn assert_within(&self, class: *mut c_void) -> RuntimeResult<()> {
        if !class.is_null() && unsafe { (*class.cast::<UClass>()).class_within }.is_null() {
            Err(RuntimeError::new("Class has no ClassWithin"))
        } else { Ok(()) }
    }
    fn assert_constructor(&self, class: *mut c_void) -> RuntimeResult<()> {
        if !class.is_null() {
            let _ = unsafe { self.access.class_metadata(class.cast())? };
        }
        Ok(())
    }
    fn register_count(&self) -> i32 { self.access.object_count() }
    fn package_class(&self) -> RuntimeResult<*mut c_void> {
        Ok(self.access.native_class("Package")?.cast())
    }
    fn validation_error(&self, code: i32, class: *mut c_void, _outer: *mut c_void) -> RuntimeResult<()> {
        let mut error = RuntimeError::new(match code {
            0 => "Cannot construct object without class", 1 => "Class is not registered",
            2 => "Cannot construct abstract class", 3 => "Class has no outer package",
            _ => "Outer object is outside class within constraint",
        });
        if code == 3 {
            let name = i32::from_ne_bytes(unsafe { (*class.cast::<UObject>()).name });
            error = error.with_context(&crate::core_loader::native_name_for(&self.access.name_text(name)?));
        }
        Err(error)
    }
    fn name(&self) -> i32 { self.name.get() }
    fn unique_name(&self, class: *mut c_void, outer: *mut c_void) -> RuntimeResult<()> {
        use crate::object_identity::Hp2IdentityLeaf as L;
        let mut base = Vec::new();
        let mut candidate = [0_u32; crate::name::NAME_SIZE];
        let mut candidate_len = 0;
        let name = crate::object_identity::unique(class as usize, outer as usize, |op, a, b| {
            match op {
                L::CheckClass => {
                    if a == 0 { return Err(RuntimeError::new("Unique object name requires a class")); }
                    Ok(0)
                }
                L::CopyBase => {
                    let index = i32::from_ne_bytes(unsafe { (*(a as *mut UObject)).name });
                    self.access.write_name_text(index, &mut base)?;
                    Ok(0)
                }
                L::BaseLength => Ok(base.len()),
                L::BaseDigit => Ok(usize::from((u32::from(b'0')..=u32::from(b'9')).contains(&base[a]))),
                L::TerminateBase => { base.truncate(a); Ok(0) }
                L::Counter => Ok(unsafe { (*(a as *mut UClass)).class_unique } as usize),
                L::SetCounter => { unsafe { (*(a as *mut UClass)).class_unique = b as i32; } Ok(0) }
                L::FormatUnique => {
                    let mut number = [std::mem::MaybeUninit::uninit(); 512];
                    crate::ansi::format_int(a as i32, &mut number);
                    let digits = number.iter().position(|unit| unsafe { unit.assume_init() } == 0)
                        .ok_or_else(|| RuntimeError::new("Unique object number is not terminated"))?;
                    // appStrncpy(Candidate, Base, NAME_SIZE - NumberLength - 1)
                    // reserves its terminator before appStrcat appends the number.
                    let prefix = base.len().min(crate::name::NAME_SIZE - digits - 2);
                    candidate[..prefix].copy_from_slice(&base[..prefix]);
                    for (target, unit) in candidate[prefix..].iter_mut().zip(&number[..digits]) {
                        *target = unsafe { unit.assume_init() } as u32;
                    }
                    candidate_len = prefix + digits;
                    Ok(0)
                }
                L::FindUnique => {
                    let name = self.access.intern(&candidate[..candidate_len], crate::name_owner::FindName::Find)?;
                    if name == 0 { return Ok(0); }
                    Ok(unsafe { self.access.find_object(ptr::null_mut(), a as *mut UObject, name, true) } as usize)
                }
                L::InternUnique => self.access.intern(&candidate[..candidate_len], crate::name_owner::FindName::Add)
                    .map(|name| name as usize),
                _ => Err(RuntimeError::new("Unexpected unique-name identity operation")),
            }
        })?;
        self.name.set(name as i32);
        Ok(())
    }
    fn check_conflicts(&self) -> bool { self.access.check_name_conflicts() }
    fn hash_head(&self, outer: *mut c_void) -> RuntimeResult<*mut c_void> {
        let outer_index = if outer.is_null() { 0 } else { unsafe { (*outer.cast::<UObject>()).index } };
        let bucket = crate::object_storage::hash_bucket(self.name(), outer_index);
        Ok(unsafe { (*self.access.storage()).hash_head(bucket) })
    }
    fn conflict(&self, object: *mut c_void, class: *mut c_void) -> RuntimeResult<()> {
        // Native allocation logs cross-class names; exact-class lookup below
        // still decides replacement. Palette/Texture pairs legitimately share names.
        let object_name = unsafe { self.access.object_full_name(object.cast())? };
        let class_name = self.access.name_text(i32::from_ne_bytes(unsafe {
            (*class.cast::<UObject>()).name
        }))?;
        let mut message = Vec::with_capacity(13 + object_name.len() + class_name.len());
        message.extend("CONFLICT: ".bytes().map(u32::from));
        message.extend(object_name);
        message.extend(" - ".bytes().map(u32::from));
        message.extend(class_name);
        let category = self.access.intern_str("Log", crate::name_owner::FindName::Add)?;
        self.access.emit_log(category, &message)
    }
    fn select_tag(&self, _class: *mut c_void, _registered: bool) -> RuntimeResult<()> { Ok(()) }
    fn find(&self, class: *mut c_void, outer: *mut c_void) -> RuntimeResult<*mut c_void> {
        Ok(unsafe { self.access.find_object(class.cast(), outer.cast(), self.name(), true) }.cast())
    }
    fn cast(&self, object: *mut c_void, kind: i32) -> RuntimeResult<*mut c_void> {
        if object.is_null() { return Ok(ptr::null_mut()); }
        let target = match kind {
            0 => "Class", 1 => "Property", 2 => "BoolProperty",
            _ => return Err(RuntimeError::new("Invalid construction cast kind")),
        };
        let class = self.access.native_class(target)?;
        // SAFETY: `object` is a canonical published object (or null), and
        // `class` is the canonical registered class returned above.
        Ok(if unsafe { crate::object_model::is_a(object, class.cast()) } {
            object
        } else {
            ptr::null_mut()
        })
    }
    fn assert_replacement(&self, supplied: *mut c_void, object: *mut c_void) -> RuntimeResult<()> {
        if supplied.is_null() || supplied == object { Ok(()) }
        else { Err(RuntimeError::new("Replacement object mismatch")) }
    }
    fn replacing(&self, object: *mut c_void) -> RuntimeResult<()> {
        let index = unsafe { (*object.cast::<UObject>()).index };
        if unsafe { (*self.access.storage()).is_owned(index) } { Ok(()) }
        else { Err(RuntimeError::new("Cannot replace a legacy object on the direct route")) }
    }
    fn no_replace(&self, _object: *mut c_void, _class: *mut c_void) -> RuntimeResult<()> {
        Err(RuntimeError::new("Cannot replace object with a different class"))
    }
    fn snapshot_replacement(&self, class: *mut c_void, property: *mut c_void, bool_property: *mut c_void) -> RuntimeResult<DirectReplacementSnapshot> {
        let class = class.cast::<UClass>();
        let class_state = if class.is_null() { None } else {
            Some((
                unsafe { (*class).class_within },
                unsafe { (*class).class_flags & 1 },
                unsafe { (*class).class_constructor },
                unsafe { (*class).properties_size },
                unsafe { (*class.cast::<UStruct>()).properties_align },
            ))
        };
        let class_data = if class.is_null() {
            None
        } else {
            Some(unsafe { self.access.class_metadata(class)? })
        };
        let property = property.cast::<UProperty>();
        let property_state = if property.is_null() { None } else {
            Some((unsafe { (*property).offset }, unsafe { (*property).cpp_property }))
        };
        let bool_property = bool_property.cast::<hp2_abi::engine::generated::UBoolProperty>();
        let bit_mask = if bool_property.is_null() { None } else { Some(unsafe { (*bool_property).bit_mask }) };
        Ok(DirectReplacementSnapshot { class, class_state, class_data, property, property_state, bool_property, bit_mask })
    }
    fn restore_replacement(&self, snapshot: DirectReplacementSnapshot) {
        unsafe {
            if let (Some(state), false) = (snapshot.class_state, snapshot.class.is_null()) {
                (*snapshot.class).class_within = state.0;
                (*snapshot.class).class_flags |= state.1;
                (*snapshot.class).class_constructor = state.2;
                (*snapshot.class).properties_size = state.3;
                (*snapshot.class.cast::<UStruct>()).properties_align = state.4;
            }
            if !snapshot.property.is_null() {
                if let Some(state) = snapshot.property_state {
                    (*snapshot.property).offset = state.0;
                    (*snapshot.property).cpp_property = state.1;
                }
                if let Some(mask) = snapshot.bit_mask {
                    (*snapshot.bool_property).bit_mask = mask;
                }
            }
            if !snapshot.class.is_null() {
                if let Some(data) = snapshot.class_data {
                    OwnedClassRecord::replace_metadata(snapshot.class.cast(), data);
                }
            }
        }
    }
    fn allocate_record(&self, properties_size: i32) -> RuntimeResult<*mut c_void> {
        if properties_size < 0 { return Err(RuntimeError::new("Invalid object properties size")); }
        let metadata = unsafe { self.access.class_metadata(self.class)? };
        let mut layout = match metadata.kind {
            NativeRecordKind::Class => Layout::new::<OwnedClassRecord>(),
            NativeRecordKind::Function => Layout::new::<crate::object_owner::OwnedFunctionRecord>(),
            NativeRecordKind::LinkerLoad => crate::core_loader::allocation_layout(),
            NativeRecordKind::LinkerSave => crate::core_save::allocation_layout(),
            kind => metadata.native_spec.and_then(|spec| spec.layout).unwrap_or_else(|| kind.layout()),
        };
        layout = Layout::from_size_align(
            layout.size().max(properties_size as usize),
            layout.align().max(std::mem::align_of::<UObject>()),
        ).map_err(|_| RuntimeError::new("Invalid object allocation layout"))?;
        let owner = if let Some(owner) = self.staged.borrow_mut().take() {
            owner
        } else {
            OwnedObject::allocate_zeroed(layout)?
        };
        let object = owner.as_ptr();
        if metadata.kind == NativeRecordKind::Class {
            unsafe { OwnedClassRecord::initialize(object.cast(), metadata) };
        } else if metadata.kind == NativeRecordKind::Function {
            unsafe { crate::object_owner::OwnedFunctionRecord::bind(object.cast(), None) };
        }
        *self.staged.borrow_mut() = Some(owner);
        Ok(object.cast())
    }
    fn destroy(&self, object: *mut c_void) -> RuntimeResult<()> {
        let index = unsafe { (*object.cast::<UObject>()).index };
        // Replacement retains the allocation, but still reaches the native
        // final-member phase before removing the old identity.
        let mut first_error = unsafe { self.access.destroy_slot(index) }.err();
        if let Err(error) = unsafe { self.access.finalize_slot(index) } {
            first_error.get_or_insert(error);
        }
        let frame = unsafe { (*object.cast::<UObject>()).state_frame };
        unsafe {
            (*object.cast::<UObject>()).state_frame = std::ptr::null_mut();
            crate::vm_frame::free_state(frame.cast());
        }
        let owner = unsafe { crate::object_storage::take_owned_for_replacement(self.access.storage(), index)? };
        *self.staged.borrow_mut() = Some(owner);
        match first_error.take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
    fn reserve(&self, index: i32) -> RuntimeResult<()> {
        unsafe { crate::object_storage::reserve_owned_replacement(self.access.storage(), index) }
    }
    fn init_properties(&self, object: *mut c_void, class: *mut c_void, template: *mut c_void) -> RuntimeResult<()> {
        let class = class.cast::<UClass>();
        let (defaults, count) = if !template.is_null() {
            (template.cast::<u8>().cast_const(), unsafe { (*class).properties_size })
        } else {
            let defaults = unsafe { (*class).defaults.data.cast::<u8>() };
            let count = unsafe { (*class).defaults.num };
            (defaults.cast_const(), count)
        };
        match unsafe { self.access.init_properties(object.cast(), (*class).properties_size, class, defaults, count) } {
            Ok(()) => Ok(()),
            Err(error) => {
                let _ = unsafe { self.access.exit_properties(object.cast(), class) };
                Err(error)
            }
        }
    }
    fn add(&self, _object: *mut c_void, index: i32) -> RuntimeResult<()> {
        let owner = self.staged.borrow_mut().take()
            .ok_or_else(|| RuntimeError::new("Missing staged object allocation"))?;
        unsafe { crate::object_storage::add_owned(self.access.storage(), owner, index)?; }
        Ok(())
    }
    fn assert_valid(&self, _object: *mut c_void) -> RuntimeResult<()> { Ok(()) }
    fn load_config(&self, object: *mut c_void) -> RuntimeResult<()> {
        // SAFETY: the generic constructor invokes this only for a live, published
        // canonical object whose class has configuration metadata.
        unsafe { self.access.load_config(object.cast()) }
    }
    fn load_localized(&self, object: *mut c_void) -> RuntimeResult<()> {
        // SAFETY: the generic constructor invokes this only for a live, published
        // canonical object whose class has localization metadata.
        unsafe { self.access.load_localized(object.cast()) }
    }
}

impl<'a, F> object_construction::ObjectConstructionOps for DirectConstruction<'a, F>
where
    F: FnOnce(CoreAccess<'a>, *mut UObject) -> RuntimeResult<()>,
{
    type Error = RuntimeError;
    type Constructor = usize;

    fn assert_error(&self) -> RuntimeResult<()> { Ok(()) }
    unsafe fn allocate(&self, request: &Hp2ObjectAllocationRequest) -> RuntimeResult<*mut c_void> {
        unsafe {
            object_construction::allocate_with_ops(self, request, |ops, request, next| {
                next(ops, request)
            })
        }
    }
    fn constructor_snapshot(&self, class: *mut c_void) -> RuntimeResult<usize> {
        Ok(unsafe { self.access.class_metadata(class.cast())? }.constructor_identity)
    }
    unsafe fn invoke_constructor(&self, constructor: usize, object: *mut c_void) -> RuntimeResult<()> {
        let class = unsafe { (*object.cast::<UObject>()).class.cast::<UClass>() };
        let metadata = unsafe { self.access.class_metadata(class)? };
        if metadata.constructor_identity != constructor {
            return Err(RuntimeError::new("Class constructor changed during allocation"));
        }
        if let Some(construct) = self.constructor.borrow_mut().take() {
            construct(self.access, object.cast())
        } else {
            unsafe { (metadata.ops.construct)(self.access, object.cast()) }
        }
    }
}

const CLASS_INHERIT_MASK: u32 = 0x0000_04ec;

struct DirectClassRegistration<'a> {
    access: CoreAccess<'a>,
}

unsafe impl crate::class_metadata::ClassRegistrationOps for DirectClassRegistration<'_> {
    type Error = RuntimeError;

    unsafe fn super_register(&self, class: *mut c_void) -> RuntimeResult<()> {
        let parent = unsafe { (*class.cast::<UClass>()).super_field.cast::<UClass>() };
        if !parent.is_null() && unsafe { (*parent).defaults.data }.is_null() {
            unsafe { self.access.register_class(parent)?; }
        }
        // UClass::Register reaches UStruct::Register before constructing defaults.
        unsafe {
            (*class.cast::<UStruct>()).friendly_name = (*class.cast::<UObject>()).name;
        }
        Ok(())
    }
    unsafe fn restore_config_name(&self, class: *mut c_void) -> RuntimeResult<()> {
        let class = class.cast::<UClass>();
        let metadata = unsafe { self.access.class_metadata(class)? };
        if let Some(spec) = metadata.native_spec {
            let name = self.access.intern_str(spec.config, crate::name_owner::FindName::Add)?;
            unsafe { ptr::addr_of_mut!((*class).class_config_name).cast::<i32>().write_unaligned(name); }
        }
        Ok(())
    }
    unsafe fn reset_defaults(&self, class: *mut c_void) -> RuntimeResult<()> {
        unsafe { self.access.reset_class_defaults(class.cast()) }
    }
    unsafe fn initialize_default(&self, class: *mut c_void) -> RuntimeResult<()> {
        unsafe { self.access.initialize_default(class.cast()) }
    }
    unsafe fn superclass(&self, class: *mut c_void) -> *mut c_void {
        unsafe { (*class.cast::<UClass>()).super_field }
    }
    unsafe fn constructor_identity(&self, class: *mut c_void) -> usize {
        unsafe { self.access.class_metadata(class.cast()).map_or(0, |data| data.constructor_identity) }
    }
    unsafe fn default_object(&self, class: *mut c_void) -> RuntimeResult<*mut c_void> {
        Ok(unsafe { self.access.default_object(class.cast())? }.cast())
    }
    unsafe fn run_static_constructor(&self, class: *mut c_void, default: *mut c_void) -> RuntimeResult<()> {
        unsafe {
            crate::core_bootstrap::static_constructor(
                self.access,
                class.cast(),
                default.cast(),
            )
        }
    }
    unsafe fn class_flags(&self, class: *mut c_void) -> u32 {
        unsafe { (*class.cast::<UClass>()).class_flags }
    }
    unsafe fn add_class_flags(&self, class: *mut c_void, flags: u32) -> RuntimeResult<()> {
        unsafe { (*class.cast::<UClass>()).class_flags |= flags; }
        Ok(())
    }
    unsafe fn link_defaults(&self, class: *mut c_void) -> RuntimeResult<()> {
        unsafe { self.access.link_struct(class.cast(), None, false) }
    }
    unsafe fn load_config(&self, class: *mut c_void) -> RuntimeResult<()> {
        // MetadataLoadConfig targets GetDefaultObject(), not the UClass instance.
        let default = unsafe { self.access.default_object(class.cast())? };
        unsafe { self.access.load_config(default) }
    }
    unsafe fn load_localized(&self, class: *mut c_void) -> RuntimeResult<()> {
        let default = unsafe { self.access.default_object(class.cast())? };
        unsafe { self.access.load_localized(default) }
    }
}
