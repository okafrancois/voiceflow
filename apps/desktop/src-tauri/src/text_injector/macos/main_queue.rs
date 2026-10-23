//! Synchronous macOS main-queue execution for clipboard operations.

use std::ffi::c_void;

type DispatchQueue = *const c_void;

extern "C" {
    fn pthread_main_np() -> i32;
    static _dispatch_main_q: u8;
    fn dispatch_sync_f(
        queue: DispatchQueue,
        context: *mut c_void,
        work: unsafe extern "C" fn(*mut c_void),
    );
}

pub(super) unsafe fn run_on_main_sync<T: Send, F: FnOnce() -> T + Send>(f: F) -> T {
    // Native menu callbacks already run on the main thread. Dispatching
    // synchronously to that same serial queue would abort the process.
    if pthread_main_np() != 0 {
        return f();
    }

    struct Ctx<T, F> {
        f: Option<F>,
        result: Option<T>,
    }
    extern "C" fn trampoline<T, F: FnOnce() -> T>(ctx: *mut c_void) {
        unsafe {
            let ctx = &mut *(ctx as *mut Ctx<T, F>);
            if let Some(f) = ctx.f.take() {
                ctx.result = Some(f());
            }
        }
    }
    let mut ctx = Ctx {
        f: Some(f),
        result: None,
    };
    let main_queue = &_dispatch_main_q as *const u8 as DispatchQueue;
    dispatch_sync_f(
        main_queue,
        &mut ctx as *mut _ as *mut c_void,
        trampoline::<T, F>,
    );
    ctx.result.expect("run_on_main_sync failed")
}
