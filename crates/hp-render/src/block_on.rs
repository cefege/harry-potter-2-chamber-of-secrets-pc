//! Minimal `std`-only executor for the handful of async wgpu entry points
//! (adapter/device acquisition, buffer mapping callbacks).
//!
//! wgpu 30 exposes these as futures; the renderer is a synchronous engine
//! subsystem and adding an async runtime for three call sites would be
//! disproportionate. This blocks the calling thread with a park/unpark
//! waker — sufficient because wgpu completes its setup work on other
//! threads or inline.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, RawWaker, RawWakerVTable, Waker};
use std::thread::{self, Thread};

struct ThreadParker {
    thread: Thread,
    notified: AtomicBool,
}

// Safety: Thread and AtomicBool are both Send + Sync.
unsafe fn waker_into_raw(parker: Arc<ThreadParker>) -> RawWaker {
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone_fn, wake_fn, wake_by_ref_fn, drop_fn);

    unsafe fn clone_fn(data: *const ()) -> RawWaker {
        let arc = unsafe { Arc::from_raw(data as *const ThreadParker) };
        let cloned = arc.clone();
        std::mem::forget(arc);
        RawWaker::new(Arc::into_raw(cloned) as *const (), &VTABLE)
    }

    unsafe fn wake_fn(data: *const ()) {
        let arc = unsafe { Arc::from_raw(data as *const ThreadParker) };
        arc.notified.store(true, Ordering::Release);
        arc.thread.unpark();
    }

    unsafe fn wake_by_ref_fn(data: *const ()) {
        let arc = unsafe { Arc::from_raw(data as *const ThreadParker) };
        arc.notified.store(true, Ordering::Release);
        arc.thread.unpark();
        std::mem::forget(arc);
    }

    unsafe fn drop_fn(data: *const ()) {
        drop(unsafe { Arc::from_raw(data as *const ThreadParker) });
    }

    RawWaker::new(Arc::into_raw(parker) as *const (), &VTABLE)
}

/// Drive `future` to completion on the current thread.
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let parker = Arc::new(ThreadParker {
        thread: thread::current(),
        notified: AtomicBool::new(false),
    });
    let waker = unsafe { Waker::from_raw(waker_into_raw(parker.clone())) };
    let mut cx = Context::from_waker(&waker);
    loop {
        if let std::task::Poll::Ready(output) = future.as_mut().poll(&mut cx) {
            return output;
        }
        while !parker.notified.swap(false, Ordering::AcqRel) {
            thread::park();
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn resolves_ready_and_pending_futures() {
        assert_eq!(super::block_on(async { 40 + 2 }), 42);
        assert_eq!(
            super::block_on(std::future::ready(String::from("ok"))),
            "ok"
        );
    }
}
