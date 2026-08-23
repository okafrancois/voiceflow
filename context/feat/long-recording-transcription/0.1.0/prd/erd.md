# Long Recording Transcription Specification

## Version

- Feature: `long-recording-transcription`
- User-visible name: Complete Long Dictation
- Version: `0.1.0`
- Status: Completed

## Problem Statement

The local Whisper engine receives the complete 16 kHz recording but can return
only an early portion when a dictation lasts longer than one model window.
Production logs from 2026-08-23 show both sides of the mismatch:

- a 77.25-second recording reached the engine as 1,235,965 samples, but its
  transcription stopped mid-sentence after 355 characters;
- a 121.14-second recording reached the engine as 1,938,253 samples, but its
  transcription contained only 191 characters.

The recorder, stop-time tail flush, channel consumer, and local buffering
consumer all delivered their complete inputs in those sessions. The loss occurs
when one sherpa-onnx Whisper stream decodes the full long buffer.

## Goal

Local Whisper transcription must decode every part of a long recording without
changing short-recording behavior or the capture lifecycle.

## Non-Goals

1. Do not change cloud STT streaming behavior.
2. Do not change SenseVoice or Qwen3-ASR decoding in this version.
3. Do not add frontend state or user-configurable segmentation settings.
4. Do not add overlapping windows or text-level deduplication in this version.

## First-Principles Model

1. The recorded samples are the source of truth. A successful long-form strategy
   must account for every input sample exactly once.
2. Whisper decoding quality is reliable on bounded model windows and unreliable
   for the observed 60-to-121-second single-stream inputs.
3. A split near low-energy audio is less likely to cut a spoken word than a
   fixed timestamp split.
4. Balanced windows avoid a very short final window while keeping every window
   below the decoder limit.
5. Segmentation belongs in the headless local STT engine. The recorder and
   frontend must not decide how a model consumes long audio.

## Architecture

`SherpaOnnxEngine::transcribe()` remains the batch entry point. Before decoding,
it derives contiguous sample ranges for the selected engine:

- Whisper input at or below 28 seconds uses one unchanged range;
- longer Whisper input uses balanced ranges of at most 28 seconds;
- each internal boundary moves toward the lowest-energy audio near its balanced
  target;
- SenseVoice and Qwen3-ASR continue to use one range.

The existing recognizer decodes each range in order while held by the same
exclusive lock. Empty segment results are discarded and the remaining text is
joined with one space.

## Data Contract

No IPC or persisted-data contract changes.

Internal segmentation contract:

```rust
struct SampleRange {
    start: usize, // inclusive, 16 kHz mono sample index
    end: usize,   // exclusive, 16 kHz mono sample index
}
```

For non-empty input, ranges must be ordered, contiguous, non-empty, start at
zero, and end at `samples.len()`.

## Acceptance Criteria

1. Whisper input of 28 seconds or less is decoded once with its original sample
   slice.
2. Whisper input longer than 28 seconds is decoded through multiple ranges, each
   no longer than 28 seconds.
3. Long-input ranges preserve all samples in their original order exactly once.
4. Internal boundaries prefer the lowest-energy candidate near their balanced
   target.
5. Segment text is returned in recording order with empty results omitted.
6. A segment decode error fails the complete transcription instead of returning
   partial text as success.
7. SenseVoice and Qwen3-ASR inputs keep their existing single-decode behavior.
8. Production logs report the segment count for long Whisper transcription.

## BDD Scenarios

### Decode a short Whisper recording unchanged

Given a local Whisper recording lasting 20 seconds
When the backend transcribes it
Then the recognizer decodes one segment containing the original samples.

### Decode a long Whisper recording completely

Given a local Whisper recording lasting 65 seconds
When the backend transcribes it
Then the recognizer decodes at least three ordered segments
And no segment exceeds 28 seconds
And concatenating the segment sample ranges reconstructs the original recording.

### Prefer a quiet boundary

Given a long recording with a low-energy pause near a balanced split target
When the backend derives the segment ranges
Then the boundary falls inside that pause.

### Preserve other local engines

Given a long SenseVoice or Qwen3-ASR recording
When the backend transcribes it
Then the recognizer receives one unchanged sample range.

### Fail atomically

Given a long Whisper recording split into multiple segments
And one segment cannot be decoded
When transcription finishes
Then the backend returns the decode error
And does not return the earlier segment text as a successful transcription.

## Verification evidence

1. Six inline engine tests cover short input, exact window boundaries, complete
   long-input coverage, quiet-boundary selection, engine-specific routing, text
   ordering, empty output, and error propagation.
2. A 100.83-second generated French fixture places `kangourou violet` only in
   its final sentence. The ignored real-model regression test passed with the
   installed Whisper Turbo model and found both marker words.
3. `cargo test --quiet` passed all non-ignored Rust tests. The library result was
   526 passed and 0 failed; the integration suites also passed.
4. `cargo clippy --all-features -- -D warnings` passed.
5. `cargo fmt -- --check` passed.
