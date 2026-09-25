---
title: Incremental local Whisper transcription
type: feat
status: active
date: 2026-09-25
spec: context/feat/incremental-local-transcription/0.1.0/prd/erd.md
---

# Incremental Local Whisper Transcription

## Overview

Decode completed Whisper windows during the recording so that only the final
window is left to decode after the user stops. This is the first of two steps
to reduce post-recording latency in the Tauri app. The second step, an opt-in
on-device streaming engine (Apple SpeechAnalyzer on macOS 26), is deferred to
its own spec.

## Problem Frame

- Current: `BufferingConsumer` buffers everything and decodes in `finish()`.
  Median post-recording wait is 9.6 s for 30–60 s recordings and 19.5 s above
  60 s (production `quality_metrics.db`, 2026-08-24 → 2026-09-25).
- Desired: for recordings longer than 33 s, windows of 22–28 s start decoding
  as soon as they are closed, and `finish()` only decodes the last window
  (under 33 s).

## Scope Boundaries

In scope:
- Pure boundary function in `stt_engine/sherpa_onnx/engine.rs`
- Incremental Whisper path in `stt_engine/buffering_engine.rs`
- Spec, plan, and index updates

Out of scope:
- Shorter windows, partial results to the frontend, settings
- SenseVoice, Qwen3-ASR, cloud providers
- Apple SpeechAnalyzer engine (follow-up)

## Implementation Units

### Unit 1 — Boundary policy

- Files: `src-tauri/src/stt_engine/sherpa_onnx/engine.rs`
- Approach: add `incremental_whisper_boundary(pending)` reusing
  `quietest_boundary` and the existing window constants.
- Verification: unit tests for the threshold, cut range, minimum remainder,
  and quiet-boundary preference.

### Unit 2 — Incremental consumer

- Files: `src-tauri/src/stt_engine/buffering_engine.rs`
- Approach: introduce a window decoder seam so the consumer logic can be tested
  without models; spawn early window decodes in `send_chunk()`; await and join
  in order in `finish()`.
- Verification: unit tests with a recording fake decoder covering short,
  long, ordering, empty windows, failures, and non-Whisper engines.

### Unit 3 — Documentation

- Register the spec and plan in the indexes; record verification evidence.

## System-Wide Impact

- The recognizer is still guarded by one mutex; early decodes and the final
  decode run one after another, as before.
- Early decodes use CPU during the recording. The recorder runs on its own
  thread and only writes buffers, so capture is not expected to be affected.
- A canceled recording drops the consumer; early decodes already running
  finish and their results are discarded.

## Risks & Dependencies

- Online boundaries differ from the balanced boundaries used today for
  recordings above 33 s. Both respect the same 5–28 s window bounds and quiet
  boundary search.
- A cancellation during a long early decode can delay the next dictation's
  decode by the remainder of that window (at most ~6 s with whisper-turbo).

## Follow-up

- Apple SpeechAnalyzer streaming engine as a user-selectable "fast" engine on
  macOS 26 (Swift bridge through swift-rs, runtime version check, Whisper
  fallback). The user explicitly accepted on 2026-09-25 the accuracy tradeoff
  that ADR-002 requires for streaming recognition; record it in a dedicated ADR
  with that spec.

## Verification Evidence

2026-09-25 — Units 1–3 implemented test-first.

- Failing first: `cargo test --lib incremental_boundary` and
  `cargo test --lib buffering_engine` failed to compile before the
  implementation (`incremental_whisper_boundary` and `WindowDecoder` missing).
- `cargo test`: 948 passed, 0 failed (29 suites), including 4 boundary tests
  and 7 consumer tests.
- `cargo clippy --all-features -- -D warnings`: clean.
- `cargo fmt -- --check`: clean.

Real-model check (ignored test
`real_whisper_incremental_decode_matches_a_single_decode`, release build,
whisper-turbo on CoreML): a 108.7 s French TTS dictation (Thomas voice, the
same 133-word text twice) fed in real time.

| Mode | Wait after stop | Words | Word errors vs script |
|------|-----------------|-------|-----------------------|
| Incremental | 8.4 s | 266 / 266 | ~6 (~2.3 %) |
| Single block (v1.2.3 path) | 23.1 s | 210 / 266 | ~61 (~22.9 %) |

The single-block path dropped a 56-word passage in the second half; the
incremental path returned the full text in order. One sample only.

Still open before completion: the production check from the spec (median
post-recording wait of local Whisper recordings above 60 s, measured in
`quality_metrics.db` after real use of a build containing this change).
