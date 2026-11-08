//! Port of Core's `appSpc`: return a thread-local NUL-terminated run of spaces.

use std::cell::{Cell, UnsafeCell};

use crate::ansi::Tchar;

thread_local! {
    static SPACING: UnsafeCell<[Tchar; 256]> = const { UnsafeCell::new([0; 256]) };
    static LAST_NUM: Cell<i32> = const { Cell::new(-1) };
}

/// Returns the current thread's reusable NUL-terminated space buffer.
///
/// The pointer remains valid until the next `app_spc` call on this thread or
/// until the thread exits. Counts outside the legacy buffer's valid range are
/// bounded to 0..=255 to avoid out-of-bounds writes.
pub fn app_spc(num: i32) -> *const Tchar {
    let spaces = num.clamp(0, 255) as usize;
    SPACING.with(|spacing| {
        LAST_NUM.with(|last_num| {
            if last_num.get() != num {
                // SAFETY: this thread-local buffer is only mutated by this
                // function on the owning thread; callers receive it only as a
                // const pointer, valid until their next call or thread exit.
                unsafe {
                    let buffer = spacing.get();
                    for index in 0..spaces {
                        *(*buffer).as_mut_ptr().add(index) = 32 as Tchar;
                    }
                    *(*buffer).as_mut_ptr().add(spaces) = 0;
                    last_num.set(num);
                    (*buffer).as_ptr()
                }
            } else {
                // SAFETY: `spacing` points to this thread's static TLS array.
                unsafe { (*spacing.get()).as_ptr() }
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::{Tchar, app_spc};

    fn read(pointer: *const Tchar, limit: usize) -> Vec<Tchar> {
        let mut result = Vec::new();
        // SAFETY: `app_spc` returns a 256-element TLS array with a terminator.
        unsafe {
            for index in 0..limit {
                let value = *pointer.add(index);
                if value == 0 {
                    return result;
                }
                result.push(value);
            }
        }
        panic!("app_spc result was not terminated within {limit} TCHARs");
    }

    #[test]
    fn app_spc_contents_and_reuse() {
        assert_eq!(read(app_spc(3), 256), vec![32 as Tchar; 3]);
        assert_eq!(read(app_spc(3), 256), vec![32 as Tchar; 3]);
        assert!(read(app_spc(0), 256).is_empty());
        assert_eq!(read(app_spc(2), 256), vec![32 as Tchar; 2]);
    }

    #[test]
    fn app_spc_bounds_invalid_counts() {
        assert!(read(app_spc(-8), 256).is_empty());
        assert_eq!(read(app_spc(300), 256), vec![32 as Tchar; 255]);
    }

    #[test]
    fn app_spc_buffers_are_thread_local() {
        let main = app_spc(4);
        let worker = std::thread::spawn(|| {
            assert_eq!(read(app_spc(1), 256), vec![32 as Tchar]);
        });
        worker.join().unwrap();
        assert_eq!(read(main, 256), vec![32 as Tchar; 4]);
    }
}
