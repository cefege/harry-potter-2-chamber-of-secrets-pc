//! Ordered static package bootstrap policy from `Launch/Src/HP2StaticPackages.cpp`.
//!
//! C++ retains native function-pointer identities and registration macros. Rust
//! owns the lookup-table reset/fill/termination rules and registration order.
use std::ffi::c_void;

#[derive(Debug)]
pub struct Thrown;
type R<T> = Result<T, Thrown>;

#[repr(C)]
pub struct Hp2StaticPackagesServices {
    pub lookup_table_count: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    pub lookup_table_clear: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub lookup_table_set_selector: unsafe extern "C" fn(*mut c_void, i32, i32) -> i32,
    pub lookup_table_is_null: unsafe extern "C" fn(*mut c_void, i32, *mut i32) -> i32,
    pub report_error: unsafe extern "C" fn(*mut c_void, i32, i32) -> i32,
    pub register_runtime: unsafe extern "C" fn(*mut c_void, i32) -> i32,
    pub register_client: unsafe extern "C" fn(*mut c_void, i32) -> i32,
}

const LOOKUP_COUNT: i32 = 1;
const ERROR_LOOKUP_OVERFLOW: i32 = 0;
const ERROR_LOOKUP_COUNT: i32 = 1;
const ERROR_LOOKUP_NOT_TERMINATED: i32 = 2;

unsafe fn st(status: i32) -> R<()> {
    if status == 0 { Ok(()) } else { Err(Thrown) }
}

unsafe fn out_i32(
    f: unsafe extern "C" fn(*mut c_void, *mut i32) -> i32,
    context: *mut c_void,
) -> R<i32> {
    let mut value = 0;
    unsafe {
        st(f(context, &mut value))?;
    }
    Ok(value)
}

/// Clears, fills, and validates `GNativeLookupFuncs` in the original lookup order.
///
/// # Safety
/// `services` and `context` must remain valid for the calls. C++ leaf failures are
/// represented by nonzero status and never unwind through Rust.
pub unsafe fn install_native_lookups(
    services: &Hp2StaticPackagesServices,
    context: *mut c_void,
) -> R<()> {
    let capacity = unsafe { out_i32(services.lookup_table_count, context)? };
    for index in 0..capacity {
        unsafe {
            st((services.lookup_table_clear)(context, index))?;
        }
    }

    let mut lookup = 0;
    for selector in 0..LOOKUP_COUNT {
        if lookup >= capacity {
            unsafe {
                st((services.report_error)(
                    context,
                    ERROR_LOOKUP_OVERFLOW,
                    lookup,
                ))?;
            }
            return Err(Thrown);
        }
        unsafe {
            st((services.lookup_table_set_selector)(
                context, lookup, selector,
            ))?;
        }
        lookup += 1;
    }

    if lookup != LOOKUP_COUNT {
        unsafe {
            st((services.report_error)(context, ERROR_LOOKUP_COUNT, lookup))?;
        }
        return Err(Thrown);
    }
    if lookup >= capacity {
        unsafe {
            st((services.report_error)(
                context,
                ERROR_LOOKUP_NOT_TERMINATED,
                lookup,
            ))?;
        }
        return Err(Thrown);
    }
    let mut is_null = 0;
    unsafe {
        st((services.lookup_table_is_null)(
            context,
            lookup,
            &mut is_null,
        ))?;
    }
    if is_null == 0 {
        unsafe {
            st((services.report_error)(
                context,
                ERROR_LOOKUP_NOT_TERMINATED,
                lookup,
            ))?;
        }
        return Err(Thrown);
    }
    Ok(())
}

/// Runs the Engine, Render, Fire, then editor-transaction registration leaves.
///
/// # Safety
/// The native registration leaves must accept selectors 0 through 3.
pub unsafe fn register_runtime_classes(
    services: &Hp2StaticPackagesServices,
    context: *mut c_void,
) -> R<()> {
    for selector in 0..4 {
        unsafe {
            st((services.register_runtime)(context, selector))?;
        }
    }
    Ok(())
}

/// Runs SDL, audio, then WGPU client registration leaves.
///
/// # Safety
/// Available only in a client-package build; the C++ caller preserves that gate.
pub unsafe fn register_client_classes(
    services: &Hp2StaticPackagesServices,
    context: *mut c_void,
) -> R<()> {
    for selector in 0..3 {
        unsafe {
            st((services.register_client)(context, selector))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Mock {
        table: Vec<Option<i32>>,
        clears: Vec<i32>,
        sets: Vec<(i32, i32)>,
        errors: Vec<(i32, i32)>,
        runtime: Vec<i32>,
        client: Vec<i32>,
    }

    unsafe fn mock<'a>(context: *mut c_void) -> &'a mut Mock {
        unsafe { &mut *context.cast() }
    }
    unsafe extern "C" fn count(context: *mut c_void, out: *mut i32) -> i32 {
        unsafe {
            *out = mock(context).table.len() as i32;
        }
        0
    }
    unsafe extern "C" fn clear(context: *mut c_void, index: i32) -> i32 {
        let m = unsafe { mock(context) };
        m.table[index as usize] = None;
        m.clears.push(index);
        0
    }
    unsafe extern "C" fn set(context: *mut c_void, index: i32, selector: i32) -> i32 {
        let m = unsafe { mock(context) };
        m.table[index as usize] = Some(selector);
        m.sets.push((index, selector));
        0
    }
    unsafe extern "C" fn is_null(context: *mut c_void, index: i32, out: *mut i32) -> i32 {
        unsafe {
            *out = i32::from(mock(context).table[index as usize].is_none());
        }
        0
    }
    unsafe extern "C" fn error(context: *mut c_void, code: i32, value: i32) -> i32 {
        unsafe {
            mock(context).errors.push((code, value));
        }
        1
    }
    unsafe extern "C" fn runtime(context: *mut c_void, selector: i32) -> i32 {
        unsafe {
            mock(context).runtime.push(selector);
        }
        0
    }
    unsafe extern "C" fn client(context: *mut c_void, selector: i32) -> i32 {
        unsafe {
            mock(context).client.push(selector);
        }
        0
    }

    const SVC: Hp2StaticPackagesServices = Hp2StaticPackagesServices {
        lookup_table_count: count,
        lookup_table_clear: clear,
        lookup_table_set_selector: set,
        lookup_table_is_null: is_null,
        report_error: error,
        register_runtime: runtime,
        register_client: client,
    };

    #[test]
    fn preserves_lookup_reset_order_termination_and_registration_order() {
        let mut mock = Mock {
            table: vec![Some(-1); 32],
            ..Default::default()
        };
        unsafe {
            install_native_lookups(&SVC, (&mut mock as *mut Mock).cast()).unwrap();
            register_runtime_classes(&SVC, (&mut mock as *mut Mock).cast()).unwrap();
            register_client_classes(&SVC, (&mut mock as *mut Mock).cast()).unwrap();
        }
        assert_eq!(mock.clears, (0..32).collect::<Vec<_>>());
        assert_eq!(
            mock.sets,
            (0..LOOKUP_COUNT).map(|i| (i, i)).collect::<Vec<_>>()
        );
        assert_eq!(
            &mock.table[..LOOKUP_COUNT as usize],
            (0..LOOKUP_COUNT).map(Some).collect::<Vec<_>>().as_slice()
        );
        assert_eq!(mock.table[LOOKUP_COUNT as usize], None);
        assert!(mock.errors.is_empty());
        assert_eq!(mock.runtime, vec![0, 1, 2, 3]);
        assert_eq!(mock.client, vec![0, 1, 2]);
    }

    #[test]
    fn reports_overflow_before_selecting_an_out_of_bounds_lookup() {
        let mut mock = Mock {
            table: vec![None; 0],
            ..Default::default()
        };
        assert!(unsafe { install_native_lookups(&SVC, (&mut mock as *mut Mock).cast()) }.is_err());
        assert_eq!(mock.sets, Vec::<(i32, i32)>::new());
        assert_eq!(mock.errors, vec![(ERROR_LOOKUP_OVERFLOW, 0)]);
    }
}
