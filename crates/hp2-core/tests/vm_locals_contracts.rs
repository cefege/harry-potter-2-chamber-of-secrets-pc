use hp2_core::vm_locals::with_locals;
use std::panic::{AssertUnwindSafe, catch_unwind, panic_any};
use std::sync::Barrier;

unsafe fn filled(data: *mut u8, size: usize, value: u8) -> bool {
    unsafe { std::slice::from_raw_parts(data, size) }
        .iter()
        .all(|byte| *byte == value)
}

#[test]
fn nested_scopes_preserve_aligned_live_bytes_across_size_boundaries() {
    for _ in 0..4 {
        for size in [0, 1, 15, 16, 17, 4096, 262144] {
            with_locals(size, |outer| unsafe {
                assert!(!outer.is_null());
                assert_eq!(outer as usize % 16, 0);
                outer.write_bytes(0xA7, size);
                for nested_size in [0, 1, 17, 4096, 262144] {
                    with_locals(nested_size, |nested| {
                        assert!(!nested.is_null());
                        assert_eq!(nested as usize % 16, 0);
                        nested.write_bytes(0x3C, nested_size);
                        assert!(filled(outer, size, 0xA7));
                        assert!(filled(nested, nested_size, 0x3C));
                    });
                    assert!(filled(outer, size, 0xA7));
                }
            });
        }
    }
}

#[test]
fn nested_panic_preserves_the_suspended_scope_and_error_payload() {
    with_locals(65537, |outer| unsafe {
        outer.write_bytes(0x6D, 65537);
        let error = catch_unwind(AssertUnwindSafe(|| {
            with_locals(17, |middle| {
                middle.write_bytes(0x81, 17);
                with_locals(262144, |inner| {
                    inner.write_bytes(0x42, 262144);
                    assert!(filled(outer, 65537, 0x6D));
                    assert!(filled(middle, 17, 0x81));
                    panic_any(71_u32);
                });
            });
        }));
        assert_eq!(error.unwrap_err().downcast_ref::<u32>(), Some(&71));
        with_locals(262144, |replacement| {
            replacement.write_bytes(0xF2, 262144);
            assert!(filled(replacement, 262144, 0xF2));
            assert!(filled(outer, 65537, 0x6D));
        });
        assert!(filled(outer, 65537, 0x6D));
    });
}

#[test]
fn simultaneous_threads_keep_live_bytes_independent() {
    let barrier = Barrier::new(2);
    std::thread::scope(|scope| {
        let worker = scope.spawn(|| {
            with_locals(4097, |data| unsafe {
                barrier.wait();
                data.write_bytes(0x31, 4097);
                barrier.wait();
                barrier.wait();
                filled(data, 4097, 0x31)
            })
        });
        let main_preserved = with_locals(4097, |data| unsafe {
            data.write_bytes(0x72, 4097);
            barrier.wait();
            barrier.wait();
            let preserved = filled(data, 4097, 0x72);
            data.write_bytes(0x93, 4097);
            barrier.wait();
            preserved
        });
        let worker_preserved = worker.join().unwrap();
        assert!(main_preserved);
        assert!(worker_preserved);
    });
}
