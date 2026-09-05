#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn send_enter() -> Result<(), String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};
    let mut keyboard = Enigo::new(&Settings::default()).map_err(|error| error.to_string())?;
    keyboard
        .key(Key::Return, Direction::Click)
        .map_err(|error| error.to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn send_enter() -> Result<(), String> {
    Err("Quick controls are unavailable on this platform".to_string())
}
