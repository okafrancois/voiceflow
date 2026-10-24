---
title: Live managed MLX transcription
status: completed
version: 0.1.0
date: 2026-10-24
---

# Live managed MLX transcription

## Observable outcome

While recording with any curated managed MLX STT model, real decoded text appears before recording stops. Stopping produces the authoritative complete transcription through the existing correction, polish, history and delivery pipeline. Provisional text is never pasted, saved as a completed entry, or polished.

## Scope and design

- Use the existing `RecordingConsumer`, manager, pinned offline worker and `transcription-partial` event. Native/cloud engines and MLX polish retain their behavior.
- A managed-MLX-only consumer keeps every 16 kHz PCM sample for final full-recording inference. A single owned background task decodes bounded recent-audio snapshots during recording, forwarding genuine model output, not simulated token animation.
- Optional window/context capture must not block MLX audio ingestion or live preview startup. Poll it concurrently, retain completed context for postprocessing, and cancel unfinished capture when forwarding ends. Native/cloud prompt ordering remains unchanged.
- Preview inference is coalesced: at most one in-flight snapshot, no unbounded queued decode jobs, at least two seconds of newly received audio between requests, and at most thirty seconds of recent audio per preview. A long recording's preview represents recent speech; each event replaces the previous preview rather than appending overlapping transcripts.
- This slice is live chunked transcription, not a claim that all three upstream models support a stateful microphone/token protocol. It preserves the existing verified family-specific language/prompt options and final full-audio accuracy.
- Finalization closes preview publication before decoding the complete audio, including the last partial chunk. An in-flight preview gets a bounded graceful drain before cancellation; owned futures/tasks must not outlive recording cancellation/drop. Preview failures never finalize a transcript or prevent the final full-audio attempt.
- Backend events carry the existing `task_id`, `text`, `is_definite` contract; preview results are non-definite. Emission is gated by the current recording and cancellation state. No new capability, network endpoint or frontend scheduling logic.
- The pill and dictation home render backend partial text reactively. New sessions, cancellation, error and completion clear it; stale task events cannot resurrect text. No model-selection explanatory clutter or invented performance scores.

## BDD acceptance

1. Given enough voiced audio and a ready model, when recording continues, a nonempty model-derived partial event arrives before `finish()` and replaces earlier preview text.
2. Given slow inference and many input chunks, audio ingestion remains responsive, preview work is bounded/coalesced, and the final decoder receives every sample exactly once in order.
3. Given cancellation/drop or finalization, pending preview work stops, no subsequent partial is emitted, and the worker is reusable/restartable without unrelated process termination.
4. Given preview failure or silence/short audio, no fabricated transcript is emitted; final transcription retains existing minimum-duration and error behavior.
5. Given French audio, each curated checkpoint produces a French live partial and a complete French final result through the real consumer/manager; no external HTTP service or production settings are involved.
6. Given stale partial events, the frontend does not overwrite a newer recording or completed result. Listeners clean up on unmount.

## Verification

Failing-first consumer lifecycle/coalescing/full-tail tests, capture event-gating checks, frontend event/render tests, real isolated French replay for all three curated checkpoints, then relevant Rust/frontend/worker gates and independent review. Installed UI/live microphone acceptance is separate from controlled chunk replay and must be reported honestly.

## Delivery evidence

Completed on 2026-10-24: real French chunk replay passed for all three managed checkpoints with partial output before stop; final combined gates passed 1,137 Rust tests and 160 frontend tests, plus Clippy, formatting, build and documentation checks. Independent reviews are resolved. The signed DEV bundle was installed and launched with existing settings unchanged. This does not establish installed live microphone acceptance or general accuracy/latency benchmarks. Exact evidence and limits: [completed execution plan](../../../../plans/completed/2026-10-24-010-feat-mlx-live-transcription-plan.md).
