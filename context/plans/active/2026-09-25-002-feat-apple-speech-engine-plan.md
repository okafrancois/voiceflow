---
title: Apple speech engine
type: feat
status: active
date: 2026-09-25
spec: context/feat/apple-speech-engine/0.1.0/prd/erd.md
adr: context/architecture/decisions/007-apple-speech-streaming-engine.md
---

# Apple Speech Engine

## Overview

Add Apple `SpeechAnalyzer` (macOS 26+) as an opt-in "Apple (fast)" model in the
Tauri app. Audio is streamed to the analyzer while recording, so the wait after
the recording stops no longer grows with the dictation length.

## Problem Frame

- Current: every local engine decodes after the recording stops (median wait
  19.5 s above 60 s of audio).
- Desired: users who pick "Apple (fast)" get the transcript a fraction of a
  second after stopping, as in the native prototype (median 1.2 s including
  polish, ~0.1 s of recognition).

## Scope Boundaries

In scope: Swift bridge, build integration, engine type and catalog entry,
streaming consumer, batch engine, fallback, model list UI, i18n.

Out of scope: partial transcript display, vocabulary hints, benchmark.

## Implementation Units

### Unit 1 — Swift bridge and build

- Files: `src-tauri/swift/AppleSpeech/**`, `src-tauri/build.rs`,
  `src-tauri/Cargo.toml` (build dependency `serde_json`)
- `build.rs` compiles the package for the Rust target triple when the macOS SDK
  is 26+, links it statically, and sets `cfg(apple_speech)`.
  `VOICEFLOW_DISABLE_APPLE_SPEECH` forces the stub.

### Unit 2 — Rust engine

- Files: `stt_engine/apple/{mod,bridge,engine,consumer}.rs`, `traits.rs`,
  `models.rs`, `unified_manager.rs`, `sherpa_onnx/engine.rs`
- `EngineType::Apple`, `APPLE_SPEECH` built-in model, availability from the
  bridge, asset install through `download_model`, delete rejected, batch
  engine instance, opt-in exclusion from recommendations and fallbacks.

### Unit 3 — Recording path and settings

- Files: `commands/audio/capture.rs`, `commands/settings/mod.rs`,
  `state/unified_state.rs`
- Apple model → `AppleStreamingConsumer`; start failure → fallback to the first
  downloaded non-built-in model. The dictation language reaches the manager.

### Unit 4 — Model list UI

- Files: `lib/tauri.ts`, `components/Home/model/VoiceInputSection.tsx`,
  `i18n/locales/*.json`, `components/Home/__tests__/ModelSettings.test.tsx`
- Built-in label instead of size, no delete button, Apple hint asking for an
  explicit dictation language.

## System-Wide Impact

- macOS builds now run `swift build` (release, ~5 s incremental).
- `get_models` calls the bridge once per listing to read availability.
- History, quality metrics, and retry record the `apple` engine through the
  existing `settings.model` path.

## Risks & Dependencies

- `auto` dictation language maps to the system locale. On an English system
  dictating in French, the user must set French explicitly.
- CI macOS runners need the macOS 26 SDK to include the engine; otherwise the
  stub is built and the model is hidden.
- Tauri sync commands run on the main thread. Bridge status queries therefore
  never run on the caller: they run on a background thread at startup, on
  dictation language changes, and after asset installation, and listings read
  the cached answer (the engine stays hidden until it is known).

## Verification Evidence

2026-09-25:

- Bridge proof: `cargo test --lib stt_engine::apple -- --ignored` on macOS 27.2
  with Xcode 27: a French TTS recording (Thomas voice, 11 s, fed in 100 ms
  chunks) transcribed as "Bonjour, ceci est un test de la reconnaissance vocale
  d'Apple dans l'application Voiceflow. On vérifie que la transcription
  fonctionne pendant l'enregistrement", with `finish()` returning after 162 ms.
- `cargo test`: 956 passed, 0 failed.
- `cargo clippy --all-features -- -D warnings`: clean with the bridge and with
  `VOICEFLOW_DISABLE_APPLE_SPEECH=1` (stub).
- `cargo fmt -- --check`: clean.
- Frontend: `tsc && vite build` OK, `pnpm --filter @voiceflow/shared typecheck`
  OK, `pnpm check:i18n` OK, vitest 112 passed.
- `cargo test`: 957 passed after moving status queries to a background
  thread; clippy clean with the bridge and with the stub.
- E2E: `settings.spec.ts` (2 tests) failed from 13:36 UTC on this machine. The
  same failure reproduced with the bridge disabled and on the pre-change
  baseline `b1c0de6` in a separate worktree, while the suite had passed at
  13:34 UTC with the same code. The Apple release commit was made with
  `SKIP_E2E=1`; vitest and markdown link checks still ran.

Still open: manual dictation test in the local install build, and the
environment-dependent settings modal e2e failure.
