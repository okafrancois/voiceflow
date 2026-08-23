---
title: "fix: Restore macOS permissions and local STT model downloads"
type: fix
status: completed
date: 2026-08-23
---

# Restore macOS permissions and local STT model downloads

## Overview

Make microphone and screen-recording permission actions work from the macOS app, prevent completed Qwen3-ASR downloads from being rejected, and expose the compatible Whisper variants already supported by sherpa-onnx.

## Problem frame

Current behavior:

- The development executable runs without the audio-input entitlement. macOS TCC refuses to show the microphone prompt.
- The screen-recording action opens System Settings without first calling the macOS screen-capture request API.
- Qwen3-ASR reaches 100%, extracts successfully, then fails validation because the estimated minimum size for `merges.txt` is 5,869 bytes larger than the real file.
- The UI only exposes four hard-coded local STT models even though the current Whisper engine can load more official sherpa-onnx exports.

Desired behavior:

- A user click produces the native permission request or opens the correct settings pane when a decision already exists.
- Local macOS testing launches a signed `.app` carrying the repository entitlements.
- A complete Qwen3-ASR archive remains installed and selectable after extraction.
- The model page lists curated multilingual Whisper variants from Tiny through Large v3 and Turbo, with downloads using the exact official filenames.

## Scope boundaries

- **In scope**: macOS permission request flow, local packaged-app launch workflow, STT model metadata, download validation, UI model types and labels, tests, and canonical documentation.
- **Out of scope**: TCC database resets, edits to `src-tauri/capabilities/`, new STT engine families, arbitrary user-supplied model repositories, and automatic downloads of large models.

## Implementation units

- [x] **Unit 1: Reproduce the permission defects**
  - Add testable permission-action decisions.
  - Assert that undetermined microphone access requests the OS prompt.
  - Assert that denied microphone access opens System Settings.
  - Assert that screen recording requests access before opening System Settings when needed.

- [x] **Unit 2: Repair the macOS permission flow**
  - Remove the blocking microphone wait from the permission provider.
  - Call `CGRequestScreenCaptureAccess` from the screen-recording action.
  - Add a local macOS command that builds and launches the entitled dev app bundle.

- [x] **Unit 3: Reproduce and repair model completion**
  - Add a regression test using the real Qwen3-ASR auxiliary-file size.
  - Validate small archive support files by successful extraction and non-empty presence instead of rounded megabyte estimates.
  - Verify download completion before emitting the complete event.

- [x] **Unit 4: Expand the compatible model catalogue**
  - Add official multilingual Whisper Tiny, Medium INT8, Large v3 INT8, and Turbo INT8 definitions.
  - Resolve each model to its official sherpa-onnx repository and runtime filenames.
  - Update frontend model types, language hints, tests, and the engine contract.

- [x] **Unit 5: Verify the full behavior**
  - Run targeted failing-first tests, the Rust suite, clippy, formatting, frontend build, shared typecheck, and i18n checks.
  - Build the entitled macOS `.app`, inspect its signature and Info.plist, launch it through Launch Services, and confirm the permission/model states from logs.

## System-wide impact

- Permission state and actions remain backend-owned and keep the existing IPC command names.
- The model catalogue remains a backend source of truth; the frontend only renders `get_models` results.
- Existing model directories and settings remain compatible.
- Large models stay opt-in and are never downloaded during onboarding.

## Risks and dependencies

- macOS associates TCC grants with the signed app identity. Rebuilding an ad-hoc development bundle may require granting access again if its code requirement changes.
- Whisper Large v3 is accurate but slow and memory-intensive. The UI must expose it as an explicit choice, not a default.
- Official model repositories can change. Definitions use verified filenames and conservative validation rather than guessing file layouts.

## Verification evidence

- Permission decision tests: 4 passed.
- Local STT model-definition tests: 8 passed, including the real Qwen3-ASR `merges.txt` size regression.
- Model manager and Whisper engine integration tests: 40 passed, 3 ignored because they require downloaded fixtures.
- Full Rust suite: passed with `cargo test -- --test-threads=1`. The default parallel invocation exposes a pre-existing shared-SQLite test lock.
- Frontend: desktop production build passed; 66 Vitest tests passed; shared-package typecheck and i18n checks passed.
- Build workflow tests: 3 passed, including optional local polish runtime handling and automatic full-Xcode selection.
- macOS bundle: `Voice Flow Dev.app` built and launched with identifier `com.voiceflow.voicetotext.dev`, an ad-hoc signature, the audio-input entitlement, and the microphone usage description.
- Runtime log: the repaired completeness check recognized the existing Qwen3-ASR installation and loaded it successfully with Core ML.
- Live macOS validation: the app log recorded `app_permission_request_completed permission="microphone" granted=true`; after restart, accessibility, input monitoring, microphone, and screen recording all reported `granted`.
- Clippy: the new catalogue code is clean. The repository-wide `-D warnings` check remains blocked by five pre-existing lints in unrelated audio, window-context, cloud-provider, and window lifecycle code.
