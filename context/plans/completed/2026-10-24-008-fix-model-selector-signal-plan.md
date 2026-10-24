---
title: Remove redundant MLX model-selection copy
type: fix
status: completed
date: 2026-10-24
---

## Overview

Keep the model selector focused on choosing and managing a model, following the [catalogue contract](../../feat/model-catalog/0.1.0/prd/erd.md) and [managed MLX contract](../../feat/mlx-audio-stt/0.2.0/prd/erd.md).

## Problem frame

The managed MLX catalogue repeats infrastructure explanations above the list and underneath each model. They do not distinguish models or help selection. Speed and accuracy are currently unknown for these checkpoints; explanatory copy must not substitute for actual evidence or invented rankings. The user also asks whether MLX text streaming is implemented.

## Scope boundaries

Remove redundant STT selector/onboarding copy and its translations, preserve model actions and useful native descriptions, and verify the installed runtime's streaming APIs against the application's actual path. Research measurement sources without turning incomparable upstream results into application performance scores. Coordinate with the separate MLX polish implementation; do not change its files, worker/runtime, production settings, capabilities, or Git state.

## Implementation units

- [x] Update catalogue acceptance criteria and add failing selector/onboarding regressions.
- [x] Remove redundant guidance, empty model descriptions, and the unused MLX ratings footnote.
- [x] Verify upstream performance evidence and distinguish streaming capability from implemented behavior.
- [x] Run frontend checks, visually inspect the actual component, and record limits.

## System-wide impact

This is presentation-only. Backend model availability, downloads, selection, runtime progress, and measured-versus-unknown state remain authoritative and unchanged. Shared model rows continue to support meaningful native and polish descriptions.

## Risks and dependencies

The shared checkout has an active MLX polish implementation. Do not trigger the desktop build's Cargo cleanup or install an unverified intermediate polish bundle. Published WER and speed results need dataset, language, hardware, precision, and measurement-scope context; they are not automatically comparable to the shipped MLX checkpoints.

## Verification evidence

- Failing-first regressions reproduced selector guidance, onboarding guidance, and the irrelevant ratings footnote: `/tmp/voiceflow-model-selector-copy-red.log`, `/tmp/voiceflow-model-selector-onboarding-red.log`, and `/tmp/voiceflow-model-selector-note-red.log`. The native ratings/description assertion stayed green.
- Full desktop Vitest suite after implementation: **133 passed across 23 files**; `/tmp/voiceflow-model-selector-frontend-verified.log`.
- Desktop `tsc --noEmit`, shared package `typecheck`, `pnpm check:i18n`, and standalone `vite build`: passed. Logs: `/tmp/voiceflow-model-selector-types-final.log`, `/tmp/voiceflow-model-selector-shared-types.log`, `/tmp/voiceflow-model-selector-i18n-final.log`, and `/tmp/voiceflow-model-selector-vite-build.log`. Existing Vite chunk-size/deprecation warnings remain unrelated.
- Browser verification rendered the actual `VoiceInputSection`/`SettingsProvider`/catalogue with isolated fixture settings and mocked commands, not the native application. Confirmed three rows, no descriptions/guidance/ratings note, no page errors, no horizontal overflow at 860px or 390px, and working size/installed filters. Inspected dark/light screenshots at `/tmp/voiceflow-model-selector-dark.png`, `/tmp/voiceflow-model-selector-narrow.png`, and `/tmp/voiceflow-model-selector-light.png`.
- T3 snapshots timed out twice with `Preview automation snapshot timed out after 15000ms.` The test runner's default Chromium revision was absent; verification used the already-installed Chromium headless shell revision 1234 via an explicit executable path. No browser installation was performed. The temporary fixture page, owned Vite server, and preview tab were removed/stopped afterward.
- Verified the installed MLX Audio 0.6.1 package and upstream versioned implementations. Whisper offers AlignAtt chunk decoding; Parakeet yields chunk results; Qwen emits token results from supplied audio. The current application still buffers MLX audio until `finish()` and returns one final worker response. No streaming implementation or live-audio verification is claimed.
- Reviewed the primary NVIDIA, Qwen, and OpenAI model cards. Documented the required warm/cold speed and same-corpus French WER protocol, precision/hardware caveats, and actual streaming boundaries in the [canonical STT reference](../../reference/providers/stt.md#performance-evidence-and-model-selection-data). No new benchmark, accuracy score, or model-performance ranking was shipped.

## Delivery boundary and extracted invariant

The source cleanup and frontend build are complete. The installed development app was **not** rebuilt/replaced in this slice because the shared checkout contains an active, not-yet-verified MLX polish implementation. Include this cleanup in the next verified development bundle; do not present the browser fixture as an installed native-app result. Production app/settings and runtime behavior remain untouched.

Model-selection copy must distinguish alternatives or communicate an actionable state. Infrastructure implementation details belong in developer/support documentation, not repeated under every model. Streaming API availability must not be confused with connected microphone input or actual partial-text delivery.
