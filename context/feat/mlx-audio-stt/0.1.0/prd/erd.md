---
title: Local MLX Audio dictation
version: 0.1.0
status: completed
date: 2026-10-24
---

# Local MLX Audio dictation

## Outcome

Voice Flow records through its existing shortcut and audio pipeline, submits the complete recording to a local MLX Audio server after release, and inserts the resulting transcript through the existing delivery pipeline.

## Contract

- Add provider ID `mlx-audio`, displayed as MLX Audio (Local), to the existing speech-service configuration and recording-consumer dispatch.
- Default to `http://127.0.0.1:8000/v1` and `mlx-community/whisper-large-v3-turbo-asr-fp16`. Neither credentials nor a second model download are required.
- Only loopback HTTP(S) endpoints are accepted. Do not follow redirects or use an HTTP proxy for this local provider.
- Connection checks call `GET /v1/models` without recording or transcribing user audio. An empty loaded-model list is valid: MLX Audio loads cached models on demand.
- Buffer every 16 kHz mono PCM chunk, including the last partial chunk. On finish, drain the audio channel and send one multipart WAV request to `/v1/audio/transcriptions`, with `response_format=json`, the selected model, and an optional normalized language.
- Propagate transport, HTTP, and malformed-response failures. Do not silently switch models or return a successful transcript on failure.
- Closing a session discards its buffered audio without submitting it. Empty sessions do not trigger inference.
- Existing shortcuts, preprocessing, transcript delivery, other providers, and local-engine selection remain available. History identifies MLX Audio as local.
- The UI explains that this provider runs locally and requires the MLX Audio server to be running; it does not ask for an API key.

## Acceptance evidence

Deterministic HTTP tests must inspect WAV samples, model and language fields, no credentials, late audio chunks, empty/cancelled sessions, malformed JSON, and server failures. Run a separate explicitly ignored integration test against the actual local server and a French audio fixture. Verify the built desktop UI and install a local development bundle for testing.

## Scope

Dictation and its connection/settings/history integration. TTS, server lifecycle management, cloud provider protocols, release publication, and replacement of the signed production application are outside this change. File-import/retry paths keep their existing local-engine policy.

## Verification status

Completed on 2026-10-24: deterministic HTTP contracts, full Rust/frontend checks, real local Whisper inference, signed development-bundle installation, native UI Connected check and a successful local dictation in the installed application. Evidence is recorded in the [execution plan](../../../../plans/completed/2026-10-24-002-feat-mlx-audio-stt-plan.md).
