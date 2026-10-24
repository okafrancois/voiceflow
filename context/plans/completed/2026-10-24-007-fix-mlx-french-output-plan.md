---
title: Preserve French in managed MLX transcription
type: fix
status: completed
date: 2026-10-24
---

## Overview

Fix unwanted English output for French dictation under the [managed MLX contract](../../feat/mlx-audio-stt/0.2.0/prd/erd.md).

## Problem frame

The running development app already selects `stt_engine_language=fr`, but its raw MLX transcript is English before Apple polish runs. `BufferingConsumer` joins internal domain/subdomain codes into Whisper's initial prompt. On the same saved French recording, `initial_prompt="it general"` reproduces English output; no prompt, a French prompt, and actual vocabulary terms produce French. Explicit `task="transcribe"` does not fix the metadata prompt.

## Scope boundaries

Correct MLX recording prompt construction only. Preserve explicit user vocabulary, glossary terms, native-engine behavior, polish, settings, retained recordings, and the separate ongoing MLX polish implementation. Do not change production settings, permissions, dependencies, or capabilities.

## Implementation units

- [x] Confirm selected language and raw-vs-polished output from dev logs.
- [x] Reproduce the prompt-dependent language change on the same real recording.
- [x] Add failing prompt-construction regressions and implement the narrow fix.
- [x] Verify real French output through the recording consumer and run relevant suites.
- [x] Rebuild, install, and reopen the development app; document evidence.

## System-wide impact

The fix belongs in backend recording prompt construction rather than UI language settings or polish. Native engines retain existing prompt assembly. MLX receives real user-provided vocabulary but not opaque application category codes.

## Risks and dependencies

The polish implementation is editing the shared checkout; coordinate Cargo builds and avoid including incomplete polish changes in the installed bundle. Real audio is read only from development history, never uploaded or copied into the repository. Do not claim a language-quality guarantee for arbitrary user instructions.

## Verification evidence

Initial controlled real-model comparisons are recorded in `/tmp/voiceflow-french-output-diagnosis.log` and `/tmp/voiceflow-french-prompt-diagnosis.log`. Whisper and independent Qwen both identify French from the same saved recording without the internal category prompt.

- Failing-first prompt tests reproduced both MLX gaps while native prompt ordering passed: `/tmp/voiceflow-mlx-french-prompt-red.log`.
- Focused buffering suite: 12 passed, 2 ignored after the fix; `/tmp/voiceflow-mlx-french-prompt-green.log`.
- The opt-in `real_managed_mlx_recording_stays_french_with_domain_metadata` test passed through `BufferingConsumer` with the same saved French recording, installed isolated models, and internal domain/subdomain metadata. It unloads the manager before asserting; `/tmp/voiceflow-mlx-french-recording-green.log`. This proves the reported regression, not a general language-quality guarantee.
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib`: 775 passed, 7 ignored; `/tmp/voiceflow-mlx-french-lib-final.log`.
- `cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-features -- -D warnings`: passed; `/tmp/voiceflow-mlx-french-clippy.log`.
- `rustfmt --edition 2021 --check apps/desktop/src-tauri/src/stt_engine/buffering_engine.rs`: passed; `/tmp/voiceflow-mlx-french-fmt-scoped.log`. Full-checkout formatting was not claimed: the separate polish task had an in-progress external test with formatting differences and was left untouched.
- Independent scoped source review found no actionable issue. It confirmed dictation reaches the helper and preserves explicit prompts, glossary terms, native ordering, and lifecycle behavior.
- `npm --prefix apps/desktop run tauri:build:mac:local-install`: passed, including frontend typechecking/build and development bundle signature verification; `/tmp/voiceflow-mlx-french-bundle.log`.
- Installed and reopened only `~/Applications/Voice Flow Dev.app` after confirming the old dev app was idle. Staging and installed bundle identifiers were `com.voiceflow.voicetotext.dev`; deep/strict signatures passed. Source and installed executable SHA256 matched `02d24187448bc7457efac6e41f2fce39d6132dd699787bd16ec0a81727615c0b`. The new executable ran as PID 49974 and its CLI reported `ok: true`, recording/transcription false, state `idle`.
- The previous development bundle was preserved at `~/Applications/.Voice Flow Dev.backup-2026-10-24T10-55-27.029Z`. No settings, retained recordings, production installation, or permissions were changed. The managed MLX polish task resumed after the bundle build and installation completed.

## Extracted invariant

Internal classification identifiers are application metadata, not transcription vocabulary. Do not pass them as Whisper prompt text for managed MLX dictation. Diagnose language changes at the raw transcript boundary before changing language preferences or polish behavior.
