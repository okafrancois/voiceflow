---
title: Native app review — fixes and new features
type: fix
status: active
date: 2026-09-24
---

# Native app review: fixes and new features

## Overview

A full read of `voiceflow-native/` (Swift, macOS 26) surfaced lifecycle bugs,
main-thread stalls, a destructive dictionary matcher, clipboard races and a
set of small correctness issues, plus a list of features. The user asked to
implement every finding and every proposed feature. Product priority applies:
STT accuracy > STT stability > UX > speed.

## Proof of completion

- `swift build` and `swift test` green in `voiceflow-native/VoiceFlow`.
- Every pure-logic fix lands with a failing-first test.
- Behaviour that needs a real Mac session (hotkey, AX, clipboard, pill) is
  listed under "Manual verification" with what to check.

## Implementation units

### U1 — Dictation lifecycle (accuracy)
- Start the microphone before the engine is ready; buffer audio and flush it
  into the engine once built (`EngineFeed`).
- A stop requested before the engine is ready is remembered and honoured.
- Escape cancels a dictation in progress; "undo last insertion" in the menu.
- Bootstrap only prepares Apple assets when the Apple engine and a concrete
  locale are selected; a hotkey press while preparing gives feedback.

### U2 — Main thread
- Event tap runs on a dedicated thread with its own run loop.
- AX capture/insert/read run off the main thread, with a messaging timeout.
- History refresh uses SQL aggregates, runs off the main thread.
- Vocabulary persistence is debounced and does not write per matched entry.

### U3 — Dictionary safety
- Whole-word matching for dictionary entries and snippet triggers.
- Learned corrections become suggestions; they apply only after the user
  accepts them (or after the same correction is observed several times).
- Correction watching only when the text actually landed in the watched field.

### U4 — Clipboard
- Restore only after the paste is consumed (longer delay, `changeCount`
  guard), and mark the dictation as transient/concealed.

### U5 — History
- Dedicated `app_name` column with `PRAGMA user_version` migrations.
- History filter and statistics cover every engine family.
- Statistics computed in SQL (no 5 000-row cap). Retention applied daily.

### U6 — Robustness
- Data races: recorder levels, polisher prewarm.
- WhisperKit cache keeps one model; preload on engine change.
- Polishing of long dictations is chunked by paragraph.
- Voice-processing ducking of other audio minimised.
- Pill on the screen under the mouse.
- Download errors surfaced; update check persisted (the appcast is already
  published by `.github/workflows/release-native.yml`).
- Unused Apple Events entitlement removed; log rotation keeps a backup.
- Hard-coded French strings go through `L.t`; language names follow the UI.

### U7 — Tests and tooling
- Tests for `SilenceTrimmer`, `PolishGuard`, vocabulary matching, correction
  diffing, `Shortcut.matches`, trigger-mode state machine, text chunking.
- Localization key check script; native app added to verification commands.

### U8 — Features
- STT biasing from the dictionary (Whisper prompt, Apple contextual strings).
- Context-aware spacing and capitalisation from the text before the caret.
- Live preview in the pill; error state in the pill.
- Voice commands (new line, new paragraph…).
- Command mode: transform the selected text with a spoken instruction.
- Per-app dictation language.
- Richer menu bar (start/stop, engine, language, polish).
- One-click import of the Tauri app history and dictionary.

## Risks

- Event tap on a background thread: callbacks now race with settings
  reloads — guard shared state with a lock.
- Contextual-string APIs must be checked against the macOS 26 SDK before use.

## Automated verification (2026-09-24)

- `swift build` (debug and release) — clean, no warning in project sources,
  Swift 6 language mode for app and tests.
- `swift test` — 72 tests in 14 suites pass (engine feed, trigger modes,
  shortcuts, vocabulary matching, correction diffing, smart spacing, voice
  commands, polish chunking and guard, silence trimmer, history migrations
  and SQL statistics, Tauri import parsing, Whisper prompt echo, insertion
  verdict).
- `tools/check-strings.py` — every user-facing key has an English entry.
- `./build.sh` — bundle signed, entitlements reduced to audio input.

## Independent review (2026-09-24)

A read-only review of the diff found no blocker and seven important
defects, all fixed:

1. A chord (Fn + ←) during processing cancelled the previous dictation —
   chord cancels now only apply while recording.
2. Imported Tauri bare terms changed the case of ordinary words — imported
   as case-sensitive identity entries.
3. Undo after a command/selection replacement erased the original text — the
   replaced selection is kept and restored.
4. Undo fell back to a blind ⌘Z after an Accessibility write — refused with a
   message instead.
5. Native migrations collided with Tauri's `user_version` — native schema
   version moved to `native_meta`, columns ensured on every open.
6. Toggle/double-tap desynchronised after a refused start —
   `HotkeyManager.startRejected()`.
7. Smart spacing lowercased German nouns — lowercasing limited to languages
   where it is safe.

Minor fixes: voice-command punctuation (colon kept, no double full stop),
accent-sensitive dictionary matching, new variants of active entries become
suggestions, vocabulary flushed on quit, chained clipboard restores, AX
timeout verified before falling back, pill notice alpha, stale live preview,
no auto-start after the microphone prompt, own synthetic key events ignored
by the tap, echoed Whisper prompt stripped.

## Manual verification (needs a real session)

- Hold ⌥ Space and speak immediately: first word present (Apple engine,
  first dictation after launch).
- Tap and release the shortcut very quickly: pill disappears, no stuck
  recording.
- Escape while recording (nothing inserted) and while transcribing (entry in
  history, nothing inserted).
- Fn as hold shortcut, then Fn + ↑: no dictation left running.
- Dictate twice in a row in Notes/Mail: space and capital joined correctly.
- Paste path (text > 400 characters) in an Electron app (Slack, VS Code):
  dictation pasted, previous clipboard back afterwards.
- Menu › Retirer la dernière insertion in a native field and in Chromium.
- Command mode: select a paragraph, say "traduis en anglais".
- Voice commands: "Bonjour. À la ligne. Merci."
- Dictionary term with Whisper and Apple: term spelled right without a
  replacement entry (biasing).
- Error with main window closed (mute the mic): message shown in the pill.
- Multi-screen: pill shows on the screen under the mouse.
- Tauri import from Settings › Données, run twice: no duplicates.

## Deferred

- Per-app engine: dropped (one model in memory; switching per app costs
  seconds per dictation).
- AGENTS.md rule 5 (English-only comments) vs the French comments of
  `voiceflow-native/`: decision pending with the user.

## Progress

- [x] U1 · [x] U2 · [x] U3 · [x] U4 · [x] U5 · [x] U6 · [x] U7 · [x] U8
