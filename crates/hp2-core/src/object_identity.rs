//! Object naming policy; native leaves retain FName interning and TCHAR scratch storage.
//! No borrowed native storage is retained across a callback.
type Result<T, E = i32> = std::result::Result<T, E>;

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2IdentityOperation {
    Rename,
    Path,
    Full,
    Unique,
}

/// cbindgen:prefix-with-name
#[repr(C)]
#[derive(Clone, Copy)]
pub enum Hp2IdentityLeaf {
    Outer,
    Class,
    InternName,
    UniqueCall,
    Unhash,
    LogRename,
    AssignName,
    Hash,
    Clear,
    None,
    AppendName,
    AppendDot,
    PathCall,
    ClassPrefix,
    CheckClass,
    CopyBase,
    BaseLength,
    BaseDigit,
    TerminateBase,
    Counter,
    SetCounter,
    FormatUnique,
    FindUnique,
    InternUnique,
}

#[repr(C)]
pub struct Hp2IdentityArgs {
    pub object: usize,
    pub object_class: usize,
    pub parent: usize,
    pub name: usize,
    pub stop: usize,
}

#[repr(C)]
pub struct Hp2IdentityServices {
    pub context: *mut std::ffi::c_void,
    pub call: unsafe extern "C" fn(
        *mut std::ffi::c_void,
        Hp2IdentityLeaf,
        usize,
        usize,
        *mut usize,
    ) -> i32,
}

struct Native<'a>(&'a Hp2IdentityServices);
impl Native<'_> {
    fn call(&self, leaf: Hp2IdentityLeaf, a: usize, b: usize) -> Result<usize> {
        let mut result = 0;
        let status = unsafe { (self.0.call)(self.0.context, leaf, a, b, &mut result) };
        if status == 0 { Ok(result) } else { Err(status) }
    }
}

/// Shared path ordering for native identity callers and direct Rust VM leaves.
/// The callback must not retain a borrow of any engine storage across reentry.
pub(crate) fn path(
    object: usize,
    stop: usize,
    mut call: impl FnMut(Hp2IdentityLeaf, usize, usize) -> Result<usize>,
) -> Result<()> {
    use Hp2IdentityLeaf::*;
    if object == stop {
        call(None, 0, 0)?;
    } else {
        call(Clear, 0, 0)?;
        let outer = call(Outer, object, 0)?;
        if outer != 0 && outer != stop {
            call(PathCall, outer, stop)?;
            call(AppendDot, 0, 0)?;
        }
        call(AppendName, object, 0)?;
    }
    Ok(())
}

/// Shares the source class-counter, trailing-digit and collision policy with
/// canonical construction without retaining object storage across callbacks.
pub(crate) fn unique<E>(
    object_class: usize,
    parent: usize,
    mut call: impl FnMut(Hp2IdentityLeaf, usize, usize) -> Result<usize, E>,
) -> Result<usize, E> {
    use Hp2IdentityLeaf::*;
    call(CheckClass, object_class, 0)?;
    call(CopyBase, object_class, 0)?;
    let mut end = call(BaseLength, 0, 0)?;
    while end > 0 && call(BaseDigit, end - 1, 0)? != 0 {
        end -= 1;
    }
    call(TerminateBase, end, 0)?;
    loop {
        let count = call(Counter, object_class, 0)? as i32;
        call(SetCounter, object_class, count.wrapping_add(1) as usize)?;
        call(FormatUnique, count as usize, 0)?;
        if call(FindUnique, parent, 0)? == 0 { break; }
    }
    call(InternUnique, 0, 0)
}

/// # Safety
/// Services and all engine handles must remain live for synchronous reentry.
pub unsafe fn execute(
    services: &Hp2IdentityServices,
    op: Hp2IdentityOperation,
    q: &Hp2IdentityArgs,
) -> Result<usize> {
    use Hp2IdentityLeaf::*;
    let n = Native(services);
    match op {
        Hp2IdentityOperation::Rename => {
            let name = if q.name != 0 {
                n.call(InternName, q.name, 0)?
            } else {
                let outer = n.call(Outer, q.object, 0)?;
                let class = n.call(Class, q.object, 0)?;
                n.call(UniqueCall, outer, class)?
            };
            n.call(Unhash, q.object, 0)?;
            n.call(LogRename, q.object, name)?;
            n.call(AssignName, q.object, name)?;
            n.call(Hash, q.object, 0)?;
        }
        Hp2IdentityOperation::Path => path(q.object, q.stop, |op, a, b| n.call(op, a, b))?,
        Hp2IdentityOperation::Full => {
            if q.object == 0 {
                n.call(None, 0, 0)?;
            } else {
                n.call(ClassPrefix, q.object, 0)?;
                n.call(PathCall, q.object, 0)?;
            }
        }
        Hp2IdentityOperation::Unique => {
            return unique(q.object_class, q.parent, |op, a, b| n.call(op, a, b));
        }
    }
    Ok(0)
}
