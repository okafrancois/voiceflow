use crate::services::product_workflows::{
    CapturedContext, CapturedContextInput, ContextCaptureSettings, ContextSource,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ActiveWindowMetadata {
    application_id: Option<String>,
    application_name: Option<String>,
    window_title: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct FocusedFieldContext {
    role: Option<String>,
    selected_text: Option<String>,
}

pub async fn capture_focused_context(
    settings: &ContextCaptureSettings,
    clipboard_text: Option<String>,
) -> CapturedContext {
    let metadata_enabled = settings.application_metadata;
    let accessibility_enabled = settings.focused_field || settings.selected_text;
    let metadata_task = metadata_enabled
        .then(|| tauri::async_runtime::spawn_blocking(capture_active_window_metadata_blocking));
    let accessibility_task = accessibility_enabled
        .then(|| tauri::async_runtime::spawn_blocking(capture_focused_field_blocking));

    let metadata = match metadata_task {
        Some(task) => task.await.ok().unwrap_or_default(),
        None => ActiveWindowMetadata::default(),
    };
    let focused = match accessibility_task {
        Some(task) => task.await.ok().unwrap_or_default(),
        None => FocusedFieldContext::default(),
    };

    let mut sources = Vec::new();
    if metadata.application_id.is_some()
        || metadata.application_name.is_some()
        || metadata.window_title.is_some()
    {
        sources.push(ContextSource::WindowMetadata);
    }
    if focused.role.is_some() || focused.selected_text.is_some() {
        sources.push(ContextSource::Accessibility);
    }
    if settings.clipboard
        && clipboard_text
            .as_deref()
            .is_some_and(|text| !text.trim().is_empty())
    {
        sources.push(ContextSource::Clipboard);
    }

    let mut input = CapturedContextInput {
        application_id: metadata.application_id,
        application_name: metadata.application_name,
        window_title: metadata.window_title,
        focused_field_role: settings.focused_field.then_some(focused.role).flatten(),
        selected_text: settings
            .selected_text
            .then_some(focused.selected_text)
            .flatten(),
        clipboard_text: settings.clipboard.then_some(clipboard_text).flatten(),
        ocr_text: None,
        sources,
        captured_at_ms: chrono::Utc::now().timestamp_millis(),
    };

    if settings.ocr_fallback && input.selected_text.is_none() {
        if let Some(ocr) = super::window_context::capture_window_context().await {
            if input.window_title.is_none() {
                input.window_title = ocr.window_title;
            }
            input.ocr_text = Some(ocr.filtered_text);
            input.sources.push(ContextSource::Ocr);
        }
    }

    CapturedContext::new(input).filtered_by(settings)
}

pub fn capture_application_context() -> CapturedContext {
    let metadata = capture_active_window_metadata_blocking();
    let mut sources = Vec::new();
    if metadata.application_id.is_some()
        || metadata.application_name.is_some()
        || metadata.window_title.is_some()
    {
        sources.push(ContextSource::WindowMetadata);
    }
    CapturedContext::new(CapturedContextInput {
        application_id: metadata.application_id,
        application_name: metadata.application_name,
        window_title: metadata.window_title,
        sources,
        captured_at_ms: chrono::Utc::now().timestamp_millis(),
        ..CapturedContextInput::default()
    })
}

pub fn activate_application(application_id: &str) -> Result<(), String> {
    if capture_application_context().application_id.as_deref() == Some(application_id) {
        return Ok(());
    }
    activate_application_platform(application_id)
}

#[cfg(target_os = "macos")]
fn activate_application_platform(application_id: &str) -> Result<(), String> {
    macos::activate_application(application_id)
}

#[cfg(not(target_os = "macos"))]
fn activate_application_platform(_application_id: &str) -> Result<(), String> {
    Err("Source application activation is unavailable on this platform".to_string())
}

fn capture_active_window_metadata_blocking() -> ActiveWindowMetadata {
    let window = xcap::Window::all().ok().and_then(|windows| {
        windows.into_iter().find(|window| {
            !window.is_minimized().unwrap_or(true) && window.is_focused().unwrap_or(false)
        })
    });
    let Some(window) = window else {
        return active_application_without_window();
    };

    let process_id = window.pid().ok();
    ActiveWindowMetadata {
        application_id: platform_application_id(process_id),
        application_name: window
            .app_name()
            .ok()
            .filter(|name| !name.trim().is_empty()),
        window_title: window.title().ok().filter(|title| !title.trim().is_empty()),
    }
}

#[cfg(target_os = "macos")]
fn active_application_without_window() -> ActiveWindowMetadata {
    macos::active_application()
}

#[cfg(not(target_os = "macos"))]
fn active_application_without_window() -> ActiveWindowMetadata {
    ActiveWindowMetadata::default()
}

#[cfg(target_os = "macos")]
fn platform_application_id(process_id: Option<u32>) -> Option<String> {
    macos::application_id(process_id)
}

#[cfg(not(target_os = "macos"))]
fn platform_application_id(process_id: Option<u32>) -> Option<String> {
    process_id.map(|pid| format!("pid:{pid}"))
}

#[cfg(target_os = "macos")]
fn capture_focused_field_blocking() -> FocusedFieldContext {
    macos::focused_field()
}

#[cfg(not(target_os = "macos"))]
fn capture_focused_field_blocking() -> FocusedFieldContext {
    FocusedFieldContext::default()
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{ActiveWindowMetadata, FocusedFieldContext};
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSString;
    use objc::{class, msg_send, sel, sel_impl};
    use std::ffi::{c_void, CStr};
    use std::os::raw::c_char;
    use std::ptr;

    type AXUIElementRef = *const c_void;
    type CFStringRef = *const c_void;
    type CFTypeRef = *const c_void;

    const AX_ERROR_SUCCESS: i32 = 0;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXUIElementCreateSystemWide() -> AXUIElementRef;
        fn AXUIElementCopyAttributeValue(
            element: AXUIElementRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> i32;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: CFTypeRef);
    }

    pub(super) fn active_application() -> ActiveWindowMetadata {
        unsafe {
            let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
            let application: id = msg_send![workspace, frontmostApplication];
            if application.is_null() {
                return ActiveWindowMetadata::default();
            }
            let application_id = ns_optional_property(application, sel!(bundleIdentifier));
            let application_name = ns_optional_property(application, sel!(localizedName));
            ActiveWindowMetadata {
                application_id,
                application_name,
                window_title: None,
            }
        }
    }

    pub(super) fn application_id(process_id: Option<u32>) -> Option<String> {
        let pid = process_id?;
        unsafe {
            let application: id = msg_send![
                class!(NSRunningApplication),
                runningApplicationWithProcessIdentifier: pid as i32
            ];
            if application.is_null() {
                return Some(format!("pid:{pid}"));
            }
            ns_optional_property(application, sel!(bundleIdentifier))
                .or_else(|| Some(format!("pid:{pid}")))
        }
    }

    pub(super) fn activate_application(application_id: &str) -> Result<(), String> {
        unsafe {
            let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
            let applications: id = msg_send![workspace, runningApplications];
            let count: usize = msg_send![applications, count];
            for index in 0..count {
                let application: id = msg_send![applications, objectAtIndex: index];
                let matches = ns_optional_property(application, sel!(bundleIdentifier))
                    .is_some_and(|bundle_id| bundle_id == application_id);
                if matches {
                    let activated: bool = msg_send![application, activateWithOptions: 2u64];
                    return activated.then_some(()).ok_or_else(|| {
                        format!("Could not activate application: {application_id}")
                    });
                }
            }
        }
        Err(format!(
            "Source application is not running: {application_id}"
        ))
    }

    pub(super) fn focused_field() -> FocusedFieldContext {
        unsafe {
            let system = AXUIElementCreateSystemWide();
            if system.is_null() {
                return FocusedFieldContext::default();
            }
            let focused = copy_attribute(system, "AXFocusedUIElement");
            CFRelease(system as CFTypeRef);
            let Some(focused) = focused else {
                return FocusedFieldContext::default();
            };

            let context = FocusedFieldContext {
                role: copy_attribute_string(focused as AXUIElementRef, "AXRole"),
                selected_text: copy_attribute_string(focused as AXUIElementRef, "AXSelectedText")
                    .filter(|text| !text.trim().is_empty()),
            };
            CFRelease(focused);
            context
        }
    }

    unsafe fn copy_attribute(element: AXUIElementRef, attribute: &str) -> Option<CFTypeRef> {
        let attribute_name = NSString::alloc(nil).init_str(attribute);
        let mut value: CFTypeRef = ptr::null();
        let error =
            AXUIElementCopyAttributeValue(element, attribute_name as CFStringRef, &mut value);
        let _: () = msg_send![attribute_name, release];
        (error == AX_ERROR_SUCCESS && !value.is_null()).then_some(value)
    }

    unsafe fn copy_attribute_string(element: AXUIElementRef, attribute: &str) -> Option<String> {
        let value = copy_attribute(element, attribute)?;
        let text = ns_string(value as id);
        CFRelease(value);
        text
    }

    unsafe fn ns_optional_property(object: id, selector: objc::runtime::Sel) -> Option<String> {
        let responds: bool = msg_send![object, respondsToSelector: selector];
        if !responds {
            return None;
        }
        let value: id = msg_send![object, performSelector: selector];
        ns_string(value)
    }

    unsafe fn ns_string(value: id) -> Option<String> {
        if value.is_null() {
            return None;
        }
        let utf8: *const c_char = msg_send![value, UTF8String];
        if utf8.is_null() {
            return None;
        }
        CStr::from_ptr(utf8)
            .to_str()
            .ok()
            .map(str::to_string)
            .filter(|text| !text.trim().is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_context_settings_do_not_include_clipboard_or_ocr() {
        let settings = ContextCaptureSettings::default();
        let context = CapturedContext::new(CapturedContextInput {
            application_id: Some("com.example.Editor".to_string()),
            selected_text: Some("selected".to_string()),
            clipboard_text: Some("copied".to_string()),
            ocr_text: Some("visible".to_string()),
            sources: vec![
                ContextSource::WindowMetadata,
                ContextSource::Accessibility,
                ContextSource::Clipboard,
                ContextSource::Ocr,
            ],
            ..CapturedContextInput::default()
        })
        .filtered_by(&settings);

        assert_eq!(
            context.application_id.as_deref(),
            Some("com.example.Editor")
        );
        assert_eq!(context.selected_text.as_deref(), Some("selected"));
        assert!(context.clipboard_text.is_none());
        assert!(context.ocr_text.is_none());
    }
}
