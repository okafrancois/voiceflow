#![cfg_attr(all(test, not(target_os = "windows")), allow(dead_code))]

use std::time::Duration;

#[cfg(target_os = "windows")]
use enigo::Settings;
use enigo::{Direction, Enigo, Key, Keyboard};
#[cfg(target_os = "windows")]
use tracing::info;
use tracing::warn;

use super::InjectionMethod;

pub struct WindowsInjector;

const CHUNK_SIZE: usize = 100;
const CHUNK_DELAY_MS: u64 = 50;

#[cfg(target_os = "windows")]
impl super::TextInjector for WindowsInjector {
    fn insert(&self, text: &str) -> Result<InjectionMethod, String> {
        let grapheme_count = text.chars().count();
        info!(
            text_len = text.len(),
            grapheme_count, "text_injection_started"
        );

        let mut keyboard = Enigo::new(&Settings::default())
            .map_err(|error| format!("Failed to create Windows input driver: {error}"))?;
        let mut clipboard = SystemClipboard::default();
        let result = inject_with_drivers(text, &mut keyboard, &mut clipboard, &std::thread::sleep);
        match &result {
            Ok(method) => info!(?method, "text_injection_completed"),
            Err(error) => warn!(error = %error, "text_injection_failed"),
        }
        result
    }
}

fn inject_with_drivers(
    text: &str,
    keyboard: &mut dyn KeyboardDriver,
    clipboard: &mut dyn ClipboardDriver,
    delay: &dyn Fn(Duration),
) -> Result<InjectionMethod, String> {
    let char_count = text.chars().count();
    if !text.contains('\n') && char_count <= 400 {
        match try_keyboard_sequence(text, keyboard, delay) {
            Ok(()) => return Ok(InjectionMethod::Keyboard),
            Err(error) => {
                warn!(error = %error, "text_injection_keyboard_fallback");
            }
        }
    }

    paste_with_clipboard_transaction(text, keyboard, clipboard, delay)?;
    Ok(InjectionMethod::Clipboard)
}

fn try_keyboard_sequence(
    text: &str,
    keyboard: &mut dyn KeyboardDriver,
    delay: &dyn Fn(Duration),
) -> Result<(), String> {
    let chars: Vec<char> = text.chars().collect();
    let chunk_count = chars.len().div_ceil(CHUNK_SIZE);

    for (index, chunk) in chars.chunks(CHUNK_SIZE).enumerate() {
        let chunk_text: String = chunk.iter().collect();
        keyboard
            .text(&chunk_text)
            .map_err(|error| format!("chunk {} failed: {error}", index + 1))?;
        if index + 1 < chunk_count {
            delay(Duration::from_millis(CHUNK_DELAY_MS));
        }
    }

    Ok(())
}

fn paste_with_clipboard_transaction(
    text: &str,
    keyboard: &mut dyn KeyboardDriver,
    clipboard: &mut dyn ClipboardDriver,
    delay: &dyn Fn(Duration),
) -> Result<(), String> {
    let previous_text = clipboard
        .read_text()
        .map_err(|error| format!("clipboard read failed: {error}"))?;
    clipboard
        .write_text(text)
        .map_err(|error| format!("clipboard write failed: {error}"))?;

    delay(Duration::from_millis(20));
    let paste_result = send_clipboard_paste_shortcut(keyboard);
    delay(Duration::from_millis(100));
    let restore_result = match previous_text {
        Some(previous_text) => clipboard.write_text(&previous_text),
        None => clipboard.clear(),
    };

    match (paste_result, restore_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(paste_error), Ok(())) => Err(format!("clipboard paste failed: {paste_error}")),
        (Ok(()), Err(restore_error)) => Err(format!("clipboard restore failed: {restore_error}")),
        (Err(paste_error), Err(restore_error)) => Err(format!(
            "clipboard paste failed: {paste_error}; clipboard restore failed: {restore_error}"
        )),
    }
}

trait KeyboardDriver {
    fn text(&mut self, text: &str) -> Result<(), String>;
    fn key(&mut self, key: Key, direction: Direction) -> Result<(), String>;
}

impl KeyboardDriver for Enigo {
    fn text(&mut self, text: &str) -> Result<(), String> {
        Keyboard::text(self, text).map_err(|error| error.to_string())
    }

    fn key(&mut self, key: Key, direction: Direction) -> Result<(), String> {
        Keyboard::key(self, key, direction).map_err(|error| error.to_string())
    }
}

trait ClipboardDriver {
    fn read_text(&mut self) -> Result<Option<String>, String>;
    fn write_text(&mut self, text: &str) -> Result<(), String>;
    fn clear(&mut self) -> Result<(), String>;
}

#[cfg(target_os = "windows")]
#[derive(Default)]
struct SystemClipboard {
    clipboard: Option<arboard::Clipboard>,
}

#[cfg(target_os = "windows")]
impl SystemClipboard {
    fn get(&mut self) -> Result<&mut arboard::Clipboard, String> {
        if self.clipboard.is_none() {
            self.clipboard = Some(arboard::Clipboard::new().map_err(|error| error.to_string())?);
        }
        self.clipboard
            .as_mut()
            .ok_or_else(|| "clipboard initialization failed".to_string())
    }
}

#[cfg(target_os = "windows")]
impl ClipboardDriver for SystemClipboard {
    fn read_text(&mut self) -> Result<Option<String>, String> {
        match self.get()?.get_text() {
            Ok(text) => Ok(Some(text)),
            Err(arboard::Error::ContentNotAvailable) => Ok(None),
            Err(error) => Err(error.to_string()),
        }
    }

    fn write_text(&mut self, text: &str) -> Result<(), String> {
        self.get()?
            .set_text(text)
            .map_err(|error| error.to_string())
    }

    fn clear(&mut self) -> Result<(), String> {
        self.get()?.clear().map_err(|error| error.to_string())
    }
}

fn send_clipboard_paste_shortcut(keyboard: &mut dyn KeyboardDriver) -> Result<(), String> {
    let mut errors = Vec::new();

    if let Err(error) = keyboard.key(Key::Control, Direction::Press) {
        errors.push(format!("ctrl_press_failed: {error}"));
        errors.extend(release_keyboard_modifiers(keyboard));
        return Err(errors.join("; "));
    }

    if let Err(error) = keyboard.key(paste_shortcut_key(), Direction::Click) {
        errors.push(format!("v_click_failed: {error}"));
    }

    if let Err(error) = keyboard.key(Key::Control, Direction::Release) {
        errors.push(format!("ctrl_release_failed: {error}"));
    }

    errors.extend(release_keyboard_modifiers(keyboard));

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn paste_shortcut_key() -> Key {
    #[cfg(target_os = "windows")]
    {
        Key::V
    }
    #[cfg(not(target_os = "windows"))]
    {
        Key::Unicode('v')
    }
}

fn release_keyboard_modifiers(keyboard: &mut dyn KeyboardDriver) -> Vec<String> {
    [
        Key::Control,
        Key::LControl,
        Key::RControl,
        Key::Shift,
        Key::LShift,
        Key::RShift,
        Key::Alt,
        Key::Option,
        Key::Meta,
    ]
    .into_iter()
    .filter_map(|key| {
        keyboard
            .key(key, Direction::Release)
            .err()
            .map(|error| format!("modifier_release_failed({key:?}): {error}"))
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::time::Duration;

    use super::{
        inject_with_drivers, paste_shortcut_key, release_keyboard_modifiers,
        send_clipboard_paste_shortcut, ClipboardDriver, InjectionMethod, KeyboardDriver,
    };
    use enigo::{Direction, Key};

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct KeyEvent {
        key: Key,
        direction: Direction,
    }

    #[derive(Default)]
    struct FakeKeyboard {
        events: Vec<KeyEvent>,
        failures: Vec<(Key, Direction, &'static str)>,
        typed_text: Vec<String>,
        fail_text: bool,
    }

    impl FakeKeyboard {
        fn fail_on(mut self, key: Key, direction: Direction, message: &'static str) -> Self {
            self.failures.push((key, direction, message));
            self
        }

        fn fail_text(mut self) -> Self {
            self.fail_text = true;
            self
        }
    }

    impl KeyboardDriver for FakeKeyboard {
        fn text(&mut self, text: &str) -> Result<(), String> {
            self.typed_text.push(text.to_string());
            if self.fail_text {
                return Err("keyboard text rejected".to_string());
            }
            Ok(())
        }

        fn key(&mut self, key: Key, direction: Direction) -> Result<(), String> {
            self.events.push(KeyEvent { key, direction });
            if let Some((_, _, message)) =
                self.failures
                    .iter()
                    .find(|(failed_key, failed_direction, _)| {
                        *failed_key == key && *failed_direction == direction
                    })
            {
                return Err((*message).to_string());
            }

            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeClipboard {
        current: Option<String>,
        writes: Vec<String>,
        clear_count: usize,
        fail_write: bool,
    }

    impl ClipboardDriver for FakeClipboard {
        fn read_text(&mut self) -> Result<Option<String>, String> {
            Ok(self.current.clone())
        }

        fn write_text(&mut self, text: &str) -> Result<(), String> {
            if self.fail_write {
                return Err("clipboard locked".to_string());
            }
            self.current = Some(text.to_string());
            self.writes.push(text.to_string());
            Ok(())
        }

        fn clear(&mut self) -> Result<(), String> {
            self.current = None;
            self.clear_count += 1;
            Ok(())
        }
    }

    #[test]
    fn long_text_is_written_before_paste_and_previous_clipboard_is_restored() {
        let requested = "é".repeat(401);
        let mut keyboard = FakeKeyboard::default();
        let mut clipboard = FakeClipboard {
            current: Some("previous value".to_string()),
            ..FakeClipboard::default()
        };

        let method = inject_with_drivers(&requested, &mut keyboard, &mut clipboard, &|_| {})
            .expect("long text should use clipboard delivery");

        assert_eq!(method, InjectionMethod::Clipboard);
        assert_eq!(
            clipboard.writes,
            vec![requested, "previous value".to_string()]
        );
        assert_eq!(clipboard.current.as_deref(), Some("previous value"));
        assert!(keyboard.events.iter().any(|event| {
            event.key == paste_shortcut_key() && event.direction == Direction::Click
        }));
    }

    #[test]
    fn short_unicode_text_uses_keyboard_without_touching_clipboard() {
        let requested = "Déjà prêt — café ☕";
        let mut keyboard = FakeKeyboard::default();
        let mut clipboard = FakeClipboard {
            current: Some("keep me".to_string()),
            ..FakeClipboard::default()
        };

        let method = inject_with_drivers(requested, &mut keyboard, &mut clipboard, &|_| {})
            .expect("Unicode keyboard delivery should succeed");

        assert_eq!(method, InjectionMethod::Keyboard);
        assert_eq!(keyboard.typed_text, vec![requested]);
        assert!(clipboard.writes.is_empty());
        assert_eq!(clipboard.current.as_deref(), Some("keep me"));
    }

    #[test]
    fn medium_ascii_text_is_typed_in_bounded_chunks_with_delays() {
        let requested = "a".repeat(250);
        let mut keyboard = FakeKeyboard::default();
        let mut clipboard = FakeClipboard::default();
        let delays = RefCell::new(Vec::new());

        let method = inject_with_drivers(&requested, &mut keyboard, &mut clipboard, &|delay| {
            delays.borrow_mut().push(delay);
        })
        .expect("medium text should use chunked keyboard delivery");

        assert_eq!(method, InjectionMethod::Keyboard);
        assert_eq!(
            keyboard
                .typed_text
                .iter()
                .map(String::len)
                .collect::<Vec<_>>(),
            vec![100, 100, 50]
        );
        assert_eq!(
            delays.into_inner(),
            vec![Duration::from_millis(50), Duration::from_millis(50)]
        );
        assert!(clipboard.writes.is_empty());
    }

    #[test]
    fn multiline_text_uses_clipboard_even_when_short() {
        let mut keyboard = FakeKeyboard::default();
        let mut clipboard = FakeClipboard::default();

        let method = inject_with_drivers(
            "first line\nsecond line",
            &mut keyboard,
            &mut clipboard,
            &|_| {},
        )
        .expect("multiline clipboard delivery should succeed");

        assert_eq!(method, InjectionMethod::Clipboard);
        assert!(keyboard.typed_text.is_empty());
        assert_eq!(clipboard.writes, vec!["first line\nsecond line"]);
        assert_eq!(clipboard.clear_count, 1);
    }

    #[test]
    fn keyboard_failure_falls_back_to_clipboard_for_emoji_text() {
        let requested = "Reply with 👋🏽";
        let mut keyboard = FakeKeyboard::default().fail_text();
        let mut clipboard = FakeClipboard::default();

        let method = inject_with_drivers(requested, &mut keyboard, &mut clipboard, &|_| {})
            .expect("clipboard fallback should recover keyboard failure");

        assert_eq!(method, InjectionMethod::Clipboard);
        assert_eq!(clipboard.writes, vec![requested]);
        assert_eq!(clipboard.clear_count, 1);
    }

    #[test]
    fn clipboard_write_failure_is_reported_without_pasting() {
        let mut keyboard = FakeKeyboard::default().fail_text();
        let mut clipboard = FakeClipboard {
            fail_write: true,
            ..FakeClipboard::default()
        };

        let error = inject_with_drivers("hello", &mut keyboard, &mut clipboard, &|_| {})
            .expect_err("a locked clipboard must fail delivery");

        assert!(error.contains("clipboard write failed: clipboard locked"));
        assert!(!keyboard.events.iter().any(|event| {
            event.key == paste_shortcut_key() && event.direction == Direction::Click
        }));
    }

    #[test]
    fn paste_failure_restores_previous_clipboard_and_reports_error() {
        let mut keyboard = FakeKeyboard::default().fail_text().fail_on(
            paste_shortcut_key(),
            Direction::Click,
            "paste blocked",
        );
        let mut clipboard = FakeClipboard {
            current: Some("original".to_string()),
            ..FakeClipboard::default()
        };

        let error = inject_with_drivers("hello", &mut keyboard, &mut clipboard, &|_| {})
            .expect_err("a blocked paste must fail delivery");

        assert!(error.contains("clipboard paste failed"));
        assert_eq!(clipboard.current.as_deref(), Some("original"));
        assert_eq!(clipboard.writes, vec!["hello", "original"]);
    }

    #[test]
    fn paste_shortcut_releases_control_when_v_click_fails() {
        let mut keyboard = FakeKeyboard::default().fail_on(
            paste_shortcut_key(),
            Direction::Click,
            "v was blocked",
        );

        let error = send_clipboard_paste_shortcut(&mut keyboard).unwrap_err();

        assert!(error.contains("v_click_failed: v was blocked"));
        assert!(keyboard.events.starts_with(&[
            KeyEvent {
                key: Key::Control,
                direction: Direction::Press,
            },
            KeyEvent {
                key: paste_shortcut_key(),
                direction: Direction::Click,
            },
            KeyEvent {
                key: Key::Control,
                direction: Direction::Release,
            },
        ]));
    }

    #[test]
    fn paste_shortcut_reports_control_release_failure_after_cleanup_attempt() {
        let mut keyboard = FakeKeyboard::default().fail_on(
            Key::Control,
            Direction::Release,
            "ctrl release blocked",
        );

        let error = send_clipboard_paste_shortcut(&mut keyboard).unwrap_err();

        assert!(error.contains("ctrl_release_failed: ctrl release blocked"));
        assert!(error.contains("modifier_release_failed(Control): ctrl release blocked"));
    }

    #[test]
    fn modifier_cleanup_releases_common_modifier_keys() {
        let mut keyboard = FakeKeyboard::default();

        let errors = release_keyboard_modifiers(&mut keyboard);

        assert!(errors.is_empty());
        assert_eq!(
            keyboard.events,
            vec![
                KeyEvent {
                    key: Key::Control,
                    direction: Direction::Release,
                },
                KeyEvent {
                    key: Key::LControl,
                    direction: Direction::Release,
                },
                KeyEvent {
                    key: Key::RControl,
                    direction: Direction::Release,
                },
                KeyEvent {
                    key: Key::Shift,
                    direction: Direction::Release,
                },
                KeyEvent {
                    key: Key::LShift,
                    direction: Direction::Release,
                },
                KeyEvent {
                    key: Key::RShift,
                    direction: Direction::Release,
                },
                KeyEvent {
                    key: Key::Alt,
                    direction: Direction::Release,
                },
                KeyEvent {
                    key: Key::Option,
                    direction: Direction::Release,
                },
                KeyEvent {
                    key: Key::Meta,
                    direction: Direction::Release,
                },
            ]
        );
    }
}
