//! Scope the fullscreen activation-policy workaround to this status menu.

mod policy;

use std::sync::Arc;

use cocoa::{
    base::{id, nil},
    foundation::NSString,
};
use objc::{class, msg_send, sel, sel_impl};
use parking_lot::Mutex;
use tauri::{tray::TrayIcon, AppHandle, Manager};
use tracing::warn;

use policy::MenuActivationPolicy;

struct Registration {
    // NotificationCenter retains these tokens. Their addresses cross the
    // Tauri boundary, but are dereferenced and removed only on the main thread.
    tokens: Vec<usize>,
    policy: Arc<MenuActivationPolicy>,
}

impl Registration {
    unsafe fn remove(self) {
        let center: id = msg_send![class!(NSNotificationCenter), defaultCenter];
        for token in self.tokens {
            let _: () = msg_send![center, removeObserver: token as id];
        }
        self.policy.end();
    }
}

#[derive(Default)]
struct MenuPolicyState(Mutex<Option<Registration>>);

pub(super) fn install(tray: &TrayIcon) -> tauri::Result<()> {
    let app = tray.app_handle().clone();
    tray.with_inner_tray_icon(move |inner| unsafe {
        let item = inner.ns_status_item().ok_or_else(|| {
            std::io::Error::other("macOS status item is unavailable")
        })?;
        let item_ptr = &*item as *const _ as id;
        let menu: id = msg_send![item_ptr, menu];
        if menu == nil {
            return Err(std::io::Error::other("macOS status menu is unavailable"));
        }

        let center: id = msg_send![class!(NSNotificationCenter), defaultCenter];
        let mut registration = Registration {
            tokens: Vec::with_capacity(2),
            policy: Arc::new(MenuActivationPolicy::default()),
        };
        for (name, opening) in [
            ("NSMenuDidBeginTrackingNotification", true),
            ("NSMenuDidEndTrackingNotification", false),
        ] {
            let name = NSString::alloc(nil).init_str(name);
            let policy = registration.policy.clone();
            let callback = block::ConcreteBlock::new(move |_notification: id| {
                if opening {
                    policy.begin();
                } else {
                    policy.end();
                }
            }).copy();
            // Filter by the exact NSMenu so application menus and other tray
            // items cannot alter this policy lease. A nil queue keeps delivery
            // synchronous on AppKit's menu-tracking thread.
            let token: id = msg_send![center, addObserverForName: name object: menu queue: nil usingBlock: &*callback];
            let _: () = msg_send![name, release];
            if token == nil {
                registration.remove();
                return Err(std::io::Error::other("macOS menu observer registration failed"));
            }
            registration.tokens.push(token as usize);
        }

        if app.try_state::<MenuPolicyState>().is_none() {
            app.manage(MenuPolicyState::default());
        }
        let previous = app.state::<MenuPolicyState>().0.lock().replace(registration);
        if let Some(previous) = previous {
            previous.remove();
        }
        Ok(())
    })??;
    Ok(())
}

pub(super) fn cleanup(app: &AppHandle) {
    let registration = app
        .try_state::<MenuPolicyState>()
        .and_then(|state| state.0.lock().take());
    if let Some(registration) = registration {
        if let Err(error) = app.run_on_main_thread(move || unsafe { registration.remove() }) {
            warn!(error = %error, "tray_menu_observer_cleanup_failed");
        }
    }
}
