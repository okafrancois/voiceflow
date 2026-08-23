#![allow(unexpected_cfgs)]

use std::process::Command;

use super::{
    permission_request_flow, PermissionKind, PermissionProvider, PermissionRequestFlow,
    PermissionStatus,
};

const MICROPHONE_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone";
const SCREEN_RECORDING_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture";

fn open_settings(url: &str) -> Result<(), String> {
    Command::new("open")
        .arg(url)
        .spawn()
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn request_microphone_access() {
    use objc::{class, msg_send, sel, sel_impl};

    #[link(name = "AVFoundation", kind = "framework")]
    extern "C" {}

    unsafe {
        let media_type: *mut objc::runtime::Object = msg_send![
            class!(NSString),
            stringWithUTF8String: c"soun".as_ptr()
        ];
        extern crate block;
        let completion = block::ConcreteBlock::new(move |granted: objc::runtime::BOOL| {
            tracing::info!(
                permission = "microphone",
                granted = granted == objc::runtime::YES,
                "app_permission_request_completed"
            );
        });
        let completion = completion.copy();
        let _: () = msg_send![
            class!(AVCaptureDevice),
            requestAccessForMediaType: media_type
            completionHandler: &*completion
        ];
    }
}

fn request_screen_recording_access() -> bool {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGRequestScreenCaptureAccess() -> bool;
    }

    unsafe { CGRequestScreenCaptureAccess() }
}

pub struct MacosPermissions;

impl PermissionProvider for MacosPermissions {
    fn check_accessibility(&self) -> PermissionStatus {
        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            fn AXIsProcessTrusted() -> bool;
        }
        if unsafe { AXIsProcessTrusted() } {
            PermissionStatus::Granted
        } else {
            PermissionStatus::Denied
        }
    }

    fn check_input_monitoring(&self) -> PermissionStatus {
        #[link(name = "IOKit", kind = "framework")]
        extern "C" {
            fn IOHIDCheckAccess(request_type: u32) -> u32;
        }
        if unsafe { IOHIDCheckAccess(0) } == 0 {
            PermissionStatus::Granted
        } else {
            PermissionStatus::Denied
        }
    }

    fn check_microphone(&self) -> PermissionStatus {
        use objc::{class, msg_send, sel, sel_impl};

        #[link(name = "AVFoundation", kind = "framework")]
        extern "C" {}

        unsafe {
            let media_type: *mut objc::runtime::Object = msg_send![
                class!(NSString),
                stringWithUTF8String: c"soun".as_ptr()
            ];
            // AVAuthorizationStatus: 0=notDetermined, 1=restricted, 2=denied, 3=authorized
            let status: i64 = msg_send![
                class!(AVCaptureDevice),
                authorizationStatusForMediaType: media_type
            ];
            match status {
                3 => PermissionStatus::Granted,
                2 | 1 => PermissionStatus::Denied,
                _ => PermissionStatus::NotDetermined,
            }
        }
    }

    fn check_screen_recording(&self) -> PermissionStatus {
        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGPreflightScreenCaptureAccess() -> bool;
        }

        if unsafe { CGPreflightScreenCaptureAccess() } {
            PermissionStatus::Granted
        } else {
            PermissionStatus::Denied
        }
    }

    fn apply_accessibility(&self) -> Result<(), String> {
        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> bool;
        }
        #[link(name = "CoreFoundation", kind = "framework")]
        extern "C" {
            fn CFDictionaryCreate(
                allocator: *const std::ffi::c_void,
                keys: *const *const std::ffi::c_void,
                values: *const *const std::ffi::c_void,
                num_values: isize,
                key_callbacks: *const std::ffi::c_void,
                value_callbacks: *const std::ffi::c_void,
            ) -> *const std::ffi::c_void;
            static kCFBooleanTrue: *const std::ffi::c_void;
            static kAXTrustedCheckOptionPrompt: *const std::ffi::c_void;
        }
        unsafe {
            let keys = [kAXTrustedCheckOptionPrompt];
            let values = [kCFBooleanTrue];
            let dict = CFDictionaryCreate(
                std::ptr::null(),
                keys.as_ptr(),
                values.as_ptr(),
                1,
                std::ptr::null(),
                std::ptr::null(),
            );
            AXIsProcessTrustedWithOptions(dict);
        }
        Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn()
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn apply_input_monitoring(&self) -> Result<(), String> {
        Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent")
            .spawn()
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn apply_microphone(&self) -> Result<(), String> {
        match permission_request_flow(PermissionKind::Microphone, self.check_microphone()) {
            PermissionRequestFlow::Request => request_microphone_access(),
            PermissionRequestFlow::OpenSettings => open_settings(MICROPHONE_SETTINGS_URL)?,
            PermissionRequestFlow::RequestThenOpenSettingsIfDenied => unreachable!(),
        }
        Ok(())
    }

    fn apply_screen_recording(&self) -> Result<(), String> {
        match permission_request_flow(
            PermissionKind::ScreenRecording,
            self.check_screen_recording(),
        ) {
            PermissionRequestFlow::RequestThenOpenSettingsIfDenied => {
                if !request_screen_recording_access() {
                    open_settings(SCREEN_RECORDING_SETTINGS_URL)?;
                }
            }
            PermissionRequestFlow::OpenSettings => open_settings(SCREEN_RECORDING_SETTINGS_URL)?,
            PermissionRequestFlow::Request => unreachable!(),
        }
        Ok(())
    }
}
