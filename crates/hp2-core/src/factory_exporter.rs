//! Factory selection and import/export transactions. Native leaves retain typed
//! virtual calls and stack-owned engine buffers; every failed leaf stops effects.
use std::ffi::c_void;

type Result<T, E = i32> = std::result::Result<T, E>;

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2FactoryExporterOperation {
    Import,
    ImportText,
    ImportBinary,
    Find,
    Archive,
    Text,
    File,
    FileText,
    FileBinary,
}

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2FactoryExporterLeaf {
    CheckInput,
    CheckExplicit,
    ClassCurrent,
    ClassAdvance,
    ClassDefault,
    Supported,
    Priority,
    Construct,
    Delete,
    CreateNew,
    IsText,
    FilenameEmpty,
    ImportLog,
    Parse,
    InvokeNew,
    TextScope,
    BinaryScope,
    LoadText,
    LoadBinary,
    BinarySentinel,
    InvokeTextImport,
    InvokeBinaryImport,
    PostProcess,
    CheckResult,
    ImportMissing,
    FindCall,
    FormatCount,
    FormatMatches,
    ExportCompatible,
    ExportMissing,
    InvokeArchive,
    Indent,
    SetIndent,
    InvokeTextExport,
    FileTextScope,
    FileBinaryScope,
    ExportBuffer,
    Identical,
    IdenticalLog,
    Prompt,
    Save,
    SaveFailed,
}

#[repr(C)]
pub struct Hp2FactoryExporterServices {
    pub call:
        unsafe extern "C" fn(*mut c_void, Hp2FactoryExporterLeaf, usize, usize, *mut usize) -> i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Hp2FactoryExporterArgs {
    pub native: *mut c_void,
    pub input: usize,
    pub indent: i32,
    pub no_replace: i32,
    pub prompt: i32,
}

struct Flow<'a> {
    services: &'a Hp2FactoryExporterServices,
    args: Hp2FactoryExporterArgs,
}
use Hp2FactoryExporterLeaf as L;
use Hp2FactoryExporterOperation as O;

impl Flow<'_> {
    fn call(&self, leaf: L, a: usize, b: usize) -> Result<usize> {
        let mut value = 0;
        let status = unsafe { (self.services.call)(self.args.native, leaf, a, b, &mut value) };
        if status == 0 { Ok(value) } else { Err(status) }
    }
    fn leaf(&self, leaf: L) -> Result<usize> {
        self.call(leaf, 0, 0)
    }
    fn get(&self, leaf: L, a: usize) -> Result<usize> {
        self.call(leaf, a, 0)
    }
    fn compare(&self, a: usize, b: usize) -> Result<i32> {
        Ok((self.get(L::Priority, a)? as i32).wrapping_sub(self.get(L::Priority, b)? as i32))
    }
    // Exact legacy unstable selection/partition ordering, including equal priorities.
    fn sort(&self, factories: &mut [usize]) -> Result<()> {
        let count = factories.len() as isize;
        let ptr = factories.as_mut_ptr();
        crate::sort::quicksort(
            count,
            |_| Ok(()),
            |i, j| {
                self.compare(unsafe { *ptr.add(i as usize) }, unsafe {
                    *ptr.add(j as usize)
                })
            },
            |i, j| unsafe { std::ptr::swap(ptr.add(i as usize), ptr.add(j as usize)) },
        )
    }
    fn import_attempt(&self, factory: usize) -> Result<usize> {
        if self.get(L::CreateNew, factory)? != 0 {
            if self.leaf(L::FilenameEmpty)? != 0 {
                self.call(L::ImportLog, factory, 0)?;
                self.get(L::Parse, factory)?;
                return self.get(L::InvokeNew, factory);
            }
        } else if self.leaf(L::FilenameEmpty)? == 0 {
            let text = self.get(L::IsText, factory)? != 0;
            self.call(L::ImportLog, factory, if text { 1 } else { 2 })?;
            return self.get(if text { L::TextScope } else { L::BinaryScope }, factory);
        }
        Ok(0)
    }
    fn import(&self) -> Result<usize> {
        self.leaf(L::CheckInput)?;
        if self.args.input != 0 {
            self.get(L::CheckExplicit, self.args.input)?;
            let result = self.import_attempt(self.args.input)?;
            if result != 0 {
                self.get(L::CheckResult, result)?;
            } else {
                self.leaf(L::ImportMissing)?;
            }
            return Ok(result);
        }
        let mut factories = Vec::new();
        loop {
            let class = self.leaf(L::ClassCurrent)?;
            if class == 0 {
                break;
            }
            let default = self.get(L::ClassDefault, class)?;
            if default != 0
                && self.get(L::Supported, default)? != 0
                && (self.get(L::Priority, default)? as i32) >= 0
            {
                factories.push(self.get(L::Construct, class)?);
            }
            self.leaf(L::ClassAdvance)?;
        }
        self.sort(&mut factories)?;
        let mut result = 0;
        for &factory in &factories {
            result = self.import_attempt(factory)?;
            if result != 0 {
                self.get(L::CheckResult, result)?;
                break;
            }
        }
        // Deliberately not RAII: the native implementation deletes owned objects
        // only on normal return, never while unwinding a virtual call.
        for factory in factories {
            self.get(L::Delete, factory)?;
        }
        if result == 0 {
            self.leaf(L::ImportMissing)?;
        }
        Ok(result)
    }
    fn find(&self) -> Result<usize> {
        self.leaf(L::CheckInput)?;
        loop {
            let class = self.leaf(L::ClassCurrent)?;
            if class == 0 {
                return Ok(0);
            }
            let default = self.get(L::ClassDefault, class)?;
            if default != 0 && self.get(L::Supported, default)? != 0 {
                let count = self.get(L::FormatCount, default)?;
                for index in 0..count {
                    if self.call(L::FormatMatches, default, index)? != 0 {
                        return self.get(L::Construct, class);
                    }
                }
            }
            self.leaf(L::ClassAdvance)?;
        }
    }
    fn export(&self, operation: O) -> Result<usize> {
        self.leaf(L::CheckInput)?;
        let exporter = if self.args.input != 0 {
            self.args.input
        } else {
            self.leaf(L::FindCall)?
        };
        if exporter == 0 {
            self.get(L::ExportMissing, matches!(operation, O::Archive) as usize)?;
            return Ok(0);
        }
        let result = match operation {
            O::File => self.get(
                if self.get(L::IsText, exporter)? != 0 {
                    L::FileTextScope
                } else {
                    L::FileBinaryScope
                },
                exporter,
            )?,
            O::Archive => {
                self.get(L::ExportCompatible, exporter)?;
                self.get(L::InvokeArchive, exporter)?;
                0
            }
            O::Text => {
                self.get(L::ExportCompatible, exporter)?;
                let saved = self.get(L::Indent, exporter)?;
                self.call(L::SetIndent, exporter, self.args.indent as usize)?;
                self.get(L::InvokeTextExport, exporter)?;
                self.call(L::SetIndent, exporter, saved)?;
                0
            }
            _ => unreachable!(),
        };
        if self.args.input == 0 {
            self.get(L::Delete, exporter)?;
        }
        Ok(result)
    }
    fn file_buffer(&self) -> Result<usize> {
        self.get(L::ExportBuffer, self.args.input)?;
        if self.args.no_replace != 0 {
            if self.leaf(L::Identical)? != 0 {
                self.leaf(L::IdenticalLog)?;
                return Ok(1);
            }
            if self.args.prompt != 0 && self.leaf(L::Prompt)? == 0 {
                return Ok(1);
            }
        }
        if self.leaf(L::Save)? == 0 {
            self.leaf(L::SaveFailed)?;
            return Ok(0);
        }
        Ok(1)
    }
}

/// # Safety
/// The native context and typed handles must stay live through synchronous,
/// potentially reentrant callbacks; all callbacks must capture C++ exceptions.
pub unsafe fn execute(
    services: &Hp2FactoryExporterServices,
    operation: O,
    args: Hp2FactoryExporterArgs,
) -> Result<usize> {
    let flow = Flow { services, args };
    match operation {
        O::Import => flow.import(),
        O::ImportText | O::ImportBinary => {
            let text = matches!(operation, O::ImportText);
            if flow.leaf(if text { L::LoadText } else { L::LoadBinary })? == 0 {
                return Ok(0);
            }
            if !text {
                flow.leaf(L::BinarySentinel)?;
            }
            flow.get(L::Parse, args.input)?;
            let result = flow.get(
                if text {
                    L::InvokeTextImport
                } else {
                    L::InvokeBinaryImport
                },
                args.input,
            )?;
            if !text && result != 0 {
                flow.get(L::PostProcess, result)?;
            }
            Ok(result)
        }
        O::Find => flow.find(),
        O::Archive | O::Text | O::File => flow.export(operation),
        O::FileText | O::FileBinary => flow.file_buffer(),
    }
}
