---
title: Original Dictation Target
type: feat
status: active
date: 2026-09-04
---

# Original dictation target implementation plan

## Overview

Add opt-in macOS delivery to the application or exact editable field captured
when recording starts. Offer foreground activation for compatibility and direct
Accessibility insertion for uninterrupted background delivery.

## Problem frame

The recording pipeline captures application context but the final injector
targets current focus. Voice Flow can already activate a captured macOS
application for workflow previews and read its focused accessibility element,
but normal dictation does not carry a delivery target through processing and
cannot write to a retained element.

## Scope boundaries

### In scope

- Persisted enablement and foreground/background preference.
- Immutable per-recording target capture.
- Target-aware final insertion and explicit failure behavior.
- macOS Accessibility background insertion.
- macOS settings UI and all locale keys.
- Tests and canonical documentation updates.

### Out of scope

- Windows/Linux target adapters.
- Universal support for custom-rendered or secure fields.
- Changes to manual retry, history, tray, or quick-control targeting.
- Automatic foreground restoration after reliable delivery.

## Implementation units

1. Completed: define the specification, pure delivery policy, and
   failing-first routing tests.
2. Completed: add settings defaults, persistence validation, typed frontend
   fields, and failing-first settings tests.
3. Completed: capture a retained macOS accessibility target and implement direct
   selected-text insertion without application activation.
4. Completed: thread the target snapshot through normal recording finalization,
   implement verified foreground routing, and suppress direct polish streaming
   for targeted sessions.
5. Completed: add macOS-only controls, descriptions, all locale entries, and UI
   tests.
6. In progress: automated verification and architecture documentation are
   complete. Signed-app compatibility checks against representative third-party
   fields remain manual because they require live microphone input and native
   application accessibility trees.

## System-wide impact

- `commands/settings`: two persisted settings with strict enum validation.
- `commands/audio`: session-time capture and final delivery routing.
- `text_injector`: retained macOS target and background insertion adapter.
- `sensors/focused_context`: verified foreground application activation.
- Frontend settings types, controls, tests, and ten locale catalogs.
- Data-flow and feature indexes after verification.

No history schema, audio pipeline, STT engine, polish engine, permission
capability file, or raw IPC boundary changes are required.

## Risks and dependencies

- Accessibility element references can become stale when an application rebuilds
  its view tree. That is an expected background-mode failure.
- Some applications expose readable but non-writable accessibility attributes.
- Direct polish streaming currently writes to current focus; targeted sessions
  must prevent that path before the first delta.
- Foreground activation is asynchronous. Delivery must wait and verify rather
  than relying only on a fixed delay.
- Native focus and background writes need signed-app manual verification because
  unit tests cannot control third-party application accessibility trees.

## Verification evidence

- Rust failing-first evidence: the first focused test run failed on the missing
  settings, target types, and delivery routes before implementation.
- `cargo test --lib original_target`: 6 passed.
- `cargo test --lib direct_stream_typing`: 2 passed.
- Escalated full Rust test run: 645 library tests and all non-ignored integration
  suites passed. Only existing live API/model checks remained ignored.
- `cargo clippy --all-features -- -D warnings`: passed.
- `cargo fmt -- --check`: passed.
- Desktop TypeScript typecheck: passed.
- Desktop Vitest run: 14 files and 88 tests passed.
- Desktop production Vite build: passed with existing dependency/chunk warnings.
- Locale parity and Markdown link checks: passed.
- Native Tauri development launch: passed and reported Accessibility permission
  available. Microphone permission was not granted, so no live dictation was
  attempted.
- Browser smoke check: the shell rendered meaningful content with no blank page
  or Vite error overlay. Direct-browser IPC errors are expected because the
  Tauri bridge exists only in the native webview.
