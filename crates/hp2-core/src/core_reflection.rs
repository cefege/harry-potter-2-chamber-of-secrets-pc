//! Direct reflection access over the canonical owned object and class records.

use crate::core_access::{CoreAccess, RuntimeError, RuntimeResult};
use crate::core_class_specs::NativeRecordKind;
use crate::object_archive::ObjectArchive;
use crate::object_lifecycle::{self, LifecycleOps};
use crate::object_owner::{ClassData, ClassOps, OwnedClassRecord};
use crate::{object_model, object_storage};
use hp2_abi::engine::generated::{FStateFrame, UClass, UField, UFunction, UObject, UProperty, UStruct};
use std::ffi::c_void;
use std::ptr;

impl CoreAccess<'_> {
    /// Advances Source `FObjectIterator` through the live canonical object table.
    ///
    /// # Safety
    /// The cursor and optional class remain live for this synchronous operation.
    pub unsafe fn next_object_index(self, index: &mut i32, class: *mut UClass) -> RuntimeResult<()> {
        unsafe { crate::object_storage::next_index_is_a(self.storage(), index, class.cast()) }
            .map_err(|_| RuntimeError::new("Canonical object iteration failed"))
    }

    /// # Safety
    /// `class` is a live owned class record, including its initialized Rust tail.
    pub(crate) unsafe fn class_metadata(self, class: *mut UClass) -> RuntimeResult<ClassData> {
        if class.is_null() {
            return Err(RuntimeError::new("Object has no class"));
        }
        // SAFETY: the caller supplies the complete canonical class allocation.
        unsafe { OwnedClassRecord::metadata(class.cast()) }
    }

    /// # Safety
    /// The object and its class remain live through the returned operation call.
    pub(crate) unsafe fn class_ops(self, object: *mut UObject) -> RuntimeResult<ClassOps> {
        let class = unsafe { ptr::addr_of!((*object).class).read() };
        // Copy operation metadata before any callback or class-body mutation.
        Ok(unsafe { self.class_metadata(class.cast())? }.ops)
    }

    /// Calls the registered native `UObject::IsPendingKill` operation.
    ///
    /// # Safety
    /// `object` is a live canonical object and remains live through the callback.
    pub unsafe fn is_pending_kill(self, object: *mut UObject) -> RuntimeResult<bool> {
        let ops = unsafe { self.class_ops(object)? };
        unsafe { (ops.is_pending_kill)(self, object) }
    }

    /// Calls the registered native `UObject::ProcessRemoteFunction` operation.
    ///
    /// # Safety
    /// Object, function, parameters and optional stack remain valid for the synchronous callback.
    pub unsafe fn process_remote_function(
        self, object: *mut UObject, function: *mut UFunction, parameters: *mut c_void,
        stack: *mut hp2_abi::engine::generated::FFrame,
    ) -> RuntimeResult<bool> {
        let ops = unsafe { self.class_ops(object)? };
        unsafe { (ops.process_remote_function)(self, object, function, parameters, stack) }
    }



    /// # Safety
    /// The object and its class are live canonical records.
    pub(crate) unsafe fn record_kind(self, object: *mut UObject) -> RuntimeResult<NativeRecordKind> {
        let class = unsafe { ptr::addr_of!((*object).class).read() };
        if class.is_null() {
            return Err(RuntimeError::new("Object has no class"));
        }
        // SAFETY: the caller supplies the complete canonical class allocation.
        unsafe { OwnedClassRecord::kind(class.cast()) }
    }

    /// Finds by the existing name/outer hash; no allocation or callback occurs.
    ///
    /// # Safety
    /// Non-null class and outer pointers identify live canonical records.
    pub unsafe fn find_object(
        self,
        class: *mut UClass,
        outer: *mut UObject,
        name: i32,
        exact_class: bool,
    ) -> *mut UObject {
        unsafe { self.lookup_resolved_object(class, outer, name, exact_class, false) }
    }

    unsafe fn lookup_resolved_object(
        self, class: *mut UClass, outer: *mut UObject, name: i32,
        exact_class: bool, any_package: bool,
    ) -> *mut UObject {
        let ops = DirectLookup { access: self, class, exact_class };
        match crate::package_flow::lookup_object_with(&ops, name as usize, outer as usize, any_package) {
            Ok(object) => object as *mut UObject,
            Err(never) => match never {},
        }
    }

    /// Finds existing objects using the canonical name/path and ANY_PACKAGE rules.
    /// This never loads a package, creates an object, or adds a name.
    ///
    /// # Safety
    /// `class` is null or a live class in this owner.
    pub(crate) unsafe fn find_object_text(
        self, class: *mut UClass, name: &[u32],
    ) -> RuntimeResult<*mut UObject> {
        let mut resolver = DirectNameResolution {
            access: self, original: name.split(|unit| *unit == 0).next().unwrap_or_default(),
            configured: None, offset: 0, outer: ptr::null_mut(),
        };
        if !crate::package_flow::resolve_name_with(&mut resolver)? { return Ok(ptr::null_mut()); }
        let name = self.find_name(resolver.text())?;
        Ok(unsafe { self.lookup_resolved_object(class, resolver.outer, name, false, true) })
    }

    /// Finds a loaded object by canonical name or path using ANY_PACKAGE lookup.
    ///
    /// # Safety
    /// `class` is null or a live class in this owner.
    pub unsafe fn find_object_by_name(
        self,
        class: *mut UClass,
        name: &[u32],
    ) -> RuntimeResult<Option<*mut UObject>> {
        let object = unsafe { self.find_object_text(class, name)? };
        Ok((!object.is_null()).then_some(object))
    }

    /// Walks the actual child/super links in reflected declaration order.
    ///
    /// # Safety
    /// The structure, field chain and optional field class remain live.
    pub unsafe fn find_field(
        self,
        mut structure: *mut UStruct,
        name: i32,
        field_class: *mut UClass,
    ) -> *mut UField {
        while !structure.is_null() {
            let mut field = unsafe { (*structure).children.cast::<UField>() };
            while !field.is_null() {
                if unsafe {
                    i32::from_ne_bytes((*field.cast::<UObject>()).name) == name
                        && object_model::is_a(field.cast(), field_class.cast())
                } {
                    return field;
                }
                field = unsafe { (*field).next.cast() };
            }
            structure = unsafe { (*structure.cast::<UField>()).super_field.cast() };
        }
        ptr::null_mut()
    }

    /// Exports one live reflected property using its canonical property serializer.
    ///
    /// # Safety
    /// `object` and its class/property metadata remain live for the synchronous call.
    pub unsafe fn export_property_text(
        self,
        object: *mut UObject,
        name: &[u32],
    ) -> RuntimeResult<Vec<u32>> {
        if object.is_null() { return Err(RuntimeError::new("Null property export object")); }
        let name = self.intern(name, crate::name_owner::FindName::Find)?;
        let property_class = self.native_class("Core.Property")?;
        let class = unsafe { (*object).class.cast::<UStruct>() };
        let property = unsafe { self.find_field(class, name, property_class) }.cast::<UProperty>();
        if property.is_null() { return Err(RuntimeError::new("Reflected property was not found")); }
        unsafe { self.property_export_text(property, object, ptr::null_mut(), 0) }
    }

    /// Exports a property's complete value for Engine travel serialization.
    ///
    /// # Safety
    /// Property and object/default storage must be live and match the reflected layout.
    pub unsafe fn property_export_text_full(
        self, property: *mut UProperty, object: *mut UObject,
        default_object: *mut UObject, flags: u32,
    ) -> RuntimeResult<Vec<u32>> {
        unsafe { self.property_export_text(property, object, default_object, flags) }
    }

    /// Exports one fixed-array element using the canonical property text policy.
    ///
    /// # Safety
    /// Property and object/default storage must be live and match the reflected layout.
    pub unsafe fn property_export_text_indexed(
        self, property: *mut UProperty, object: *mut UObject,
        default_object: *mut UObject, index: i32, flags: u32,
    ) -> RuntimeResult<Vec<u32>> {
        let value = unsafe { self.property_address(object, property)? };
        let default = if default_object.is_null() { ptr::null_mut() } else {
            unsafe { self.property_address(default_object, property)? }
        };
        let mut output = Vec::new();
        unsafe { self.property_export_text_index(property, value, default, index, flags, &mut output)?; }
        Ok(output)
    }

    /// # Safety
    /// The object, class and archive mappings remain valid through reentry.
    pub unsafe fn serialize_object(
        self,
        object: *mut UObject,
        archive: &ObjectArchive<'_>,
    ) -> RuntimeResult<()> {
        let ops = unsafe { self.class_ops(object)? };
        unsafe { (ops.serialize)(self, object, archive) }
    }

    /// # Safety
    /// The object and function are live records; parameters/results cover their
    /// reflected layout and do not alias a borrowed class or store view.
    pub unsafe fn execute_event(
        self,
        object: *mut UObject,
        function: *mut UFunction,
        parameters: *mut c_void,
        result: *mut c_void,
    ) -> RuntimeResult<()> {
        let ops = unsafe { self.class_ops(object)? };
        unsafe { (ops.execute_event)(self, object, function, parameters, result) }
    }

    /// Executes a reflected event by name through the object's canonical class operation.
    ///
    /// Returns `false` when the event is not declared on the class hierarchy.
    ///
    /// # Errors
    /// Returns name lookup or canonical VM execution failures.
    ///
    /// # Safety
    /// `object` is a live canonical object and remains live through synchronous
    /// event execution; event handlers may reenter Core.
    pub unsafe fn execute_named_event(
        self,
        object: *mut UObject,
        event: &str,
    ) -> RuntimeResult<bool> {
        if object.is_null() {
            return Err(RuntimeError::new("Cannot execute an event on a null object"));
        }
        // Use the same UObject::FindFunction policy as ObjectInstance: state
        // declarations shadow class declarations, while global lookup skips state.
        let function = unsafe { self.find_object_function(object, event, false)? };
        if function.is_null() {
            return Ok(false);
        }
        unsafe { self.execute_event(object, function, ptr::null_mut(), ptr::null_mut())?; }
        Ok(true)
    }

    /// Finds a reflected function using UObject's state-first/global lookup policy.
    ///
    /// # Safety
    /// `object` is a live canonical object whose state frame, class and fields remain live.
    pub(crate) unsafe fn find_object_function(
        self,
        object: *mut UObject,
        event: &str,
        global: bool,
    ) -> RuntimeResult<*mut UFunction> {
        if object.is_null() {
            return Err(RuntimeError::new("Cannot find a function on a null object"));
        }
        let name = self.intern_str(event, crate::name_owner::FindName::Find)?;
        unsafe { self.find_object_function_by_name(object, name, global) }
    }

    /// Finds by an existing name index using UObject's state-first/global policy.
    ///
    /// # Safety
    /// `object` and its state/class fields are live canonical records.
    pub unsafe fn find_object_function_by_name(
        self,
        object: *mut UObject,
        name: i32,
        global: bool,
    ) -> RuntimeResult<*mut UFunction> {
        let function_class = self.native_class("Function")?;
        let frame = unsafe { ptr::addr_of!((*object).state_frame).read().cast::<FStateFrame>() };
        let state = if frame.is_null() {
            0
        } else {
            unsafe { ptr::addr_of!((*frame).state_node).read() as usize }
        };
        let class = unsafe { ptr::addr_of!((*object).class).read() as usize };
        let field = crate::object_instance::find_state_first(
            name,
            global,
            state,
            class,
            |structure, name| {
                Ok(unsafe {
                    self.find_field(structure as *mut UStruct, name, function_class) as usize
                })
            },
        )?;
        Ok(field as *mut UFunction)

    }

    /// Imports console-command text into reflected parameter storage using Core's property grammar.
    ///
    /// # Safety
    /// `property` and `value` identify live reflected metadata/storage; `input` is NUL-terminated.
    pub unsafe fn import_console_argument(
        self,
        property: *mut hp2_abi::engine::generated::UProperty,
        value: *mut u8,
        input: &[u32],
    ) -> RuntimeResult<usize> {
        unsafe {
            self.property_import_text_at(
                property,
                value,
                input,
                ptr::null_mut(),
                2, // PPF_Delimited: consume one property token per console argument.
            )
        }
    }

    /// Executes UObject::ScriptConsoleExec through canonical reflected metadata and VM dispatch.
    ///
    /// # Safety
    /// Object, executor, command, and the output callback remain live through synchronous reentry.
    pub unsafe fn execute_console(
        self,
        object: *mut UObject,
        executor: *mut UObject,
        command: *const u32,
        output: &mut dyn FnMut(&[u32]) -> RuntimeResult<()>,
    ) -> RuntimeResult<bool> {
        unsafe { crate::object_instance::execute_core_console(self, object, executor, command, output) }
    }

    /// Source-gated variant for virtual ProcessEvent overrides such as AActor.
    ///
    /// # Safety
    /// The object, executor, command, output, and callback remain live through
    /// synchronous script reentry. The callback must preserve the selected object's
    /// canonical ProcessEvent semantics before invoking Core VM dispatch.
    pub unsafe fn execute_console_with(
        self,
        object: *mut UObject,
        executor: *mut UObject,
        command: *const u32,
        output: &mut dyn FnMut(&[u32]) -> RuntimeResult<()>,
        process_event: &mut dyn FnMut(*mut UObject, *mut UFunction, *mut c_void) -> RuntimeResult<()>,
    ) -> RuntimeResult<bool> {
        unsafe {
            crate::object_instance::execute_core_console_with(
                self, object, executor, command, output, process_event,
            )
        }
    }

    /// Explicitly initializes the object's execution frame using its class operation.
    ///
    /// This is not called during construction. Repeated calls follow the class's
    /// native initialization policy.
    ///
    /// # Errors
    /// Returns an ownership mismatch or the class operation's initialization error.
    ///
    /// # Safety
    /// `object` must be a live canonical allocation in this owner, and its class
    /// must remain live through the operation. The caller must invoke this only
    /// when the native execution-initialization lifecycle requires it.
    pub unsafe fn initialize_execution(self, object: *mut UObject) -> RuntimeResult<()> {
        if object.is_null() { return Err(RuntimeError::new("Cannot initialize a null object")); }
        if !self.contains_object(object) {
            return Err(RuntimeError::new("Object does not belong to this owner"));
        }
        // SAFETY: pointer identity was matched against this owner's live slots.
        let ops = unsafe { self.class_ops(object)? };
        unsafe { (ops.initialize_execution)(self, object) }
    }

    /// The object and its class remain live through post-load callbacks.
    pub unsafe fn conditional_post_load(self, object: *mut UObject) -> RuntimeResult<()> {
        object_lifecycle::conditional_post_load_with(object.cast(), &DirectLifecycle(self))
    }

    /// Runs complete class teardown once, retaining the canonical identity slot.
    ///
    /// # Errors
    /// Returns an ownership mismatch or the class's ordered teardown error.
    ///
    /// # Safety
    /// `object` is a live canonical allocation in this owner. Its fields must not
    /// be accessed after teardown, even though its identity remains published.
    pub unsafe fn conditional_destroy(self, object: *mut UObject) -> RuntimeResult<bool> {
        if object.is_null() { return Err(RuntimeError::new("Cannot destroy a null object")); }
        // SAFETY: the caller supplies a live allocation; compare its identity
        // before dispatch so a different owner's class/storage is never used.
        let index = unsafe { (*object).index };
        if self.object_at(index) != object {
            return Err(RuntimeError::new("Object does not belong to this owner"));
        }
        // SAFETY: this is this owner's exact live slot. The slot state prevents
        // reentrant teardown; flags preserve ConditionalDestroy's return value.
        let called = unsafe {
            (*self.storage()).needs_teardown(index)
                && (*object).object_flags & object_lifecycle::RF_DESTROYED == 0
        };
        unsafe { self.destroy_slot(index)?; }
        Ok(called)
    }

    /// Tears down and releases one canonical object without another GC pass.
    ///
    /// # Errors
    /// Returns the first teardown or release error. A reentrantly republished
    /// live slot is not freed as if it were the old destroyed allocation.
    ///
    /// # Safety
    /// `object` is owned here, unrooted, and not borrowed by a VM frame, callback
    /// or caller that will subsequently access it. No surviving reference may
    /// be dereferenced after release.
    pub unsafe fn delete_object(self, object: *mut UObject) -> RuntimeResult<()> {
        if object.is_null() { return Ok(()); }
        // SAFETY: the caller supplies a live owned allocation.
        let index = unsafe { (*object).index };
        if self.object_at(index) != object {
            return Err(RuntimeError::new("Object does not belong to this owner"));
        }
        let destroyed = unsafe { self.conditional_destroy(object) }.map(|_| ());
        // SAFETY: free_slot validates completed teardown before removing any
        // allocation; an operational destroy error does not bypass final cleanup.
        let released = unsafe { self.free_slot(index) };
        destroyed.and(released)
    }

    /// # Safety
    /// The object and its class remain live through edit-notification callbacks.
    pub unsafe fn post_edit_change(self, object: *mut UObject) -> RuntimeResult<()> {
        let operations = unsafe { self.class_ops(object)? };
        unsafe { (operations.post_edit_change)(self, object) }
    }
}

// Private adapter: the public generic lifecycle trait has safe raw-pointer
// methods. Do not implement it on the public capability and expose unchecked
// dereferences through otherwise-safe trait calls.
struct DirectLifecycle<'a>(CoreAccess<'a>);

impl LifecycleOps for DirectLifecycle<'_> {
    type Error = RuntimeError;

    fn index(&self, object: *mut c_void) -> RuntimeResult<u32> {
        Ok(unsafe { (*object.cast::<UObject>()).index as u32 })
    }
    fn flags(&self, object: *mut c_void) -> RuntimeResult<u32> {
        Ok(unsafe { (*object.cast::<UObject>()).object_flags })
    }
    fn set_flags(&self, object: *mut c_void, flags: u32) -> RuntimeResult<()> {
        unsafe { (*object.cast::<UObject>()).object_flags |= flags };
        Ok(())
    }
    fn clear_flags(&self, object: *mut c_void, flags: u32) -> RuntimeResult<()> {
        unsafe { (*object.cast::<UObject>()).object_flags &= !flags };
        Ok(())
    }
    fn destroy(&self, object: *mut c_void) -> RuntimeResult<()> {
        let ops = unsafe { self.0.class_ops(object.cast())? };
        unsafe { (ops.destroy)(self.0, object.cast()) }
    }
    fn post_load(&self, object: *mut c_void) -> RuntimeResult<()> {
        let ops = unsafe { self.0.class_ops(object.cast())? };
        unsafe { (ops.post_load)(self.0, object.cast()) }
    }
    fn route_error(&self, object: *mut c_void, post_load: i32) -> RuntimeResult<()> {
        let name = unsafe { i32::from_ne_bytes((*object.cast::<UObject>()).name) };
        let text = self.0.name_text(name)?;
        Err(RuntimeError::new(if post_load == 0 {
            "Failed to route Destroy"
        } else {
            "Failed to route PostLoad"
        }).with_utf32_context(text))
    }
}

/// CoreOwner's mandatory complete-class teardown dispatcher.
///
/// # Safety
/// The pointer is a live owned slot and its class operation performs complete
/// field cleanup on both success and failure. Deallocation is a separate pass.
pub(crate) unsafe fn destroy_record(access: CoreAccess<'_>, object: *mut UObject) -> RuntimeResult<()> {
    object_lifecycle::conditional_destroy_with(object.cast(), &DirectLifecycle(access)).map(|_| ())
}

struct DirectLookup<'a> {
    access: CoreAccess<'a>,
    class: *mut UClass,
    exact_class: bool,
}

impl crate::package_flow::ObjectLookupOps for DirectLookup<'_> {
    type Error = std::convert::Infallible;
    fn hash_head(&self, name: usize, outer: usize) -> Result<usize, Self::Error> {
        let outer_index = if outer == 0 { 0 } else { unsafe { (*(outer as *const UObject)).index } };
        Ok(unsafe { (*self.access.storage()).hash_head(object_storage::hash_bucket(name as i32, outer_index)) as usize })
    }
    fn name(&self, object: usize) -> Result<usize, Self::Error> {
        Ok(unsafe { i32::from_ne_bytes((*(object as *const UObject)).name) as usize })
    }
    fn outer(&self, object: usize) -> Result<usize, Self::Error> {
        Ok(unsafe { (*(object as *const UObject)).outer as usize })
    }
    fn matches_class(&self, object: usize) -> Result<bool, Self::Error> {
        Ok(self.class.is_null() || unsafe {
            if self.exact_class { (*(object as *const UObject)).class == self.class.cast() }
            else { object_model::is_a(object as *const c_void, self.class.cast()) }
        })
    }
    fn next(&self, object: usize) -> Result<usize, Self::Error> {
        Ok(unsafe { (*(object as *const UObject)).hash_next as usize })
    }
    fn count(&self) -> Result<usize, Self::Error> { Ok(self.access.object_count() as usize) }
    fn at(&self, index: usize) -> Result<usize, Self::Error> { Ok(self.access.object_at(index as i32) as usize) }
}

/// Resolves ini:/usr: through the same policy as object lookup, without looking
/// up a class early and bypassing its required package exports/defaults.
pub(crate) fn resolve_configured_load_name<'text>(
    access: CoreAccess<'_>, name: &'text [u32],
) -> RuntimeResult<Option<std::borrow::Cow<'text, [u32]>>> {
    let original = name.split(|unit| *unit == 0).next().unwrap_or_default();
    let mut resolver = DirectNameResolution {
        access, original, configured: None, offset: 0, outer: ptr::null_mut(),
    };
    if !crate::package_flow::resolve_config_name_with(&mut resolver)? { return Ok(None); }
    Ok(Some(match resolver.configured {
        Some(text) => std::borrow::Cow::Owned(text),
        None => std::borrow::Cow::Borrowed(original),
    }))
}

struct DirectNameResolution<'a, 'text> {
    access: CoreAccess<'a>,
    original: &'text [u32],
    configured: Option<Vec<u32>>,
    offset: usize,
    outer: *mut UObject,
}

impl DirectNameResolution<'_, '_> {
    fn text(&self) -> &[u32] { &self.configured.as_deref().unwrap_or(self.original)[self.offset..] }
}

impl crate::package_flow::NameResolutionOps for DirectNameResolution<'_, '_> {
    type Error = RuntimeError;
    fn config_prefix(&mut self) -> RuntimeResult<Option<bool>> {
        let text = self.text();
        let prefix = |word: &[u8]| text.len() >= word.len() && text.iter().zip(word).all(|(&unit, &byte)| {
            unit <= 127 && (unit as u8).eq_ignore_ascii_case(&byte)
        });
        Ok(if prefix(b"ini:") { Some(true) } else if prefix(b"usr:") { Some(false) } else { None })
    }
    fn name_length(&mut self) -> RuntimeResult<usize> { Ok(self.text().len()) }
    fn has_separator(&mut self) -> RuntimeResult<bool> { Ok(self.text().contains(&u32::from('.'))) }
    fn resolve_config(&mut self, system: bool) -> RuntimeResult<bool> {
        let text = &self.text()[4..];
        let separator = text.iter().rposition(|unit| *unit == u32::from('.'))
            .ok_or_else(|| RuntimeError::new("Configuration object name has no key"))?;
        let filename = if system { &[83, 121, 115, 116, 101, 109][..] } else { &[85, 115, 101, 114][..] };
        let configured = unsafe { &mut *self.access.config_storage() }
            .get(Some(filename), &text[..separator], &text[separator + 1..])?;
        let Some(mut configured) = configured else { return Ok(false); };
        // Native ConfigGet writes into appStaticString1024.
        configured.truncate(1023);
        if let Some(end) = configured.iter().position(|unit| *unit == 0) { configured.truncate(end); }
        self.configured = Some(configured);
        self.offset = 0;
        Ok(true)
    }
    fn partial_fits(&mut self) -> RuntimeResult<bool> { Ok(self.text().len() < 1024) }
    fn resolve_outer(&mut self) -> RuntimeResult<Option<usize>> {
        let separator = self.text().iter().position(|unit| *unit == u32::from('.'))
            .ok_or_else(|| RuntimeError::new("Object path has no separator"))?;
        let name = self.access.find_name(&self.text()[..separator])?;
        let package_class = self.access.native_class("Package")?;
        let mut outer = unsafe { self.access.find_object(package_class, self.outer, name, false) };
        if outer.is_null() { outer = unsafe { self.access.find_object(ptr::null_mut(), self.outer, name, false) }; }
        Ok((!outer.is_null()).then_some(outer as usize))
    }
    fn publish_outer(&mut self, outer: usize) { self.outer = outer as *mut UObject; }
    fn advance_name(&mut self) -> RuntimeResult<()> {
        let separator = self.text().iter().position(|unit| *unit == u32::from('.'))
            .ok_or_else(|| RuntimeError::new("Object path has no separator"))?;
        self.offset += separator + 1;
        Ok(())
    }
}

/// Source GetPathName text, shared by canonical VM conversions and trace producers.
pub(crate) fn object_path_text_with(
    object: *mut UObject, name_text: &impl Fn(i32) -> RuntimeResult<Vec<u32>>,
) -> RuntimeResult<Vec<u32>> {
    let mut output = Vec::new();
    append_object_path_with(object, &|name, output| {
        output.extend(name_text(name)?);
        Ok(())
    }, &mut output)?;
    Ok(output)
}

fn append_object_path_with(
    object: *mut UObject,
    name_text: &impl Fn(i32, &mut Vec<u32>) -> RuntimeResult<()>,
    output: &mut Vec<u32>,
) -> RuntimeResult<()> {
    fn append(
        object: *mut UObject,
        name_text: &impl Fn(i32, &mut Vec<u32>) -> RuntimeResult<()>,
        output: &mut Vec<u32>,
    ) -> RuntimeResult<()> {
        if object.is_null() { return Ok(()); }
        let outer = unsafe { ptr::addr_of!((*object).outer).read() };
        if !outer.is_null() {
            append(outer.cast(), name_text, output)?;
            output.push(u32::from(b'.'));
        }
        let name = i32::from_ne_bytes(unsafe { ptr::addr_of!((*object).name).read() });
        name_text(name, output)
    }
    if object.is_null() {
        output.extend("None".chars().map(u32::from));
        return Ok(());
    }
    append(object, name_text, output)
}

impl CoreAccess<'_> {
    /// Writes Source `GetPathName()` into reusable caller-owned scratch.
    ///
    /// # Safety
    /// A non-null object and its complete outer chain are live in this owner.
    pub unsafe fn write_object_path_text(
        self, object: *mut UObject, output: &mut Vec<u32>,
    ) -> RuntimeResult<()> {
        output.clear();
        append_object_path_with(object, &|name, output| self.append_name_text(name, output), output)
    }
}
