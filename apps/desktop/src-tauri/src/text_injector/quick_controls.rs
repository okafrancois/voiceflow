trait QuickControlDriver {
    fn press_primary_modifier(&mut self) -> Result<(), String>;
    fn click_z(&mut self) -> Result<(), String>;
    fn release_primary_modifier(&mut self) -> Result<(), String>;
    fn click_enter(&mut self) -> Result<(), String>;
}

fn send_undo_with(driver: &mut dyn QuickControlDriver) -> Result<(), String> {
    driver.press_primary_modifier()?;
    let click_result = driver.click_z();
    let release_result = driver.release_primary_modifier();

    match (click_result, release_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(click_error), Ok(())) => Err(format!("Undo key failed: {click_error}")),
        (Ok(()), Err(release_error)) => Err(format!("Modifier release failed: {release_error}")),
        (Err(click_error), Err(release_error)) => Err(format!(
            "Undo key failed: {click_error}; modifier release failed: {release_error}"
        )),
    }
}

pub fn send_undo() -> Result<(), String> {
    let mut driver = SystemQuickControlDriver::new()?;
    send_undo_with(&mut driver)
}

pub fn send_enter() -> Result<(), String> {
    let mut driver = SystemQuickControlDriver::new()?;
    driver.click_enter()
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
struct SystemQuickControlDriver {
    keyboard: enigo::Enigo,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl SystemQuickControlDriver {
    fn new() -> Result<Self, String> {
        use enigo::Settings;
        Ok(Self {
            keyboard: enigo::Enigo::new(&Settings::default()).map_err(|error| error.to_string())?,
        })
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl QuickControlDriver for SystemQuickControlDriver {
    fn press_primary_modifier(&mut self) -> Result<(), String> {
        use enigo::{Direction, Keyboard};
        self.keyboard
            .key(primary_modifier(), Direction::Press)
            .map_err(|error| error.to_string())
    }

    fn click_z(&mut self) -> Result<(), String> {
        use enigo::{Direction, Key, Keyboard};
        self.keyboard
            .key(Key::Unicode('z'), Direction::Click)
            .map_err(|error| error.to_string())
    }

    fn release_primary_modifier(&mut self) -> Result<(), String> {
        use enigo::{Direction, Keyboard};
        self.keyboard
            .key(primary_modifier(), Direction::Release)
            .map_err(|error| error.to_string())
    }

    fn click_enter(&mut self) -> Result<(), String> {
        use enigo::{Direction, Key, Keyboard};
        self.keyboard
            .key(Key::Return, Direction::Click)
            .map_err(|error| error.to_string())
    }
}

#[cfg(target_os = "macos")]
fn primary_modifier() -> enigo::Key {
    enigo::Key::Meta
}

#[cfg(target_os = "windows")]
fn primary_modifier() -> enigo::Key {
    enigo::Key::Control
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
struct SystemQuickControlDriver;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
impl SystemQuickControlDriver {
    fn new() -> Result<Self, String> {
        Err("Quick controls are unavailable on this platform".to_string())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
impl QuickControlDriver for SystemQuickControlDriver {
    fn press_primary_modifier(&mut self) -> Result<(), String> {
        Err("Quick controls are unavailable on this platform".to_string())
    }

    fn click_z(&mut self) -> Result<(), String> {
        Err("Quick controls are unavailable on this platform".to_string())
    }

    fn release_primary_modifier(&mut self) -> Result<(), String> {
        Err("Quick controls are unavailable on this platform".to_string())
    }

    fn click_enter(&mut self) -> Result<(), String> {
        Err("Quick controls are unavailable on this platform".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeDriver {
        calls: Vec<&'static str>,
        fail_press: bool,
        fail_click: bool,
        fail_release: bool,
    }

    impl QuickControlDriver for FakeDriver {
        fn press_primary_modifier(&mut self) -> Result<(), String> {
            self.calls.push("press");
            if self.fail_press {
                Err("press".to_string())
            } else {
                Ok(())
            }
        }

        fn click_z(&mut self) -> Result<(), String> {
            self.calls.push("click");
            if self.fail_click {
                Err("click".to_string())
            } else {
                Ok(())
            }
        }

        fn release_primary_modifier(&mut self) -> Result<(), String> {
            self.calls.push("release");
            if self.fail_release {
                Err("release".to_string())
            } else {
                Ok(())
            }
        }

        fn click_enter(&mut self) -> Result<(), String> {
            self.calls.push("enter");
            Ok(())
        }
    }

    #[test]
    fn undo_releases_modifier_after_successful_click() {
        let mut driver = FakeDriver::default();
        send_undo_with(&mut driver).unwrap();
        assert_eq!(driver.calls, vec!["press", "click", "release"]);
    }

    #[test]
    fn undo_releases_modifier_when_click_fails() {
        let mut driver = FakeDriver {
            fail_click: true,
            ..FakeDriver::default()
        };
        let error = send_undo_with(&mut driver).unwrap_err();
        assert_eq!(driver.calls, vec!["press", "click", "release"]);
        assert!(error.contains("Undo key failed"));
    }

    #[test]
    fn undo_stops_when_modifier_press_fails() {
        let mut driver = FakeDriver {
            fail_press: true,
            ..FakeDriver::default()
        };
        assert!(send_undo_with(&mut driver).is_err());
        assert_eq!(driver.calls, vec!["press"]);
    }

    #[test]
    fn undo_reports_release_failure() {
        let mut driver = FakeDriver {
            fail_release: true,
            ..FakeDriver::default()
        };
        let error = send_undo_with(&mut driver).unwrap_err();
        assert_eq!(driver.calls, vec!["press", "click", "release"]);
        assert!(error.contains("Modifier release failed"));
    }
}
