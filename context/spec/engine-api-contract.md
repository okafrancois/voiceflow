# Engine API contract testing

STT and Polish engines hide provider-specific protocols behind backend-owned interfaces. Contract tests must prove request construction and response parsing without moving provider logic into the frontend.

## Recording consumer

Every live recording uses `RecordingConsumer`:

```rust
#[async_trait]
pub trait RecordingConsumer: Send + Sync {
    async fn send_chunk(&self, pcm_data: Vec<i16>) -> Result<(), String>;
    async fn finish(&self) -> Result<String, String>;
    fn set_partial_callback(&mut self, callback: PartialResultCallback) {}
}
```

The recording pipeline owns the lifecycle:

```rust
while let Some(chunk) = rx.recv().await {
    consumer.send_chunk(chunk).await?;
}
let text = consumer.finish().await?;
```

Local `BufferingConsumer` instances collect PCM and call the local engine at `finish()`. Cloud `StreamingConsumer` instances connect before they are returned, forward PCM to the selected WebSocket client, drop the audio sender at `finish()`, and wait for the provider's final result.

## Batch transcription

`UnifiedEngineManager::transcribe(engine_type, TranscriptionRequest)` is the local batch API. `TranscriptionRequest` carries in-memory 16 kHz mono samples, language, model name, and an optional initial prompt. Cloud STT is not available through this batch function.

Long Whisper input is split into contiguous sample ranges. The ranges cover every sample exactly once and move internal boundaries toward nearby low-energy points. A failed range fails the whole transcription instead of returning a partial success.

## Shipped cloud providers

Cloud STT dispatch accepts exactly:

- `volcengine-streaming`
- `aliyun-stream`
- `elevenlabs`

Cloud Polish dispatch accepts exactly:

- `anthropic`
- `openai`

`provider_schema.rs` is the UI-facing list. Backend dispatch must reject any ID missing from that schema. Schema endpoint and model defaults must use the same constants as the runtime clients.

Volcengine has an extra invariant. Production connections use only `wss://openspeech.bytedance.com/api/v3/sauc/bigmodel_nostream`. The backend rejects official `bigmodel` and `bigmodel_async` paths before network I/O.

See [the STT reference](../reference/providers/stt.md) and [the Polish reference](../reference/providers/polish.md) for request fields and endpoints.

## Deterministic provider tests

The default suite must not need internet access, vendor credentials, billing, or account permissions. Local mock servers verify the complete contract.

| Contract | Required evidence | Test location |
|----------|-------------------|---------------|
| Volcengine STT | Handshake headers, initial binary request, audio packet flags, final frame parsing | `tests/volcengine_streaming_mock_test.rs`, `tests/cloud_stt_streaming_lifecycle_test.rs` |
| Aliyun STT | Bearer header, model query, `session.update`, audio append, commit, finish, final transcript | `tests/cloud_stt_provider_contract_test.rs` |
| ElevenLabs STT | API key header, query fields, `previous_text`, audio chunk, commit, final transcript | `tests/cloud_stt_provider_contract_test.rs` |
| Anthropic Polish | Messages path, auth and version headers, request JSON, response parsing, connection check | `tests/cloud_polish_mock_test.rs` |
| OpenAI Polish | Chat Completions path, bearer header, request JSON, response parsing, SSE preview, connection check | `tests/cloud_polish_mock_test.rs` |
| Provider membership | Exact IDs, endpoint defaults, model defaults, unsupported dispatch | `src/provider_schema.rs`, cloud integration tests |

Every mock-server lifecycle test must await its server task. A panic inside an unobserved task is not valid evidence.

## Optional live checks

Live checks are ignored by default. They may verify that a vendor reaches an authentication rejection or accepts configured credentials, but they remain dependent on DNS, internet access, vendor uptime, account entitlements, and current model availability.

```bash
cd apps/desktop/src-tauri
cargo test --test cloud_stt_streaming_lifecycle_test -- --ignored --nocapture
cargo test --test cloud_provider_api_test -- --ignored --nocapture
```

A `401` or `403` from a vendor can show that the endpoint and handshake were understood. It does not prove successful transcription or Polish output. A `400` may indicate a request contract mismatch, but vendor account policies can also affect the response. Record the exact observed response when using live checks as evidence.

## Local engine tests

Local engine tests remain responsible for:

- model selection and installed-file completeness;
- short and empty input;
- complete long-audio segmentation;
- prompt and language propagation;
- model cache behavior;
- failure propagation without partial success.

The strongest routine verification is the complete Rust suite, followed by Clippy and rustfmt:

```bash
cd apps/desktop/src-tauri
cargo test
cargo clippy --all-features -- -D warnings
cargo fmt -- --check
```
