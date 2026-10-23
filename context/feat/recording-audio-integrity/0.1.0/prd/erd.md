# Recording audio integrity 0.1.0

Status: Completed (local implementation and verification; not released).

## Goal and first principles

Preserve the recorded signal through stop-time conversion and retain the
original evidence according to the user's existing policy. Audio sample
accounting and actual WAV contents are stronger evidence than chunk counts.

## Problem

The 48 kHz to 16 kHz conversion creates a sinc resampler for each chunk,
processes it once, and discards the filter's delayed audio. The final fragment
must not be lost when recording stops. Independently, the transcription task
tries to save raw audio from mutable recording state already removed at stop,
so configured retention cannot provide playback or retry evidence.

## Contract

1. A finite resampling call returns the complete input duration, rounded to
   the nearest output frame, with the filter's trailing lookahead supplied.
   Empty input stays empty; equal rates preserve samples exactly.
2. Speech in the final milliseconds remains present in the output. Stream
   processing and the stop flush share this rule, with VAD disabled or bypassed.
3. Retained audio belongs to the transcription task that captured it and
   survives removal or replacement of global recording state.
4. Never retention writes no WAV; saved WAV samples and format match capture.
   Existing finalization, cancellation, and retention cleanup remain in force.
5. No new IPC, frontend behavior, engine selection, or settings are required.

## Architecture and data contract

`audio/resampler.rs` supplies zero lookahead for finite buffers and returns
only the duration corresponding to the input. `RecordingAudio` owns the optional
save path and shared raw PCM buffer; the transcription task owns this value.
`StreamingSttState` retains the channel and stop-flush state only. Persisted
history and settings formats are unchanged. There is no new UI.

## BDD scenarios

- Given 200 ms of 48 kHz mono with speech only at the end, when it is converted
  to 16 kHz, then it contains 3,200 samples and retains the ending signal.
- Given a two-millisecond remainder at stop, when the recording is flushed,
  then all 32 output samples reach the recognizer.
- Given retained audio with a final callback, when active recording state is
  removed, then the owning task can still save all original samples.
- Given Never retention, when recording finishes, then no WAV is written.

## Verification

- Failing-first signal tests for output duration, end-of-input energy, and a
  sub-filter-length stop fragment.
- Retention lifecycle tests with a temporary WAV, task-owned shared samples,
  Never retention, empty buffers, and invalid formats.
- Real Apple bridge experiment with French synthetic speech and a final
  marker; report this separately from deterministic tests and actual user audio.
- Rust tests, Clippy with all features, and formatting.

This contract prevents demonstrated audio loss; it does not promise perfect
recognition or establish the cause of an unavailable historical recording.

## Verification evidence (2026-10-23)

- The initial 200 ms regression failed: 3,156 output frames instead of 3,200.
- All 10 resampler tests pass, including final energy, short fragments,
  unchanged signal timing at the beginning, and invalid sample rates.
- The 48 kHz stop-flush regression and three audio persistence tests pass.
- Full Rust suite: 980 passed, 0 failed, 40 ignored across 29 suites.
- Desktop build, shared typecheck, i18n, and Rust formatting pass.
- Clippy with all features and warnings denied passes locally on Rust 1.98.0.
  CI on the published v1.4.0 commit has separate pre-existing lint failures;
  see the [investigation plan](../../../../plans/completed/2026-10-23-001-fix-dictation-tail-investigation-plan.md).
- Synthetic French speech through the unchanged Apple bridge retains its final
  marker with and without trailing silence. The original user audio is absent,
  so whole-phrase truncation remains unproven and needs a retained recording.
