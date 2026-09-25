# Incremental Local Transcription Specification

## Version

- Feature: `incremental-local-transcription`
- User-visible name: Faster Long Dictation
- Version: `0.1.0`
- Status: Active

## Problem Statement

Local Whisper transcription starts only after the recording stops. The
buffering consumer keeps every chunk in memory and decodes the whole recording
in `finish()`, so the wait after the user releases the shortcut grows with the
dictation length.

Production quality metrics (`quality_metrics.db`, 489 local transcriptions,
2026-08-24 to 2026-09-25) measure the post-recording wait as follows:

| Recording length | Median wait | p90 wait |
|------------------|-------------|----------|
| < 10 s | 2.1 s | 2.9 s |
| 10–30 s | 4.5 s | 6.3 s |
| 30–60 s | 9.6 s | 14.4 s |
| > 60 s | 19.5 s | 37.8 s |

The native macOS prototype, which transcribes while the user speaks, waits a
median 1.2 s after the recording stops. Long Whisper recordings are already
decoded as independent windows of at most 28 seconds
(`long-recording-transcription` 0.1.0), so most of those windows could be
decoded while the user is still speaking.

## Goal

Local Whisper recordings longer than one decode window must start decoding
their completed windows during the recording, so that only the final window
remains to decode after the recording stops. Recognition behavior for the
recorded audio must stay within the existing window contract.

## Non-Goals

1. Do not shorten Whisper windows below the current contract to gain speed.
2. Do not change SenseVoice or Qwen3-ASR decoding.
3. Do not change cloud STT streaming behavior.
4. Do not emit partial transcription text to the frontend in this version.
5. Do not add user-facing settings. The engine choice between speed and
   accuracy is covered by a separate on-device streaming engine feature.

## First-Principles Model

1. The recorded samples remain the source of truth. Every sample must be
   decoded exactly once, in recording order.
2. A window that can no longer change can be decoded before the recording ends.
   A window can no longer change once the audio after its end is long enough
   that the final window can never be shorter than the minimum window.
3. Whisper windows keep the existing bounds: at most 28 seconds, at least
   5 seconds, and boundaries placed on the quietest nearby audio.
4. Recordings that fit in one or two balanced windows gain nothing from early
   decoding and must keep today's exact behavior.
5. Segmentation stays in the headless local STT layer. The recorder, capture
   command, and frontend do not decide how a model consumes audio.

## Architecture

`BufferingConsumer` keeps its `RecordingConsumer` contract.

For a Whisper model, `send_chunk()` appends samples to a pending buffer. Each
time the pending buffer reaches 33 seconds (the 28-second maximum window plus
the 5-second minimum window), the consumer:

1. picks the quietest boundary between 22 and 28 seconds into the pending
   buffer, preferring 25 seconds on ties;
2. spawns the decode of the samples before that boundary through the existing
   `UnifiedEngineManager::transcribe()` path;
3. keeps the samples after the boundary (always at least 5 seconds) as the new
   pending buffer.

`finish()` decodes the remaining pending buffer (always shorter than 33
seconds) through the same path, which applies the existing balanced
segmentation when it is longer than 28 seconds. It then awaits the early
windows and the final window, and joins their non-empty texts with one space in
recording order.

For SenseVoice and Qwen3-ASR, the consumer keeps buffering the whole recording
and decodes it once in `finish()`.

The boundary policy is a pure function next to the existing Whisper
segmentation in `stt_engine/sherpa_onnx/engine.rs`, so both paths share the
same constants and energy measure.

## Data Contract

No IPC, event, settings, or persisted-data changes.

Internal boundary contract:

```rust
/// Returns the cut position (sample index, exclusive end of the window to
/// decode now) when the pending buffer holds enough audio to close a window.
pub(crate) fn incremental_whisper_boundary(pending: &[f32]) -> Option<usize>;
```

- Returns `None` while `pending.len()` is below 33 seconds of 16 kHz audio.
- Otherwise returns `Some(cut)` with `22 s <= cut <= 28 s` (in samples).

The existing `stt_duration_ms` metric keeps measuring the wall time of
`finish()`, which now represents the decoding still pending after the
recording stops.

## Acceptance Criteria

1. The boundary function returns `None` below 33 seconds of pending audio.
2. At or above 33 seconds it returns a cut between 22 and 28 seconds, and the
   remainder is at least 5 seconds.
3. The cut prefers the quietest audio in the search range.
4. For a Whisper recording of 33 seconds or less, the consumer issues exactly
   one decode request, in `finish()`, containing all samples.
5. For a longer Whisper recording, at least one decode request is issued before
   `finish()` is called.
6. Across all decode requests, the samples reconstruct the recording exactly
   once and in order, and no request exceeds 33 seconds.
7. The final text joins window texts in recording order, omitting empty ones.
8. Any failed window decode fails the whole transcription.
9. SenseVoice and Qwen3-ASR recordings issue exactly one decode request, in
   `finish()`.
10. Production logs report each early window decode with its index and length.

## BDD Scenarios

### Short dictation is unchanged

Given a local Whisper dictation lasting 20 seconds
When the recording stops
Then the backend decodes one window containing the whole recording.

### Long dictation is decoded while speaking

Given a local Whisper dictation lasting 90 seconds
When the recording is still in progress at 33 seconds
Then the backend has already started decoding the first window
And after the recording stops only the audio after the last early window
remains to decode.

### Text stays ordered and complete

Given a local Whisper dictation decoded in several windows
When the windows finish in any order
Then the final text contains each window text once, in recording order.

### A window failure is not hidden

Given a local Whisper dictation decoded in several windows
When one window fails to decode
Then the transcription fails instead of returning partial text.

## Verification

```bash
cd apps/desktop/src-tauri && cargo test stt_engine && cargo clippy --all-features -- -D warnings && cargo fmt -- --check
```

Production check after release: in `quality_metrics.db`, the median `stt_ms`
of local Whisper transcriptions whose recording lasts more than 60 seconds
drops well below the 19.5-second baseline.
