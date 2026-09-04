#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectionMethod {
    Keyboard,
    Clipboard,
    Accessibility,
}

pub trait TextInjector: Send + Sync {
    /// Insert `text` at the current cursor position.
    fn insert(&self, text: &str) -> Result<InjectionMethod, String>;
}

pub struct CapturedTextTarget {
    application_id: String,
    #[cfg(target_os = "macos")]
    accessibility: Option<macos::AccessibilityTarget>,
}

impl CapturedTextTarget {
    pub fn capture(application_id: String, capture_accessibility: bool) -> Self {
        #[cfg(not(target_os = "macos"))]
        let _ = capture_accessibility;
        Self {
            application_id,
            #[cfg(target_os = "macos")]
            accessibility: capture_accessibility
                .then(macos::AccessibilityTarget::capture)
                .flatten(),
        }
    }

    pub fn application_id(&self) -> &str {
        &self.application_id
    }

    pub fn insert_background(&self, text: &str) -> Result<InjectionMethod, String> {
        #[cfg(target_os = "macos")]
        {
            let target = self
                .accessibility
                .as_ref()
                .ok_or_else(|| "Original editable field was not captured".to_string())?;
            target.insert(text)?;
            Ok(InjectionMethod::Accessibility)
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = text;
            Err("Background original-target delivery is unavailable on this platform".to_string())
        }
    }
}

pub mod quick_controls;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(test, target_os = "windows"))]
mod windows;

pub fn create_injector() -> Box<dyn TextInjector> {
    #[cfg(target_os = "macos")]
    return Box::new(macos::MacosInjector);
    #[cfg(target_os = "windows")]
    return Box::new(windows::WindowsInjector);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    compile_error!("text_injector: unsupported platform");
}

pub fn insert_text(text: &str) -> Result<InjectionMethod, String> {
    let injector = create_injector();
    injector.insert(text)
}
