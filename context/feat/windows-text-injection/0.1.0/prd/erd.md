# Windows Text Injection Reliability Specification

## Version

- Feature: `windows-text-injection`
- User-visible name: Reliable Windows delivery
- Version: `0.1.0`
- Status: Completed

## Problem Statement

The Windows injector falls back to `Ctrl+V` for long text and failed keyboard
simulation, but its production caller supplies an empty clipboard callback. The
target application can therefore receive stale clipboard content while the IPC
command still reports success. Clipboard contents are also not restored.

## Goal

Every Windows delivery path must either insert the requested text or return a
structured failure. Clipboard fallback must write the requested text before
`Ctrl+V`, restore the prior text clipboard afterwards, and release modifier keys
even when a keyboard operation fails.

## Non-Goals

1. Do not add Windows-only product features unrelated to text delivery.
2. Do not claim native application compatibility that was not exercised on a
   Windows host.
3. Do not change dictation or polish decisions in the frontend.

## First-Principles Model

1. Delivery success is observable only if the selected delivery mechanism
   completes without error.
2. The text argument, not pre-existing clipboard state, is the source of truth.
3. A fallback must not silently transform one failure into a false success.
4. Temporary clipboard mutation must be bounded and restored when possible.
5. Modifier cleanup is mandatory because a stuck modifier damages subsequent
   user input beyond this operation.

## Information Architecture

The headless text injector selects one of two strategies:

- short single-line text: Unicode keyboard simulation, chunked when needed;
- multiline or long text, or keyboard failure: transactional clipboard paste.

The Tauri command and recording pipeline consume the injector result. The
frontend only renders the resulting success or failure event.

## Data Contract

```rust
pub enum InjectionMethod {
    Keyboard,
    Clipboard,
}

pub trait TextInjector {
    fn insert(&self, text: &str) -> Result<InjectionMethod, String>;
}
```

No persisted-data migration is required.

## Acceptance Criteria

1. Long or multiline Windows text is written to the clipboard before `Ctrl+V`.
2. Short Windows text uses keyboard simulation when that succeeds.
3. A short-text keyboard failure uses the clipboard transaction.
4. The previous text clipboard is restored after paste; an empty/non-text
   clipboard is cleared after paste.
5. Clipboard write, paste, or restore failures are returned and logged.
6. Control and common modifier keys are released after every paste attempt.
7. The `insert_text` IPC command returns an error when delivery fails.
8. Unit tests cover ASCII, accented Latin text, emoji, multiline, long text,
   keyboard fallback, clipboard failures, and modifier cleanup.
9. Native Windows coverage remains explicitly marked unverified until executed
   on a Windows test host.

## BDD Scenarios

### Deliver a short accented sentence

Given a focused Windows text field
And keyboard simulation accepts Unicode input
When Voice Flow inserts a short accented sentence
Then the sentence is delivered by keyboard
And the clipboard is unchanged.

### Deliver multiline text

Given a focused Windows text field
And the clipboard contains previous text
When Voice Flow inserts multiline text
Then the requested text is written before `Ctrl+V`
And the previous clipboard text is restored afterwards.

### Recover from keyboard failure

Given keyboard simulation rejects a short emoji-containing value
When Voice Flow inserts that value
Then it retries through the clipboard transaction
And reports clipboard delivery as the selected method.

### Report a real failure

Given keyboard simulation fails
And the clipboard cannot be written
When Voice Flow inserts text
Then the backend returns an error
And the IPC command does not report success.

## Verification

- Rust unit tests using fake keyboard, clipboard, and delay drivers.
- Existing audio pipeline tests updated to assert explicit delivery results.
- `cargo test --lib text_injector` and `cargo test --lib commands::audio::shared`.
- Full Rust tests, Clippy, formatting, and the desktop build.
- Native Windows compatibility table records execution evidence separately.

## Completion Evidence

- Transactional clipboard fallback, restoration, modifier cleanup, Unicode,
  multiline, long-text, and failure propagation are covered by Rust tests.
- The complete Rust suite passed with 629 library tests plus all non-ignored
  integration suites.
- `cargo clippy --all-features -- -D warnings` and `cargo fmt -- --check`
  passed.
- The optimized macOS application bundle and native Tauri smoke test passed.
- Native Windows execution remains unverified because no Windows runner was
  available; this does not weaken the explicit platform test matrix or unit
  coverage.
