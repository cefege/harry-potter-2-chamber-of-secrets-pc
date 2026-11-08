//! Reentrant VM local bytes, owned by Rust for each synchronous call/event.
//! Buffers retain their capacity per nesting depth until thread exit. Only the
//! current, unborrowed buffer may grow; suspended backing blocks never move.

use std::cell::RefCell;
use std::mem::{MaybeUninit, size_of};

#[repr(C, align(16))]
struct LocalWord {
    _bytes: [u8; 16],
}

struct Pool {
    buffers: Vec<Vec<MaybeUninit<LocalWord>>>,
    depth: usize,
}

thread_local! {
    static LOCALS: RefCell<Pool> = const {
        RefCell::new(Pool { buffers: Vec::new(), depth: 0 })
    };
}

struct Scope(usize);

impl Drop for Scope {
    fn drop(&mut self) {
        LOCALS.with(|pool| {
            let mut pool = pool.borrow_mut();
            debug_assert_eq!(pool.depth, self.0 + 1);
            pool.depth = self.0;
        });
    }
}

/// Run a synchronous continuation with `size` uninitialized, 16-byte-aligned
/// bytes. Even a zero-byte scope has a nonnull aligned backing address. The
/// pointer remains stable through nested scopes, but must not be dereferenced
/// after the continuation returns or unwinds. Initialize bytes before reading.
///
/// No pool borrow spans the continuation. Repeated scopes at warmed-up sizes
/// and depths allocate nothing; growing metadata never moves live byte blocks.
/// Rust's allocator owns every block, including thread-exit cleanup; no native
/// allocator service or engine object is retained in TLS.
pub fn with_locals<R>(size: usize, body: impl FnOnce(*mut u8) -> R) -> R {
    let (depth, data) = LOCALS.with(|pool| {
        let mut pool = pool.borrow_mut();
        let depth = pool.depth;
        if depth == pool.buffers.len() {
            pool.buffers.push(Vec::new());
        }
        let words = size.div_ceil(size_of::<LocalWord>()).max(1);
        let buffer = &mut pool.buffers[depth];
        if buffer.len() < words {
            buffer.resize_with(words, MaybeUninit::uninit);
        }
        let data = buffer.as_mut_ptr().cast();
        pool.depth += 1;
        (depth, data)
    });
    let _scope = Scope(depth);
    body(data)
}
