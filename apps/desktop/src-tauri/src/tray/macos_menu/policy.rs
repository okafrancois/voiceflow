//! Keep background status menus eligible for another app's fullscreen Space.

use std::sync::atomic::{AtomicIsize, Ordering};

use cocoa::base::id;
use objc::{class, msg_send, sel, sel_impl};
use tracing::{info, warn};

const NO_SAVED_POLICY: isize = -1;
const REGULAR: isize = 0;
const ACCESSORY: isize = 1;

pub(super) struct MenuActivationPolicy {
    saved: AtomicIsize,
}

impl Default for MenuActivationPolicy {
    fn default() -> Self {
        Self {
            saved: AtomicIsize::new(NO_SAVED_POLICY),
        }
    }
}

impl MenuActivationPolicy {
    /// Call only on the AppKit main thread, immediately before menu tracking.
    pub(super) unsafe fn begin(&self) {
        let app: id = msg_send![class!(NSApplication), sharedApplication];
        let current: isize = msg_send![app, activationPolicy];
        let active: bool = msg_send![app, isActive];
        if current != REGULAR || active {
            return;
        }
        if self
            .saved
            .compare_exchange(NO_SAVED_POLICY, current, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        // NSMenu tracking has not presented its windows yet. Accessory policy
        // lets those windows join another application's fullscreen Space.
        let changed: bool = msg_send![app, setActivationPolicy: ACCESSORY];
        if changed {
            info!("tray_menu_accessory_policy_applied");
        } else {
            self.saved.store(NO_SAVED_POLICY, Ordering::SeqCst);
            warn!("tray_menu_accessory_policy_failed");
        }
    }

    /// Call only on the AppKit main thread, when menu tracking ends.
    pub(super) unsafe fn end(&self) {
        let original = self.saved.swap(NO_SAVED_POLICY, Ordering::SeqCst);
        if original == NO_SAVED_POLICY {
            return;
        }
        let app: id = msg_send![class!(NSApplication), sharedApplication];
        let current: isize = msg_send![app, activationPolicy];
        if current != ACCESSORY {
            return;
        }
        let changed: bool = msg_send![app, setActivationPolicy: original];
        if changed {
            info!("tray_menu_activation_policy_restored");
        } else {
            warn!("tray_menu_activation_policy_restore_failed");
        }
    }
}
