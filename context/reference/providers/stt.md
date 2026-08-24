# Shipped cloud STT providers

This reference describes the cloud speech-to-text providers that Voice Flow can dispatch today. The provider IDs and field definitions come from `apps/desktop/src-tauri/src/provider_schema.rs`.

## Provider list

| Provider ID | Display name | Protocol | Runtime |
|-------------|--------------|----------|---------|
| `volcengine-streaming` | Volcengine Streaming | Volcengine binary WebSocket | `VolcengineStreamingClient` |
| `aliyun-stream` | Aliyun Realtime | DashScope Realtime JSON WebSocket | `AliyunStreamClient` |
| `elevenlabs` | ElevenLabs | Scribe v2 Realtime JSON WebSocket | `ElevenLabsStreamingClient` |

No other cloud STT provider ID is supported. OpenAI Whisper, OpenAI Realtime, Deepgram, Volcengine Flash, and custom batch endpoints are not exposed by the backend.

## Shared recording contract

All three providers implement the same recording lifecycle:

1. The backend creates a `StreamingSttClient` from the saved provider ID.
2. `connect()` completes the WebSocket handshake and provider setup.
3. The recording pipeline forwards 16 kHz mono PCM chunks through the provider audio channel.
4. `finish()` closes the audio channel, sends the provider's final packet, and waits for a final transcript.
5. `close()` releases the WebSocket connection.

The connection check uses the saved active configuration. It connects and closes without sending user audio.

## Volcengine Streaming

### Contract

| Field | Value |
|-------|-------|
| Provider ID | `volcengine-streaming` |
| Endpoint | `wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_nostream` |
| Resource ID default | `volc.bigasr.sauc.duration` |
| App ID header | `X-Api-App-Key` |
| Access token header | `X-Api-Access-Key` |
| Resource header | `X-Api-Resource-Id` |
| Connection header | `X-Api-Connect-Id` |
| Audio | PCM, 16 kHz, 16-bit, mono |
| Recommended chunk | 1,600 samples, about 100 ms |

Voice Flow uses only `bigmodel_nostream`. Official `bigmodel` and `bigmodel_async` URLs are rejected before the connection attempt because the product prioritizes transcription accuracy over lower latency. A custom non-Volcengine host remains possible for a test server or an explicit compatible proxy.

The initial binary request enables inverse text normalization and punctuation, disables disfluency removal, and sends `result_type: "full"`. This keeps repeated or hesitant speech available to the Polish stage while retaining provider number and punctuation normalization.

Official references:

- [Volcengine speech recognition documentation](https://www.volcengine.com/docs/6561/162929?lang=en)
- [Voice Flow decision record for `bigmodel_nostream`](../../architecture/decisions/002-nostream-volcengine.md)

## Aliyun Realtime

### Contract

| Field | Value |
|-------|-------|
| Provider ID | `aliyun-stream` |
| Default endpoint | `wss://dashscope.aliyuncs.com/api-ws/v1/realtime` |
| Default model | `qwen3-asr-flash-realtime` |
| Authentication | `Authorization: Bearer <api-key>` |
| Compatibility header | `OpenAI-Beta: realtime=v1` |
| Audio | Base64 PCM, 16 kHz, 16-bit, mono |
| Recommended chunk | 1,600 samples, about 100 ms |

The model is appended as the `model` query parameter. Voice Flow sends `session.update` with text-only output, PCM input, the selected language, and `turn_detection: null`. It then sends `input_audio_buffer.append` events. `finish()` sends `input_audio_buffer.commit`, followed by `session.finish`, and waits for `session.finished`.

Alibaba now recommends workspace-specific domains. The default legacy DashScope domain remains supported by Alibaba. Users who have a workspace-specific endpoint can enter its base URL in the provider settings. Voice Flow adds the configured model query parameter.

Official references:

- [Qwen-ASR-Realtime interaction flow](https://www.alibabacloud.com/help/en/model-studio/qwen-asr-realtime-interaction-process)
- [Qwen-ASR-Realtime client events](https://www.alibabacloud.com/help/en/model-studio/qwen-asr-realtime-client-events)

## ElevenLabs

### Contract

| Field | Value |
|-------|-------|
| Provider ID | `elevenlabs` |
| Endpoint | `wss://api.elevenlabs.io/v1/speech-to-text/realtime` |
| Default model | `scribe_v2_realtime` |
| Authentication | `xi-api-key: <api-key>` |
| Audio format query | `audio_format=pcm_16000` |
| Audio | Base64 PCM, 16 kHz, 16-bit, mono |
| Recommended chunk | 16,000 samples, about 1 second |

The client adds `language_code` when a language is selected and always adds `model_id`. Audio travels in `input_audio_chunk` messages. `finish()` sends an empty audio chunk with `commit: true` and waits for `committed_transcript`.

For the first audio chunk, Voice Flow combines the glossary, work domain, subdomain, and initial prompt into ElevenLabs `previous_text`. Later chunks do not repeat that context.

Official references:

- [ElevenLabs Realtime STT API](https://elevenlabs.io/docs/api-reference/speech-to-text/v-1-speech-to-text-realtime)
- [ElevenLabs transcripts and commit strategies](https://elevenlabs.io/docs/eleven-api/guides/how-to/speech-to-text/realtime/transcripts-and-commit-strategies)

## Deterministic contract tests

These tests run against local WebSocket servers. They need no vendor credentials or internet connection.

```bash
cd apps/desktop/src-tauri
cargo test --test volcengine_streaming_mock_test
cargo test --test cloud_stt_provider_contract_test
cargo test --test cloud_stt_streaming_lifecycle_test
```

They cover handshake headers, URL parameters, initial session messages, audio framing, final packets, response parsing, callbacks, and unsupported provider dispatch.

Live auth-rejection checks are ignored by default because they depend on vendor availability and network access:

```bash
cd apps/desktop/src-tauri
cargo test --test cloud_stt_streaming_lifecycle_test -- --ignored --nocapture
```

An ignored test reaching an auth error proves only that the vendor accepted the URL and request shape far enough to reject the credentials. It does not prove account access, transcription quality, billing status, or successful audio recognition.
