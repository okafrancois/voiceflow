//! Exercise the real AppKit policy lifecycle on the process's main thread.
//! No windows, clipboard operations, keyboard input, or activation are used.

// The existing Objective-C bridge macros emit these third-party warnings.
#![allow(unexpected_cfgs, deprecated)]

#[cfg(target_os = "macos")]
#[path = "../src/tray/macos_menu/policy.rs"]
mod macos_menu;

#[cfg(target_os = "macos")]
fn main() {
    use cocoa::base::id;
    use objc::{class, msg_send, sel, sel_impl};

    #[link(name = "AppKit", kind = "framework")]
    extern "C" {}

    unsafe {
        let app: id = msg_send![class!(NSApplication), sharedApplication];
        let original: isize = msg_send![app, activationPolicy];
        if original != 0 {
            let changed: bool = msg_send![app, setActivationPolicy: 0_isize];
            assert!(changed);
        }
        let active: bool = msg_send![app, isActive];
        assert!(!active, "the probe must not activate its application");

        let policy = macos_menu::MenuActivationPolicy::default();
        policy.begin();
        let current: isize = msg_send![app, activationPolicy];
        assert_eq!(
            current, 1,
            "a background status menu must use Accessory policy"
        );
        policy.begin();
        policy.end();
        let current: isize = msg_send![app, activationPolicy];
        assert_eq!(
            current, 0,
            "menu dismissal must restore the original policy"
        );
        policy.end();
        let current: isize = msg_send![app, activationPolicy];
        assert_eq!(current, 0, "duplicate dismissal must be harmless");
        println!("PASS: background menu policy changes and restores exactly once");

        let changed: bool = msg_send![app, setActivationPolicy: 1_isize];
        assert!(changed);
        policy.begin();
        policy.end();
        let current: isize = msg_send![app, activationPolicy];
        assert_eq!(current, 1, "an existing Accessory policy must be preserved");
        println!("PASS: existing Accessory policy remains unchanged");

        let changed: bool = msg_send![app, setActivationPolicy: 0_isize];
        assert!(changed);
        policy.begin();
        let changed: bool = msg_send![app, setActivationPolicy: 2_isize];
        assert!(changed);
        policy.end();
        let current: isize = msg_send![app, activationPolicy];
        assert_eq!(
            current, 2,
            "an explicit intervening policy change must be preserved"
        );
        println!("PASS: dismissal preserves an intervening explicit policy change");

        let current: isize = msg_send![app, activationPolicy];
        if current != original {
            let changed: bool = msg_send![app, setActivationPolicy: original];
            assert!(changed);
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("macOS status-menu policy regression is only applicable on macOS");
}
