---
title: Integrate local MLX Audio dictation
type: feat
status: completed
date: 2026-10-24
---

## Overview

Connect the existing Voice Flow dictation workflow to the MLX Audio Whisper server already installed on this Mac. Contract: [local MLX Audio dictation](../../feat/mlx-audio-stt/0.1.0/prd/erd.md).

## Problem frame

MLX Audio already transcribes French locally, but Voice Flow only dispatches three vendor-specific WebSocket providers. The desired outcome is shortcut-driven dictation with the existing local server and no credentials.

## Scope boundaries

Add one provider to existing backend-owned settings and the recording-consumer interface. Preserve signed production installation, user settings, cloud connectors, shortcuts, and model files. No Git mutation or release publication.

## Implementation units

- [x] Add failing contract checks for dispatch and complete HTTP transcription lifecycle.
- [x] Implement the local HTTP client, schema, settings readiness, and local history classification.
- [x] Update the existing UI and all locales to explain local operation without a key.
- [x] Verify contracts, full relevant checks, real local inference, and a signature-verified desktop build.
- [x] Install and configure the development bundle with a reversible local settings backup; document how to use it.
- [x] Complete the desktop connection check after the user grants the new bundle's macOS permissions.

## System-wide impact

Reuse `StreamingSttClient` and `RecordingConsumer`, with buffering inside this provider. Keep recording and insertion engine-agnostic. The legacy cloud-service settings section also hosts the local HTTP provider; visible text must distinguish local processing.

## Risks and dependencies

The server must run on this Mac. Cached model loading can make the first request slower. The signed production app cannot acquire this source change without a build; use the documented development bundle so its signature and permissions are isolated. macOS may require the user to grant permissions to that development identity.

## Verification evidence

The initial contract test failed with `Unsupported streaming STT provider: mlx-audio` before implementation. All six deterministic HTTP contract tests then passed. The explicit ignored integration check was run against the actual localhost MLX server using `/Users/bernyitoutou/Applications/MLX-Audio/samples/kokoro-fr.wav`; it returned `Bonjour, je teste une voix française directement sur mon Mac.` through `StreamingConsumer`.

Full `cargo test`: 989 passed, zero failures, 41 intentionally ignored; both native macOS test executables also succeeded. `cargo clippy --all-features -- -D warnings` and `cargo fmt -- --check` pass. Full frontend suite: 115 passed, including two MLX settings cases. The desktop frontend production build, shared TypeScript check, ten-locale i18n check and Markdown-link check pass. The targeted UI cases were repeated after the final localization-expression change, and the final desktop build includes that change.

The documented non-cleaning local-install build succeeds. Its bundled signature, development identifier, isolated URL scheme and microphone entitlement validate, and strict deep signature verification passes again after copying it to `/Users/bernyitoutou/Applications/Voice Flow Dev.app`. Generated Tauri schema churn from feature/build variants was restored to the original tracked contents; no capability files were changed. No website code is changed.

Development settings are seeded from the production settings, select `mlx-audio`, the installed Whisper checkpoint and French, and preserve the user's double-tap Fn shortcut. Production settings are unchanged, verified against the private backup under `/Users/bernyitoutou/Applications/MLX-Audio/local-data/voice-flow-backups/`. Launchers in that installation start the local server plus the development bundle, or return to the production app, without switching during an active dictation.

The production app was idle before it was closed. The development bundle is running, its bundled CLI status works, and its visible UI requests Microphone and Accessibility for the new identity. The user granted Microphone and Accessibility and confirmed completion. The native settings UI shows MLX Audio (Local), the expected loopback URL and Whisper model, and the Check action returns Connected. Native history metadata records successful `mlx-audio` dictation with `is_cloud = 0`. No OS authorization was changed by the agent. Screenshot: `/Users/bernyitoutou/Applications/MLX-Audio/samples/voice-flow-mlx-connected.png`. The main window is returned to Dictation. Log files: `/tmp/voiceflow-mlx-{cargo-test,frontend-test,clippy,build,ui-contract,i18n,links}.log`.
