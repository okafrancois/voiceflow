#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InjectionMethod {
    Keyboard,
    Clipboard,
}

pub trait TextInjector: Send + Sync {
    /// Insert `text` at the current cursor position.
    fn insert(&self, text: &str) -> Result<InjectionMethod, String>;
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
