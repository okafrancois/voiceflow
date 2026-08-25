# Paste last transcription 0.1.0

Status: completed

## Problem

A successful dictation can finish while its original target is no longer
available or focused. Voice Flow stores the result in history, but recovering it
requires opening the main window, finding the entry, and choosing its final-text
reinsertion action.

## Goal

Add a `Paste Last Transcription` item to the system tray. It inserts the newest
successful, non-empty final transcription into the currently focused target
without opening or focusing the main window.

## First-principles model

- History is the source of truth. The action must respect the configured text
  retention policy and must not keep a second hidden copy of dictated text.
- "Last transcription" means the newest successful history entry whose final
  text is not blank. A newer failed or blank entry does not hide an older usable
  result.
- Reinsertion uses the existing platform text injector so keyboard and
  transactional clipboard behavior stay identical to normal delivery.
- The backend selects the entry, performs the insertion, and records its result.
  The tray only triggers that backend action.
- The action must not show or focus the settings window because that would
  replace the user's intended insertion target.

## Information architecture

The native tray menu contains these actions in order:

1. `Show Settings`
2. `Toggle Recording`
3. `Paste Last Transcription`
4. Separator
5. `Quit`

The action is available whenever the tray is present. If no retained successful
transcription exists, the backend returns a specific error and inserts nothing.

## Data contract

```rust
struct LatestTranscriptionText {
    id: String,
    final_text: String,
}

fn get_latest_successful_transcription(
    &self,
) -> Result<Option<LatestTranscriptionText>, String>;

fn paste_last_transcription(...) -> Result<String, String>;
```

The successful command result is the existing delivery status:
`inserted_keyboard` or `inserted_clipboard`. The selected history row records
the same value. An injector failure records `failed` and returns the injector
error.

No persisted-data migration is required.

## Acceptance criteria

- The tray shows `Paste Last Transcription` between recording control and the
  separator before Quit.
- Choosing the item does not show or focus the main window.
- The backend selects the newest `success` entry with non-blank `final_text`.
- A newer error or blank entry is skipped in favor of the newest usable entry.
- The final, polished text is inserted through the existing platform injector.
- Successful insertion updates that history entry to `inserted_keyboard` or
  `inserted_clipboard`.
- Failed insertion updates the entry to `failed` and reports the original
  injector error without logging transcription content.
- If no usable entry exists, the action inserts nothing and returns
  `No successful transcription is available`.
- The action is also registered as a Tauri command so the backend behavior can
  run without the frontend.
- `text_retention = never` remains authoritative. In that mode, the action has
  no text to paste unless another retained successful entry already exists.

## BDD scenarios

### Recover the latest dictation

Given two retained successful transcriptions
And another application has a focused text field
When the user chooses `Paste Last Transcription` from the tray
Then Voice Flow inserts the final text of the newer transcription
And records the delivery method on that history entry
And the settings window remains hidden.

### Skip an unusable newest entry

Given a retained successful transcription
And a newer failed or blank history entry
When the user chooses `Paste Last Transcription`
Then Voice Flow inserts the older successful transcription.

### Report an empty history

Given no retained successful transcription
When the user chooses `Paste Last Transcription`
Then Voice Flow inserts nothing
And the backend reports that no successful transcription is available.

### Preserve a failed result for another retry

Given a retained successful transcription
And the focused application rejects text insertion
When the user chooses `Paste Last Transcription`
Then Voice Flow records `failed` on that entry
And keeps the transcription in history for a later attempt.

## Verification

- Rust store tests for newest-first selection, failed and blank row filtering,
  and empty history.
- Rust action tests for keyboard success, clipboard success, injection failure,
  and no retained result.
- Rust tray tests for menu ID dispatch and the user-visible label.
- Focused Rust tests followed by the full Rust test, Clippy, and formatting
  checks.
- Native tray behavior remains a manual macOS check because the automated
  desktop harness does not expose status-bar menus.

## Verification evidence

- Three focused store tests passed for newest usable selection, blank and failed
  row filtering, deterministic timestamp ties, and empty history.
- Four focused action tests passed for final-text insertion, keyboard and
  clipboard delivery status, injector failure, and empty history.
- Two tray contract tests passed for the visible label, action dispatch, and
  unknown menu IDs.
- The complete `cargo test` run passed: 638 library tests plus all non-ignored
  binary and integration suites. Credential-, model-, and fixture-dependent
  tests remained explicitly ignored.
- `cargo clippy --all-features -- -D warnings` and `cargo fmt -- --check` passed.
- `node scripts/check-md-links.mjs` passed. `pnpm check:md-links` could not run
  because `pnpm` is not installed, so the underlying repository script was run
  directly with Node.
- A current debug binary launched successfully with the dev application ID and
  logged `tray_created`. The `computer-use` integration was not approved to
  inspect Voice Flow, so clicking the native menu and observing focus remains a
  manual macOS check.
