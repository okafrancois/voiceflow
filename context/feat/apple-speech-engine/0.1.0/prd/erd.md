# Apple Speech Engine Specification

## Version

- Feature: `apple-speech-engine`
- User-visible name: Apple (fast)
- Version: `0.1.0`
- Status: Active

## Problem Statement

Local Whisper decodes audio after it is recorded, so the wait after the user
stops grows with the dictation length (median 4.5 s for 10–30 s recordings,
19.5 s above 60 s, production data 2026-08-24 → 2026-09-25). The native macOS
prototype uses Apple's on-device `SpeechAnalyzer`, which transcribes while the
user speaks: over 122 dictations its post-recording wait was a median 1.2 s,
of which recognition took about 0.1 s regardless of length.

Users want to choose between a fast engine and the more accurate Whisper
models.

## Goal

On macOS 26 or later, the user can select an "Apple (fast)" speech model in the
existing model list. Dictations then stream audio to `SpeechAnalyzer` while
recording, and only the end of the analysis remains after the recording stops.
On older systems and other platforms nothing changes.

## Non-Goals

1. No partial transcript display in this version.
2. No contextual vocabulary hints (glossary to `AnalysisContext`) in this
   version.
3. No change to Whisper, SenseVoice, Qwen3-ASR, or cloud providers.
4. No accuracy benchmark. The user accepted the accuracy tradeoff (ADR-007).

## First-Principles Model

1. The engine is a model choice like any other: selection, persistence,
   history, fallback, and file transcription reuse the existing model paths.
2. Availability is a backend fact (OS version, locale support, installed
   assets) exposed through `ModelInfo`. The frontend only renders it.
3. `SpeechAnalyzer` is Swift-only. A Swift package compiled into the app
   exposes a small C ABI; all asynchronous Swift work stays behind it and Rust
   calls it from blocking threads.
4. Audio already reaches consumers as 16 kHz mono PCM chunks. The Swift side
   converts them to the analyzer's preferred format.
5. When the engine cannot run, the existing model resolution falls back to a
   downloaded local model.

## Architecture

```
capture.rs ──(Apple model)──> AppleStreamingConsumer ──C ABI──> Swift AppleSpeech
          └─(other local)───> BufferingConsumer
UnifiedEngineManager ──> EngineInstance::Apple (batch: files, retry)
```

- Swift package `src-tauri/swift/AppleSpeech`, compiled by `build.rs` for the
  Rust target triple and linked statically on macOS with an SDK 26 or later.
  It owns sessions (SpeechAnalyzer + SpeechTranscriber), asset installation,
  and format conversion. Every entry point checks `#available(macOS 26, *)`.
- Rust module `stt_engine/apple`: safe wrappers, `AppleStreamingConsumer`
  (`RecordingConsumer`), and `AppleSpeechEngine` for batch requests. On
  non-macOS targets the module reports the engine as unsupported.
- Model catalog: `apple-speech`, engine `apple`, built in, no files.
- Locale: the dictation language setting. SpeechAnalyzer does not detect the
  language, so `auto` uses the system locale; the model row asks the user to
  set the dictation language.
- Opt-in: the engine is never recommended nor used as an automatic fallback.
- Runtime failure: if the session cannot start, the dictation falls back to the
  first downloaded non-built-in local model instead of being lost.

## Data Contract

- `EngineType::Apple`, serialized `"apple"`.
- `ModelDefinition` gains `built_in: bool`; `ModelInfo` exposes `built_in`.
- `apple-speech` is listed only when the OS supports `SpeechTranscriber`.
  `downloaded` means the speech assets for the dictation locale are installed.
- `download_model("apple-speech")` installs the assets for the dictation
  locale; `delete_model` is rejected for built-in models.
- History rows record `stt_engine = "apple"`, `stt_model = "apple-speech"`.

C ABI (all blocking except `feed`):

| Function | Result |
|----------|--------|
| `vf_apple_speech_status(locale)` | 0 ready, 1 assets missing, 2 locale unsupported, 3 OS unsupported |
| `vf_apple_speech_install(locale)` | empty string on success, error message otherwise |
| `vf_apple_speech_start(locale)` | session id > 0, or 0 on failure |
| `vf_apple_speech_feed(id, pcm16, count)` | queues 16 kHz mono samples |
| `vf_apple_speech_finish(id)` | JSON `{"text": …}` or `{"error": …}` |
| `vf_apple_speech_cancel(id)` | stops and discards the session |

## Acceptance Criteria

1. On macOS 26+, `get_models` lists `apple-speech` with `built_in = true`.
   Elsewhere it is absent.
2. Selecting `apple-speech` stores `stt_engine = "apple"`.
3. A dictation with the Apple model feeds audio during the recording and
   returns the transcript from `finish()`.
4. File transcription and retry work with the Apple model through the batch
   engine.
5. If the Apple model is selected but unavailable, the existing fallback picks
   a downloaded local model.
6. Deleting a built-in model returns an error; the UI shows no delete button
   and replaces the size with a built-in label.
7. The Rust crate builds, tests, and passes clippy on macOS; non-macOS builds
   compile the unsupported stub.

## BDD Scenarios

### Choose the fast engine

Given macOS 26 with the French speech assets installed
When the user selects "Apple (fast)" and dictates for 40 seconds
Then the transcript is returned shortly after the recording stops
And the history entry records the `apple` engine.

### Unsupported system

Given macOS 15
When the user opens the model list
Then "Apple (fast)" is not listed.

### Missing assets

Given macOS 26 without speech assets for the dictation language
When the user presses Download on "Apple (fast)"
Then the backend installs the assets and the model becomes selectable.

## Verification

```bash
cd apps/desktop/src-tauri && cargo test && cargo clippy --all-features -- -D warnings && cargo fmt -- --check
pnpm --filter @voiceflow/desktop build && pnpm --filter @voiceflow/shared typecheck && pnpm check:i18n
```

Manual: dictate with the Apple model in the local install build and check the
history row and post-recording latency.
