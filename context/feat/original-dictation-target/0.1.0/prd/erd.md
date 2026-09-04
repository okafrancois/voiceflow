# Original dictation target 0.1.0

Status: active

## Problem

Voice Flow currently injects a completed transcription into whichever editable
field is focused when processing finishes. Local transcription and polish can
take several seconds, so a user who changes applications during that delay can
receive dictated text in the wrong place.

## Goal

Add an opt-in macOS setting that remembers the application and editable field
focused when recording starts. Users choose between a reliable foreground mode
and a best-effort background mode.

## First-principles model

- The recording session owns an immutable delivery-policy snapshot and target.
  Changing settings or focus while transcription runs must not retarget it.
- Disabled preserves the current behavior: deliver to the field focused when
  processing completes.
- Foreground mode activates the original application before using the existing
  keyboard or transactional-clipboard injector. It may interrupt the user, but
  works with more applications.
- Background mode writes through the macOS Accessibility API to the captured
  editable element without activating its application. Unsupported or stale
  elements fail explicitly.
- A failed targeted delivery must never fall through to the currently focused
  field. History retains the final text and records `failed`.
- Direct polish streaming is incompatible with delayed targeted delivery and
  stays disabled for sessions that capture an original target.
- Background delivery does not start the current-focus correction observer,
  because another application remains focused after insertion.

## Settings contract

```rust
enum OriginalTargetMode {
    Foreground,
    Background,
}

struct AppSettings {
    original_target_enabled: bool,
    original_target_mode: OriginalTargetMode,
}
```

Both fields are persisted in the existing settings store. The feature defaults
to disabled; the stored default mode is `foreground`.

The macOS transcription settings show:

1. An `Insert into the original field` switch.
2. When enabled, a mode selector:
   - `Bring application to front` — more reliable and visibly activates it.
   - `Keep application in background` — does not change the frontmost
     application, but can fail when the target does not expose writable
     accessibility attributes.

## Delivery contract

At recording start, the backend snapshots the setting, original application
identifier, focused accessibility element, and selected-text range when
available.

At final delivery:

- disabled: call the existing current-focus injector;
- foreground: activate the captured application, wait for focus to settle,
  verify it is frontmost, then call the existing injector;
- background: restore the captured selection range on the captured element and
  set its selected text through Accessibility;
- missing, closed, unsupported, or stale targets: return an injection error and
  leave the final text in history.

The feature applies to normal completed recording delivery. Explicit history
reinsertion, retries, previews, quick controls, and tray actions retain their
existing current-target semantics.

## Scope boundaries

### In scope

- macOS target capture, foreground activation, and background accessibility
  insertion.
- Backend-owned settings, session snapshot, routing, errors, and history status.
- Typed frontend settings and localized macOS controls.
- Unit tests for settings and delivery routing, plus macOS adapter tests where
  behavior can be isolated from live applications.

### Out of scope

- Guaranteed support for games, secure fields, custom-rendered editors, or
  applications that do not expose writable Accessibility attributes.
- Windows and Linux targeted delivery.
- Automatically returning to the application used while transcription was
  processing. Foreground mode leaves the original application active.
- Retargeting after recording starts.

## Acceptance criteria

1. Existing users keep current-focus delivery until they enable the setting.
2. Foreground mode activates and verifies the captured application before
   injection; it never injects after failed activation or verification.
3. Background mode never changes the frontmost application.
4. Background failure never invokes keyboard or clipboard injection against the
   current application.
5. Each recording uses the enablement and mode captured when it started.
6. A missing target produces a visible delivery failure and a `failed` history
   status while preserving the final transcription.
7. Direct polish streaming does not insert deltas during a targeted session.
8. Settings validation rejects unknown mode values.
9. The controls appear only on macOS and explain the reliability trade-off.
10. No transcript text, accessibility value, or selection content is logged.

## BDD scenarios

### Deliver through the reliable mode

Given original-target delivery is enabled in foreground mode
And an editable field in application A is focused when recording starts
When the user switches to application B before transcription completes
Then Voice Flow activates application A
And inserts the final transcription using the existing injector
And leaves application A in the foreground.

### Deliver without changing applications

Given original-target delivery is enabled in background mode
And a writable accessibility field in application A is captured
When the user switches to application B before transcription completes
Then Voice Flow writes the final transcription at the captured selection
And application B remains frontmost.

### Reject an unsupported background target

Given original-target delivery is enabled in background mode
And the captured field no longer accepts accessibility writes
When transcription completes
Then Voice Flow does not send simulated keys or paste commands
And marks delivery failed
And preserves the transcription in history.

### Preserve current behavior when disabled

Given original-target delivery is disabled
When transcription completes
Then Voice Flow inserts into the field currently focused at completion.

## Verification

- Failing-first Rust tests for disabled, foreground, background, and failure
  routing.
- Rust settings default, serialization, validation, and migration tests.
- Frontend tests for switch visibility, mode descriptions, and persisted keys.
- Focused Rust and frontend suites followed by full Rust tests, Clippy, rustfmt,
  desktop build, shared typecheck, and locale parity.
- Manual signed macOS checks in a native editor, browser field, Electron editor,
  and one unsupported/custom field.

### Current evidence

The Rust routing, settings, session-snapshot, history-status, and direct-stream
tests pass. The full non-ignored Rust suite, Clippy, rustfmt, desktop typecheck,
frontend tests, production build, locale parity, and documentation link checks
also pass. A native development build launches with Accessibility permission.

Live foreground and background insertion across representative third-party
applications remains a signed-app manual compatibility check. It was not
automated because it depends on live microphone input and external application
accessibility trees.
