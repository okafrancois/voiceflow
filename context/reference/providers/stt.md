# Shipped speech transcription services

This reference describes the cloud and local-service speech-to-text providers that Voice Flow can dispatch today. The provider IDs and field definitions come from `apps/desktop/src-tauri/src/provider_schema.rs`.

## Provider list

| Provider ID | Display name | Protocol | Runtime |
|-------------|--------------|----------|---------|
| `volcengine-streaming` | Volcengine Streaming | Volcengine binary WebSocket | `VolcengineStreamingClient` |
| `aliyun-stream` | Aliyun Realtime | DashScope Realtime JSON WebSocket | `AliyunStreamClient` |
| `elevenlabs` | ElevenLabs | Scribe v2 Realtime JSON WebSocket | `ElevenLabsStreamingClient` |
| `mlx-audio` | MLX Audio (Local) | Loopback HTTP multipart WAV | `MlxAudioClient` |

No other service provider ID is supported. OpenAI's hosted Whisper, OpenAI Realtime, Deepgram, Volcengine Flash, and arbitrary remote batch endpoints are not exposed by the backend. The MLX Audio provider uses locally downloaded Whisper or other MLX-compatible recognition checkpoints.

## Shared recording contract

All four providers implement the same recording lifecycle:

1. The backend creates a `StreamingSttClient` from the saved provider ID.
2. `connect()` completes provider setup: a WebSocket handshake for vendor services, or a local model-list request for MLX Audio.
3. The recording pipeline forwards 16 kHz mono PCM chunks through the provider audio channel.
4. `finish()` closes the audio channel, finalizes the provider request, and waits for a final transcript.
5. `close()` releases the connection or discards a buffered local session without transcribing it.

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

## MLX Audio (Local)

### Managed engine on Apple Silicon

Apple Silicon Macs offer managed MLX models and built-in Apple Speech as their local transcription engines. Intel Macs, Windows, and Linux retain their compatible native engines. Managed MLX requires macOS 14 or newer; the hardware policy still excludes native alternatives on older Apple Silicon systems. Initial catalogue IPC awaits the actual current-language Apple Speech availability asynchronously, with a ten-second deadline rather than treating an unfinished background probe as unsupported. Language changes reload the catalogue; direct settings selection requires Apple Speech to be ready before modifying selection or cloud routing. Vendor cloud services remain optional and unchanged. See the [shared Apple Silicon model policy](../../feat/apple-silicon-model-policy/0.1.0/prd/erd.md).

STT catalogue `downloadable` metadata authorizes asset installation for Apple Speech only when its status is `AssetsMissing`. The existing download command performs installation; it does not select the model or disable cloud routing. Ready/unsupported-language states do not offer that action. Built-in models still show no fixed checkpoint size or delete action. The frontend ignores superseded catalogue responses so an older language request cannot replace newer readiness.

| Model ID | Checkpoint | Approximate weights |
|----------|------------|---------------------|
| `mlx-whisper-turbo` | `mlx-community/whisper-large-v3-turbo-asr-fp16` | 1.61 GB |
| `mlx-parakeet-v3` | `mlx-community/parakeet-tdt-0.6b-v3` | 2.51 GB |
| `mlx-qwen3-asr-0.6b` | `mlx-community/Qwen3-ASR-0.6B-8bit` | 1.01 GB |

Select and download a model in the local model settings. Voice Flow installs its private, pinned Python 3.12.15 / MLX Audio 0.6.1 environment automatically, showing runtime installation separately from checkpoint progress. No system Python, Homebrew, Terminal command, or listening HTTP server is required. Multiple checkpoints may be installed; model switching releases the previous managed worker. Unmeasured quality and speed scores are not presented as benchmarks.

Runtime and complete checkpoint availability are required before a model is considered downloaded. All required files use pinned Hugging Face revisions and exact sizes. Inference runs offline using app-private checkpoint paths and temporary 16 kHz mono PCM16 WAV files, with a versioned JSON-lines protocol over process pipes. Failed installation/inference is reported without silent native fallback. Unload and application shutdown stop the owned worker; unrelated local servers and the separate MLX Studio installation are not controlled.

Readiness queries do not spawn Python. Download validates an existing runtime before reuse and repairs broken dependencies; confirmed worker-startup failure invalidates only its readiness marker so Download can repair it. Ordinary cancellation preserves a healthy runtime. User cancellation drops active MLX inference, including spawned recording decode tasks, instead of leaving GPU work running. File-job cancellation is rejected once final delivery has begun; accepted cancellation prevents history and delivery.

Dictation, media import, history retranscription/retry, and headless backend callers use the unified local manager. Whisper receives normalized language codes and supported initial prompts; Qwen receives language names and its supported system prompt; Parakeet detects the language. A legacy native selection migrates to managed Whisper Turbo. An active legacy `mlx-audio` HTTP configuration migrates to its curated local model when recognized. Cloud credentials, unrelated cloud routing, language, and text polish are preserved. MLX text polish is not implemented by this STT change.

An explicit real-runtime check installs only into a caller-provided isolated directory:

```sh
VOICEFLOW_MLX_TEST_ROOT=/tmp/voiceflow-managed-mlx-validation \
VOICEFLOW_MLX_TEST_WAV=/absolute/path/french-sample.wav \
VOICEFLOW_MLX_SECOND_MODEL=mlx-qwen3-asr-0.6b \
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --test managed_mlx_test \
  managed_mlx_installs_and_transcribes_real_french_audio_without_a_server -- --ignored --nocapture
```

The French fixture should contain “Bonjour, je teste une voix française directement sur mon Mac.” This opt-in check can download the runtime/checkpoints and needs several GB of disk space. It verifies preload readiness, both selected models when requested, complete transcription, concurrent same-model requests with different languages reusing the preloaded worker, unload, and private-audio cleanup. See the [managed MLX specification](../../feat/mlx-audio-stt/0.2.0/prd/erd.md).

### Performance evidence and model-selection data

The managed catalogue has no measured MLX speed/accuracy ranking yet. Its zero scores mean unknown, not poor performance. Keep infrastructure explanations out of the selector and omit empty descriptions; a note explaining rating gauges is unnecessary when no gauges are shown.

Published results are available in the [NVIDIA Parakeet v3 card](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3), [Qwen3-ASR 0.6B card](https://huggingface.co/Qwen/Qwen3-ASR-0.6B), and [OpenAI Whisper Turbo card](https://huggingface.co/openai/whisper-large-v3-turbo). They do not establish the performance of Voice Flow's three pinned MLX checkpoints on the same Mac. In particular, Qwen's high-concurrency throughput and vLLM streaming support are not measurements of the shipped MLX 8-bit adapter. Do not turn vendor throughput or multilingual aggregate WER into unsourced 0–10 local rankings.

A decision-useful benchmark should record:

- **Speed:** audio duration divided by warm inference duration, plus median/p90 post-recording latency. Report cold model-loading separately and retain device, memory, runtime version, checkpoint revision/precision, clip duration, mode, and sample count.
- **Accuracy:** WER on the same fixed, licensed French corpus with verified reference transcripts and a versioned normalization procedure. Lower WER is better; it is not automatically a general-purpose accuracy percentage. Keep raw transcription and polished output evaluations separate.
- **Scope:** compare the three shipped checkpoints with identical audio and settings, include long clips and representative accents/noise, and publish the corpus/protocol/source/date. A successful short French fixture is a regression check, not an accuracy benchmark.

These measurements are a follow-up, not implemented catalogue data. Unknown values remain unknown until the benchmark exists.

### Text streaming: capability versus integration

Verified against the installed MLX Audio **0.6.1** package on 2026-10-24:

| Model family | Available upstream API | Important boundary |
|--------------|------------------------|--------------------|
| Whisper | `generate_streaming()` and `StreamingDecoder.decode_chunk()` | AlignAtt has stateful chunk processing; the convenience generator still accepts an existing audio input. |
| Parakeet | `stream_generate()` | Produces chunk results from a supplied file/waveform, not a microphone-input protocol. |
| Qwen3-ASR | `stream_transcribe()` / `generate(stream=True)` | Emits text tokens from supplied audio; this alone does not accept continuously arriving microphone chunks. |

Primary implementation references: [Whisper model](https://github.com/Blaizzy/mlx-audio/blob/v0.6.1/mlx_audio/stt/models/whisper/whisper.py), [Whisper AlignAtt](https://github.com/Blaizzy/mlx-audio/blob/v0.6.1/mlx_audio/stt/models/whisper/streaming.py), [Parakeet](https://github.com/Blaizzy/mlx-audio/blob/v0.6.1/mlx_audio/stt/models/parakeet/parakeet.py), and [Qwen3-ASR](https://github.com/Blaizzy/mlx-audio/blob/v0.6.1/mlx_audio/stt/models/qwen3_asr/qwen3_asr.py).

**Managed MLX dictation now emits live replacement previews.** `BufferingConsumer` opts into one coalesced background decoder when its partial callback is registered. After at least two seconds of fresh audio, it decodes at most the most recent thirty seconds through the existing selected model/worker. At most one snapshot is in flight, with a fifteen-second preview timeout. Optional context capture is polled concurrently so it cannot delay audio ingestion. All-zero snapshots are skipped; this is not a general silence detector. The existing family-specific prompts/language options remain unchanged.

The pill follows the newest lines; the dashboard displays the complete current preview. These are replaceable recent-window results, not committed transcript segments or a common stateful/token-stream API. Stopping closes publication, allows at most two seconds of graceful drain, then cancels and awaits cleanup before decoding the entire original recording. Every sample, including the final partial chunk, reaches final inference. Only that authoritative final result enters correction, polish, history or delivery. Cancellation/drop releases owned preview work; worker restart may be needed after cancellation. Native incremental behavior and import/retry batch behavior remain unchanged.

The [live transcription specification](../../feat/mlx-live-transcription/0.1.0/prd/erd.md) defines lifecycle and event contracts. An ignored `managed_mlx_live_test` replays real French audio into all three checkpoints and verifies actual partials before stop, final French text and closed publication. Controlled decoder tests prove bounded work and full-audio retention. Fixture results are not general accuracy/latency benchmarks or live-microphone acceptance.

### Legacy external HTTP connector

The following compatibility connector is not offered on Apple Silicon. It does not install or start MLX and should not be confused with the managed local engine.

| Field | Value |
|-------|-------|
| Provider ID | `mlx-audio` |
| Default Base URL | `http://127.0.0.1:8000/v1` |
| Default model | `mlx-community/whisper-large-v3-turbo-asr-fp16` |
| Authentication | None |
| Connection check | `GET /v1/models`, without audio or model loading |
| Transcription | `POST /v1/audio/transcriptions`, multipart WAV, `response_format=json` |
| Audio | Lossless PCM, 16 kHz, 16-bit, mono |

For the retained compatibility connector, start the MLX Audio server before dictating. No API key is needed. The default API server's model list includes loaded models only, so an empty list does not mean the downloaded checkpoints are unavailable. The connection check rejects a text-only llama.cpp server instead of accepting it as an audio-capable endpoint.

This provider buffers the recording and sends the complete WAV after release. All chunks, including the final partial chunk, are drained before submission. It emits a final transcript without simulated partial results. Cancellation before finish submits no audio. HTTP(S) endpoints must use localhost or a loopback IP; proxies and redirects are disabled. Connection requests are bounded to five seconds and inference requests to five minutes. A failed request is reported without silently switching engines.

Language tags such as `fr-FR` become `fr`; automatic language selection omits the language field. The current MLX Audio HTTP transcription contract does not forward Voice Flow's contextual glossary/initial-prompt fields. Existing post-transcription correction and polish remain available. The history identifies this provider as local. File import and history retry retain their existing native-local-engine policy.

An explicit local inference check is available after starting the server:

```sh
cd apps/desktop/src-tauri
VOICEFLOW_MLX_TEST_WAV=/absolute/path/french-sample.wav cargo test --test mlx_audio_stt_test mlx_audio_transcribes_a_real_local_french_recording -- --ignored --nocapture
```

The manual fixture should contain the French phrase about testing a French voice directly on a Mac. This check is ignored by default and does not download a model by itself; prepare the server and its checkpoint first.

## Deterministic contract tests

These tests run against local WebSocket or HTTP servers. They need no vendor credentials or internet connection.

```bash
cd apps/desktop/src-tauri
cargo test --test volcengine_streaming_mock_test
cargo test --test cloud_stt_provider_contract_test
cargo test --test mlx_audio_stt_test
cargo test --test cloud_stt_streaming_lifecycle_test
```

They cover handshake headers, URL parameters, initial session messages, audio framing, final packets, response parsing, callbacks, and unsupported provider dispatch.

Live auth-rejection checks are ignored by default because they depend on vendor availability and network access:

```bash
cd apps/desktop/src-tauri
cargo test --test cloud_stt_streaming_lifecycle_test -- --ignored --nocapture
```

An ignored test reaching an auth error proves only that the vendor accepted the URL and request shape far enough to reject the credentials. It does not prove account access, transcription quality, billing status, or successful audio recognition.
