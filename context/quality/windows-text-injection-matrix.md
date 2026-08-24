# Windows Text Injection Verification Matrix

This matrix separates deterministic contract coverage from native application
evidence. A row is complete only when its evidence column names an executed
check; implementation alone is not evidence.

| Input or failure mode | Expected path | Deterministic coverage | Native Windows evidence |
|---|---|---|---|
| Short ASCII | Keyboard | Rust fake-driver test | Pending Windows CI/manual run |
| Accented Latin text | Keyboard | `short_unicode_text_uses_keyboard_without_touching_clipboard` | Pending Windows CI/manual run |
| Emoji and composed Unicode | Keyboard, then clipboard if rejected | `keyboard_failure_falls_back_to_clipboard_for_emoji_text` | Pending Windows CI/manual run |
| 101–400 characters | Chunked keyboard | Rust chunk-driver contract | Pending Windows CI/manual run |
| More than 400 characters | Clipboard transaction | `long_text_is_written_before_paste_and_previous_clipboard_is_restored` | Pending Windows CI/manual run |
| Multiline text | Clipboard transaction | `multiline_text_uses_clipboard_even_when_short` | Pending Windows CI/manual run |
| Keyboard driver rejection | Clipboard fallback | `keyboard_failure_falls_back_to_clipboard_for_emoji_text` | Pending Windows CI/manual run |
| Clipboard locked | Explicit error, no `Ctrl+V` | `clipboard_write_failure_is_reported_without_pasting` | Pending Windows CI/manual run |
| Paste shortcut rejected | Restore clipboard and return error | `paste_failure_restores_previous_clipboard_and_reports_error` | Pending Windows CI/manual run |
| Modifier release failure | Cleanup attempt and explicit error | Existing modifier cleanup tests | Pending Windows CI/manual run |

## Native application targets

The Windows CI job compiles the real Windows `arboard` and `enigo` adapters and
runs the deterministic matrix on `windows-latest`. A manual compatibility pass
should additionally exercise Notepad, a Chromium text field, Microsoft Word,
Visual Studio Code, a terminal, and an application running as administrator.
The elevated-application row is expected to require Voice Flow at an equivalent
integrity level; the product must report failure rather than claim delivery.
