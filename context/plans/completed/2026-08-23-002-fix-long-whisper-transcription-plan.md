---
title: Complete Local Whisper Transcription for Long Recordings
type: fix
status: completed
date: 2026-08-23
---

# Complete Local Whisper Transcription for Long Recordings

## Overview

Fix local Whisper dictations that return only an early portion even though the
recording pipeline delivered the complete audio buffer. The implementation will
decode long audio through bounded, low-energy-aligned windows inside the
headless STT engine.

## Problem Frame

- Current state: one sherpa-onnx stream decodes the entire local Whisper buffer.
  Logged 77.25-second and 121.14-second inputs reached that stream completely,
  but the returned text stopped early.
- Desired state: every long Whisper sample range is decoded in order, while
  short recordings and other engine families retain their current behavior.

## Scope Boundaries

### In scope

- Local Whisper batch transcription in `sherpa_onnx/engine.rs`.
- Pure unit coverage for segmentation and aggregation.
- A real-model verification using the installed Whisper Turbo model.
- The engine contract and feature documentation.

### Out of scope

- Recorder, channel, stop, VAD, and frontend changes.
- Cloud providers.
- Overlapping audio and text deduplication.

## Implementation Units

- [x] Add failing unit tests for short and long engine-specific decode routing.
- [x] Add failing unit tests for complete range coverage and quiet boundaries.
- [x] Add failing unit tests for ordered aggregation, empty output, and errors.
- [x] Implement balanced Whisper segmentation with a 28-second maximum.
- [x] Decode and aggregate all Whisper segments inside the existing recognizer
      lock.
- [x] Record segment count and duration in structured logs.
- [x] Verify with unit tests and a real long French speech fixture.
- [x] Run Rust tests, clippy, and formatting checks.
- [x] Update the canonical indexes and move this plan to completed.

## System-Wide Impact

Only local Whisper requests longer than 28 seconds will make more than one
recognizer decode call. This increases inference time roughly with the number of
windows but prevents the far costlier failure mode of silently losing dictated
text. Memory use remains bounded by the existing full recording buffer.

## Risks and Dependencies

- A continuous word can cross a chosen boundary. Searching around each balanced
  target for the lowest-energy audio reduces that risk.
- Multiple decode calls add latency. STT accuracy and completeness outrank speed
  by project policy.
- The real-model check depends on the locally installed Whisper Turbo files and
  is not a CI requirement.

## Verification Evidence

- The initial targeted test compile failed because the segmentation helpers did
  not exist. This recorded the TDD red state before implementation.
- Six inline engine tests pass. They cover exact 28-second boundaries, a
  65-second three-segment decode, contiguous sample preservation, low-energy
  boundary selection, non-Whisper routing, aggregation, and errors.
- The installed Whisper Turbo model transcribed a 100.83-second generated French
  fixture and returned the final marker words `kangourou violet`.
- `cargo test --quiet`: exit 0. The library reported 526 passed and 0 failed;
  every non-ignored integration suite also passed.
- `cargo clippy --all-features -- -D warnings`: exit 0.
- `cargo fmt -- --check`: exit 0.
