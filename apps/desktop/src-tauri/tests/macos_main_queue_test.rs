//! A custom harness keeps the regression on the process's real main thread.
//! No clipboard access, keyboard events, or application activation occurs.

#[cfg(target_os = "macos")]
#[path = "../src/text_injector/macos/main_queue.rs"]
mod main_queue;

#[cfg(target_os = "macos")]
fn main() {
    use std::ffi::c_void;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    extern "C" {
        fn pthread_main_np() -> i32;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        static kCFRunLoopDefaultMode: *const c_void;
        fn CFRunLoopRunInMode(mode: *const c_void, seconds: f64, return_after_source: u8) -> i32;
    }

    assert_ne!(unsafe { pthread_main_np() }, 0);
    let mut calls = 0;
    let value = unsafe {
        main_queue::run_on_main_sync(|| {
            assert_ne!(pthread_main_np(), 0);
            calls += 1;
            // A reentrant call is safe while already executing main-queue work.
            main_queue::run_on_main_sync(|| "main-thread result".to_owned())
        })
    };
    assert_eq!(value, "main-thread result");
    assert_eq!(calls, 1);
    println!("PASS: main-thread calls execute exactly once without self-dispatch");

    let (sender, receiver) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        assert_eq!(unsafe { pthread_main_np() }, 0);
        let value = unsafe {
            main_queue::run_on_main_sync(|| {
                assert_ne!(pthread_main_np(), 0);
                main_queue::run_on_main_sync(|| "worker result".to_owned())
            })
        };
        sender.send(value).unwrap();
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    let value = loop {
        match receiver.try_recv() {
            Ok(value) => break value,
            Err(mpsc::TryRecvError::Disconnected) => panic!("main-queue worker disconnected"),
            Err(mpsc::TryRecvError::Empty) => {}
        }
        assert!(Instant::now() < deadline, "main-queue worker timed out");
        unsafe { CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.01, 1) };
    };
    worker.join().unwrap();
    assert_eq!(value, "worker result");
    println!("PASS: background calls execute on the main thread and return their value");
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("macOS main-queue regression is only applicable on macOS");
}
