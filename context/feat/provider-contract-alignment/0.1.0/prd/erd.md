---
title: Cloud provider contract alignment
version: 0.1.0
status: completed
date: 2026-08-24
---

# Cloud provider contract alignment

## Problem

The provider reference and Cloud Service specification list integrations that the desktop backend cannot dispatch. This makes the settings UI, connection checker, tests, and documentation disagree about what the product supports. Volcengine also keeps public code paths for lower-accuracy bidirectional interfaces even though the product contract requires `bigmodel_nostream`.

## Supported contracts

Cloud STT supports exactly these provider IDs:

| ID | Protocol | Default endpoint | Default model or resource |
|----|----------|------------------|---------------------------|
| `volcengine-streaming` | Volcengine binary WebSocket protocol | `wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_nostream` | `volc.bigasr.sauc.duration` |
| `aliyun-stream` | DashScope Realtime JSON WebSocket protocol | `wss://dashscope.aliyuncs.com/api-ws/v1/realtime` | `qwen3-asr-flash-realtime` |
| `elevenlabs` | ElevenLabs Scribe v2 Realtime JSON WebSocket protocol | `wss://api.elevenlabs.io/v1/speech-to-text/realtime` | `scribe_v2_realtime` when configured |

Cloud Polish supports exactly `anthropic` and `openai`. Anthropic uses the Messages API. OpenAI uses Chat Completions and may target a compatible endpoint through the OpenAI provider configuration.

## Requirements

1. `provider_schema.rs`, provider dispatch, connection checks, feature docs, and reference docs expose the same three STT and two Polish IDs.
2. Production Volcengine connections use `bigmodel_nostream`. The backend must reject official lower-accuracy `bigmodel` and `bigmodel_async` endpoints before opening a network connection.
3. The backend must reject unsupported Polish provider IDs instead of silently treating them as OpenAI.
4. Deterministic local contract tests must cover request construction, authentication headers, protocol messages, final response parsing, and dispatch for every supported provider.
5. Live endpoint checks must be ignored by default and require explicit credentials. A local test run must not depend on internet access or vendor availability.
6. Reference docs must separate shipped behavior from optional compatible endpoints. They must not claim support for unimplemented STT providers.

## Acceptance criteria

- The schema contract test finds exactly `volcengine-streaming`, `aliyun-stream`, and `elevenlabs` for STT.
- The schema contract test finds exactly `anthropic` and `openai` for Polish.
- Local mock-server lifecycle tests pass for all three STT providers.
- Local HTTP mock tests pass for both Polish providers, including their connection-check request.
- Unsupported STT and Polish provider IDs fail with an `Unsupported` error before network I/O.
- Official Volcengine bidirectional endpoints fail with an error that names `bigmodel_nostream`.
- `cargo test` passes without credentials or network access.
- Live credential tests remain available through ignored tests and are documented as optional.

## Out of scope

- Adding providers.
- Testing vendor service quality, pricing, uptime, or account entitlements.
- Changing credential storage or settings UI layout.
- Claiming that live vendor calls ran without credentials supplied through the documented environment variables.

## Verification status

Completed on 2026-08-24. Deterministic local contract tests passed for all five shipped cloud providers. Live checks remain ignored because no vendor network access or credentials were part of this verification.
