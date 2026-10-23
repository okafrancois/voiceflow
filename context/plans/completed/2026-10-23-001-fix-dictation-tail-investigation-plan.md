---
title: Dictation tail investigation and published branch alignment
type: fix
status: completed
date: 2026-10-23
---

## Overview

Investigate missing endings in production dictations and restore the principal
branch to the latest published Tauri application.

## Problem Frame

- Installed and published application: v1.4.0, commit `c39026e`.
- `master` remained at `aa75c71` (v1.2.3 era), 19 commits behind v1.4.0.
- The native Swift prototype was removed in `0223a76`; the branch name survived.
- Current dictations use Apple Speech in French and Apple Intelligence with the
  filler-removal profile. The user specifically reports missing endings.
- The user confirmed the ending is also absent in Voice Flow history, placing
  the reported loss before text injection. A normalized-word comparison of
  150 recent raw/final pairs found no trailing deletion/replacement involving
  four or more source words. This narrows the likely stage to capture/STT,
  without proving whether the absent speech reached the recognizer.
- All 62 capture starts on October 23 used 48 kHz mono input. Logs show no
  enqueue errors or VAD-dropped chunks. Of 58 successful history rows, none
  lost more than 45% of text during postprocessing/polish. This does not prove
  the audio or recognized text was complete.
- Audio retention is set to seven days, but recent recordings have no WAV.
  `stop_recording_sync` takes `streaming_stt` before the consumer finishes;
  capture then looks up that removed state to save audio.

## Scope Boundaries

Investigate the final audio flush, resampling, Apple finalization, and audio
retention. Fix only demonstrated defects under the existing Apple speech,
long-recording, and privacy-retention contracts. Do not change the user's
engine or publish/install a new application without a request. Older three-channel
capture failures are a separate finding unless needed for the tail fix.

## Implementation Units

1. Align local/remote `master` with the published tag using a fast-forward.
2. Reproduce stop-time loss with deterministic signal tests and a generated
   French fixture through the real Apple bridge when available.
3. Add failing-first tests and minimal fixes for demonstrated causes.
4. Verify Rust tests, Clippy, formatting, applicable frontend contracts, and
   document what remains unproven without the original recordings.

## System-Wide Impact

Audio preprocessing and retention are shared by local and cloud engines.
Preserve the backend-owned lifecycle, cancellation, and Never retention policy.

## Risks & Dependencies

Original audio is unavailable, so a synthetic reproduction cannot conclusively
identify every historical omission. Apple integration checks require macOS 26
and installed French speech assets. Repository/installed-app versions must be
reported separately; updating Git does not update the running application.

## Verification Evidence

- GitHub release inspected explicitly with `-R okafrancois/voiceflow` (bare
  `gh` defaults to the upstream AriaType repository).
- `git rev-list --left-right --count origin/master...v1.4.0`: `0 19` before alignment.
- `git switch master && git merge --ff-only v1.4.0 && git push origin master`: passed.
- Local HEAD, origin/master and peeled v1.4.0 tag all resolve to
  `c39026e3ef74983758a6eee864a5d905554b6f05` after alignment.

## Findings and implemented changes

- The failing signal regression returned 3,156 frames for a 9,600-frame,
  48 kHz input (expected 3,200 at 16 kHz). The conversion discarded the sinc
  filter lookahead on each call. Padding the finite input and trimming only
  the output padding preserves duration and end-of-input energy. Rubato 0.15
  starts at the input origin; trimming `output_delay()` incorrectly removes
  real leading audio, as the initial attempted correction demonstrated.
- `RecordingAudio` now keeps the optional path and shared raw buffer in the
  transcription task. Stop can remove the active state without losing the
  original recording. Never retention and cancellation still skip persistence.
- Two standalone probes compiled the unchanged Swift bridge and transcribed
  synthetic French speech in 200 ms chunks. Both the normal ~18.7 s fixture
  and a version trimmed to the final audible samples retained the final
  marker, `kangourou violet`. These probes do not reproduce the user's lost
  whole-phrase ending and do not establish Apple as the root cause.
- Apple's documented `finalizeAndFinishThroughEndOfInput()` waits for all
  input and final results; the bridge already calls it and awaits its result
  collector. No speculative change to this finalization was made.
  Reference: https://developer.apple.com/documentation/speech/speechanalyzer/finalizeandfinishthroughendofinput()

## Final verification

- Standalone harness using the actual repository resampler and test file:
  failing-first length assertion (3,156 vs 3,200), then all 10 tests passed.
- Retention tests failed to compile before the task-owned audio API existed;
  `cargo test --lib audio::wav_writer -- --nocapture`: 3 passed afterward.
- Desktop build, shared typecheck, and i18n: passed (existing bundle size and
  unused-translation warnings).
- Markdown links: passed.
- Full `cargo test`: 980 passed, 0 failed, 40 ignored across 29 suites, including
  the two-millisecond 48 kHz stop-tail regression and all retention checks.
- `cargo fmt -- --check`: passed.
- `cargo clippy --all-features -- -D warnings`: passed locally with Homebrew
  Rust/Clippy 1.98.0. This differs from the unpinned stable toolchain in CI.
- An initial concurrent desktop build deleted Cargo artifacts via its
  `clean:rust` script. Rust verification was restarted after it completed;
  the testing guide now documents this dependency.

The bounded investigation, branch alignment, and demonstrated fixes are
complete. This is not a claim that every reported missing ending is resolved:
the real-audio follow-up below and a future release remain separate work.

## Separate follow-ups and limits

- October 17 contained two empty ~148–152 s dictations using a three-channel
  input. Capture and stop downmix only two-channel input. The first session
  also announced stale 24 kHz mono parameters before the actual capture
  opened at 48 kHz, three channels. Recorder startup reuses cached parameters.
  These defects are outside this mono end-of-dictation slice; October 23 used
  only 48 kHz mono input. They must not be presented as the cause of today's
  missing endings.
- CI run 40012962883 on the exact published commit passes frontend contracts
  but fails Clippy: `double_must_use` emitted from existing `async_trait`
  macros on macOS and Windows, plus an existing Windows-only `unused_mut`
  in `services/transcription_workbench.rs:245`. No new code was in that run.
- Git is on `master`, but T3's existing thread metadata still reports
  `native-app`. Its metadata update tool explicitly cannot change a thread's
  branch/workspace; no unsupported state edit was attempted.
- The installed app remains v1.4.0. Source fixes are local and are not a new
  published or installed release. A real problematic recording after a build
  containing retention repair is needed to attribute whole-phrase loss.

## Subsequent authorized release

The user subsequently requested publication. [Release v1.4.1](./2026-10-23-002-release-v1.4.1-plan.md)
ships the verified fixes from `master`, resolves the pre-existing Clippy errors,
and passes macOS/Windows/frontend CI and published artifact verification.
The installed application was not replaced during publication. The need for
original audio to diagnose any remaining whole-phrase omission still applies.
