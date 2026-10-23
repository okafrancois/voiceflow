---
title: Diagnose and fix the invisible macOS fullscreen status menu
type: fix
status: completed
date: 2026-10-23
---

## Outcome and contract

The status menu must visibly open above another application's native fullscreen
Space on both the built-in and external display, without leaving that Space.
Contract: [paste last transcription](../../feat/paste-last-transcription/0.1.0/prd/erd.md).

## Current evidence

The user confirms v1.4.2 still fails on both displays. Their screenshot shows
the status icon highlighted but no menu. The previous click-policy tests prove
that settings are not activated; they do not prove native menu visibility.
Keep the main-thread paste fix and remove no passing regression.

The host runs macOS 27.2 (26B118), with a built-in display and an external
D27-30. The locked tray-icon 0.21.3 predates the upstream macOS 27 event-forwarding
fix, but that fix alone has not been shown to explain this fullscreen failure.
Tauri invokes application setup on RuntimeRunEvent::Ready, so premature tray
creation must be demonstrated rather than inferred from the setup callback.

## Implementation units

- [x] Reproduce the native failure and inspect menu/window/Space behavior.
- [x] Identify the smallest proven correction and add its failing regression.
- [x] Implement the correction and verify real fullscreen interaction.
- [x] Run relevant checks and document the result and remaining limits.

## Scope and risks

No recording, transcription, injection, settings, or permission changes.
Use an isolated native probe if needed; avoid production paste actions or
interfering with user text. Computer Use can inspect application windows but
SystemUIServer and ControlCenter access both time out. Do not equate a passing
policy test with a visible menu and do not publish an unverified fix.

## Investigation references

- [tray-icon macOS 27 click fix](https://github.com/tauri-apps/tray-icon/pull/365)
- [fullscreen startup requirements](https://github.com/tauri-apps/tray-icon/pull/122)
- [fullscreen menu report](https://github.com/tauri-apps/tray-icon/issues/251)

## Verification strategy

Use a native reproduction while another app is already fullscreen, then repeat
on both displays. Observe menu opening, dismissal, an innocuous action and
whether the foreground fullscreen Space changes. Verify desktop behavior and
explicit settings activation as well. Automated tests must cover the identified
invariant using real AppKit state where possible; they complement the visual
check rather than replacing it.

## Reproduction evidence

An isolated Tauri probe creates only a two-item status menu, with no main
window, pill, shortcuts, application settings, audio or clipboard access.
The user confirms its baseline menu is also invisible over a fullscreen app.
This narrows the failure to native status-menu presentation independently of
the production application's windows and frontend.

The probe logs NSMenu begin/end tracking notifications and its own native
window state. The baseline status window already has FullScreenAuxiliary
(256), and the menu contains both items. Tauri's overlay view remains in the
baseline. The next comparison removes that overlay and leaves AppKit's status
button/menu handling intact, without changing application activation policy.

Baseline log: `/tmp/voiceflow-fullscreen-probe-baseline.log`.
Native comparison log: `/tmp/voiceflow-fullscreen-probe-native.log`.

The user confirms the direct AppKit variant also remains invisible. Removing
the Tauri overlay alone is not a fix and is not applied to production code.
The next isolated comparison changes only activation policy from Regular to
Accessory, with the original tray implementation restored.

The user confirms the Accessory variant displays the menu correctly in the
same fullscreen scenario. Its native log confirms activation policy 1, versus
policy 0 in both failing variants. This is the first successful behavioral
comparison. Before choosing a product change that removes the Dock icon, test
whether a menu-scoped policy change is sufficient and restores the previous
policy after dismissal.

Accessory log: `/tmp/voiceflow-fullscreen-probe-accessory.log`.

The user confirms the menu-scoped policy comparison also works. Its log shows
policy 0 → 1 before tracking and 1 → 0 on dismissal. The native popup's glass
window is on the active Space in the working variants, unlike the failing
Regular variants. The correction therefore scopes Accessory policy to the
background status menu and restores normal application policy afterward.

A new real-main-thread AppKit regression fails first with policy 0 instead of
1 before implementing the policy lease. The implementation verifies one-time
restoration, preserves an existing Accessory policy, and respects an intervening
explicit policy change. The next visual check uses the actual production
registration helper, filtered to the status menu's NSMenu object.

Failing-first log: `/tmp/voiceflow-fullscreen-policy-before.log`.
Scoped comparison log: `/tmp/voiceflow-fullscreen-probe-temporary.log`.

## Final verification — 2026-10-24

The final probe imports the production registration helper and filters its
notifications to its own NSMenu. The user confirms that its menu opens and
`Mark test click` works on both displays without leaving the fullscreen Space.
The log records policy application/restoration and successful actions at both
display coordinates. The popup and glass windows join the active Space.

- Complete `cargo test -- --test-threads=1`: 982 tests pass, 40 explicitly
  ignored, no exclusions. The existing audio performance benchmark passes.
- Two native main-queue checks and three native activation-policy checks pass.
  The policy test is rerun after making its initial state setup idempotent.
- `cargo clippy --all-features -- -D warnings`, focused native test/probe
  Clippy, and `cargo fmt -- --check` pass.
- Desktop `build:local-install-frontend`, shared typecheck, i18n and Markdown
  link checks pass. The frontend build uses the same TypeScript/Vite steps
  without deleting concurrent Rust build output.
- `pnpm --filter @voiceflow/desktop tauri:build:mac:local-install` builds the
  complete development app and verifies its strict signature, development
  identifier, URL scheme and audio entitlement without launching it.

Final native log: `/tmp/voiceflow-fullscreen-probe-fixed.log`. The owned
diagnostic process is stopped after verification. Its exploratory source is
retained at `/tmp/voiceflow-fullscreen-probe-source.rs`, outside the shipped
change. Production observers are removed on exit. The real-main-thread policy
test remains in the repository.

## Documentation extraction

The testing and debugging guides now distinguish a native visible-menu check
from activation and dispatch assertions. Compare activation policies in an
isolated menu application before modifying tray overlay or Space behavior.
Keep workaround state scoped to the actual menu and restore it on dismissal.
This fix is verified on the affected macOS 27.2 machine; automated policy checks
do not establish native visibility on every macOS release.
