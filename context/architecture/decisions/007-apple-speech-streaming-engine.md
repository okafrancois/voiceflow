# ADR-007: Opt-in Apple SpeechAnalyzer Streaming Engine

**Date**: 2026-09-25
**Status**: Accepted

## Context

ADR-002 ranks accuracy above latency and forbids streaming recognition unless
the user explicitly requests it, the accuracy impact is known, and a fallback
is documented.

Production data shows the post-recording wait of local Whisper reaching a
median of 19.5 s for dictations above 60 s, while the native prototype using
Apple's on-device `SpeechAnalyzer` waits a median 1.2 s. Public English
benchmarks (Argmax, June 2025) place `SpeechTranscriber` between Whisper base
and Whisper small, below Whisper large-v3. No French benchmark exists.

## Decision

Add Apple `SpeechAnalyzer` as an **opt-in** local model, "Apple (fast)",
available on macOS 26 and later. Whisper stays the default and remains the
accurate choice.

- Explicit request: on 2026-09-25 the product owner asked for this engine as a
  user choice between speed and accuracy and waived a pre-release benchmark
  based on their own usage of the native prototype.
- Accuracy impact: expected below whisper-turbo per public English
  benchmarks; not measured for French.
- Fallback: when the engine is unavailable (older macOS, other platforms,
  unsupported locale), model resolution falls back to a downloaded local model.

The Swift-only API is reached through a Swift package (`swift/AppleSpeech`)
compiled by `build.rs` for the Rust target triple and linked statically behind
a small C ABI, instead of a sidecar process, to avoid an extra signed binary
and IPC latency.

## Alternatives Considered

- **Swift sidecar process** (Tauri `externalBin`): isolates the Swift runtime
  but adds a separately signed binary and IPC on every audio chunk.
- **swift-rs crate**: builds Swift packages from `build.rs`, but always
  compiles for the host architecture, which breaks Intel and universal builds.
  A small `build.rs` step targets the Rust target triple instead.
- **objc2-speech** (`SFSpeechRecognizer`): reachable from Rust directly but it
  is the older, less accurate API with server-side limits.

## Consequences

- Users choose between fast and accurate engines in the model list.
- The macOS build compiles a Swift package when the macOS SDK is 26 or later.
  With an older SDK, or with `VOICEFLOW_DISABLE_APPLE_SPEECH` set, the build
  links a stub that reports the engine as unsupported.
- The engine is never recommended nor picked as an automatic fallback: only an
  explicit selection uses it.
- The Swift boundary is a small blocking C ABI; asynchronous Swift work stays
  behind it.
- ADR-002 still applies to cloud providers and to the default engine.
