---
title: Fix macOS tray paste crash and fullscreen menu focus
type: fix
status: completed
date: 2026-10-23
---

## Outcome and contract

The macOS status icon opens its native menu while preserving the active
application and fullscreen Space. Paste Last Transcription inserts the retained
text without terminating Voice Flow. Contract:
[paste last transcription](../../feat/paste-last-transcription/0.1.0/prd/erd.md).

## Evidence

Two production v1.4.0 crash reports (October 23, 19:09 and 22:26 local time)
show SIGTRAP on the main thread: `dispatch_sync called on queue already owned
by current thread`. Both stacks pass through the tray callback, history paste,
and `text_injector::macos::try_clipboard_paste`. That code remains in v1.4.1.
The clipboard helper unconditionally dispatches synchronously to the main
queue, including when the caller is already on the main thread.

The macOS tray both enables native left-click menus and opens/focuses the main
window on left-button release/double click. This conflicts with menu-only
interaction and can change Spaces when another app is fullscreen. The locked
tray-icon implementation already opens the native menu on mouse-down.

## Implementation units

- [x] Reproduce main-thread reentrancy in a native regression executable,
  without touching the clipboard or synthesizing keyboard input.
- [x] Execute main-thread work inline when already on that thread; retain
  synchronous marshaling for background callers.
- [x] Test and remove macOS icon-click window activation; retain explicit
  Show Settings and existing non-macOS icon behavior.
- [x] Verify native behavior where accessible, all Rust checks, applicable
  frontend checks and documentation; record limitations.

## Scope and risks

Keep history selection, text retention, injection selection, clipboard restore,
and explicit settings activation unchanged. No permissions/capabilities change,
engine change, publication or production installation. Native regression tests
must not paste into the user's active application. Fullscreen UI verification
needs actual native menu access and cannot be inferred from mock history tests.

## Verification approach

Extract the existing main-queue helper without changing behavior, then test it
from an executable's real main thread (Rust's ordinary tests run on worker
threads). A second case pumps the main run loop to verify worker-to-main
execution and returned values. Add behavioral tray event tests before changing
the activation policy. The native probe's failure must be observed before the
reentrancy guard is added.

## Failing-first results

- The standalone native regression, compiled directly from the production
  main-queue helper, exits 133 (SIGTRAP) before the guard. After checking
  `pthread_main_np`, both nested main-thread and worker-to-main cases pass.
  No clipboard access or keyboard events are used by this test.
- The tray regression fails before the macOS policy change with
  `Some(ShowSettings)` instead of `None`; the other three tray tests pass.
  Log: `/tmp/voiceflow-tray-tests-before.log`.
- Apple documents that synchronous dispatch to the current queue deadlocks:
  [DispatchQueue.sync](https://developer.apple.com/documentation/dispatch/dispatchqueue/sync(execute:)-3segw).

## Verification results

- `cargo test --lib tray::tests` passes all four tray tests after the fix.
- `cargo test --test macos_main_queue_test` passes both native dispatch checks,
  including nested calls on the real main thread.
- `cargo test -- --test-threads=1 --skip benchmark_resampling_performance`
  passes 981 tests, with 40 existing ignored tests and one filtered benchmark;
  the custom native executable also passes both checks in that run.
- `cargo clippy --all-features -- -D warnings`, focused Clippy for
  `macos_main_queue_test`, and `cargo fmt -- --check` pass with Rust 1.99.0.
- Desktop production build, shared package typecheck and `pnpm check:i18n`
  pass. No frontend behavior changes are needed.
- Markdown link verification passes.
- `pnpm --filter @voiceflow/desktop tauri:build:mac:local-install` succeeds.
  The generated `apps/desktop/src-tauri/target/debug/bundle/macos/Voice Flow Dev.app`
  passes strict deep code-signature verification and the build script's checks
  for the separate development identifier, URL scheme and audio entitlement.
  It is not launched, installed over production, or published.

## Documentation extraction

The feature contract now covers main-thread reentrancy and native menu ownership.
The testing guide explains why a custom executable is needed for main-thread
regressions, and separates that proof from fullscreen UI validation. The
debugging guide records the crash signature and corrects its stale macOS
keyboard/clipboard threshold to match the existing 400-grapheme implementation.

## Verification limits observed

The first complete Rust run stops at the existing
`benchmark_resampling_performance`: 3.527 s against a 3 s wall-clock threshold.
An isolated run also fails at 3.758 s. Both the benchmark and resampler are
byte-for-byte unchanged from HEAD. At investigation time the host reports
0% CPU idle, 139 runnable processes, and load averages 169/179/139, supporting
resource contention as the explanation. No benchmark assertion or product
audio code is changed. The remaining functional suite is run serially with
only that named benchmark excluded; this is not a passing full-suite claim.

Computer Use can inspect the installed Voice Flow window but its screenshot
captures the floating pill; access to `com.apple.systemuiserver` returns
`Computer Use server error -10005: timeoutReached`. The fullscreen status-menu
interaction has not been visually verified. No production paste was triggered
and the installed application was not replaced.
