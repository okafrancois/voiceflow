# Engine API Contract Testing

Engines (STT and Polish) abstract vendor API differences behind unified interfaces. Each engine must correctly construct requests for its vendor and parse vendor-specific responses into unified types.

## Engine Traits

### Unified SttEngine (recording lifecycle — all engines)

```rust
#[async_trait]
pub trait SttEngine: Send + Sync {
    fn engine_type(&self) -> EngineType;
    async fn send_chunk(&self, pcm_data: Vec<i16>) -> Result<(), String>;
    async fn finish(&self) -> Result<String, String>;
    fn set_partial_callback(&mut self, callback: PartialResultCallback);
}
```

All recordings follow the same consumer task pattern:
```rust
while let Some(chunk) = rx.recv().await {
    engine.send_chunk(chunk).await?;
}
let text = engine.finish().await?;  // channel closed = recording ended
```

Engine implementations decide internally what `send_chunk` does:
- **Local (`SherpaOnnxBufferingEngine`)**: `self.pcm_chunks.lock().push(pcm_data)` — just buffer
- **Cloud streaming (`StreamingSttClient`)**: forward to internal mpsc → WebSocket — real-time
- **Cloud non-streaming**: buffer, then write WAV + HTTP at `finish()`

### File-Based Transcription (non-recording)

For drag-drop and file import, `UnifiedEngineManager::transcribe()` provides a batch API separate from the `SttEngine` trait.

## Engine Types

| EngineType | Implementation | `send_chunk` | `finish` |
|------------|---------------|-------------|----------|
| `SenseVoice` | `SherpaOnnxBufferingEngine` | Buffer Vec<i16> | Flatten → f32 → sherpa-onnx transcribe |
| `Whisper` | `SherpaOnnxBufferingEngine` | Buffer Vec<i16> | Flatten → f32 → segment long audio → transcribe each range → join text |
| `Qwen3Asr` | `SherpaOnnxBufferingEngine` | Buffer Vec<i16> | Flatten → f32 → sherpa-onnx transcribe |
| `Cloud` | `StreamingSttClient` | Forward to WebSocket | Await WebSocket final result |

## Audio Source (for file-based transcription)

```rust
pub enum AudioSource {
    File(PathBuf),       // WAV file path (cloud non-streaming, drag-drop)
    Memory(Vec<f32>),    // f32 16kHz mono samples (local STT, zero file I/O)
}
```

Local STT uses `AudioSource::Memory` — audio is passed directly from the recording pipeline to sherpa-onnx without writing a temporary WAV file.

### Long local Whisper input

Whisper input longer than 28 seconds is decoded as contiguous, balanced sample
ranges. Each internal boundary moves to the lowest-energy point within three
seconds of its balanced target. Every sample appears in exactly one range.
Short Whisper input, SenseVoice input, and Qwen3-ASR input keep one decode call.

The engine joins non-empty segment results in recording order. If any segment
decode fails, the complete transcription fails instead of returning the earlier
segments as a successful result.

## Model Management

All local STT models are defined in `stt_engine/models.rs`:

| Model | Name | Engine | Size | Preferred Languages |
|-------|------|--------|------|---------------------|
| Whisper Tiny INT8 | `whisper-tiny` | Whisper | 99M | All languages |
| SenseVoice Small | `sense-voice-small` | SenseVoice | 234M | zh, yue, ja, ko, en |
| Whisper Base | `whisper-base` | Whisper | 279M | All languages |
| Qwen3-ASR 0.6B INT8 | `qwen3-asr-0.6b-int8` | Qwen3Asr | 838M | Manual selection |
| Whisper Medium INT8 | `whisper-medium` | Whisper | 902M | All languages |
| Whisper Small | `whisper-small` | Whisper | 925M | All languages |
| Whisper Turbo INT8 | `whisper-turbo` | Whisper | 989M | All languages |
| Whisper Large v3 INT8 | `whisper-large-v3` | Whisper | 1.69G | All languages |

Model recommendation: `is_sensevoice_preferred(lang)` returns true for zh/yue/ja/ko/en — these languages get SenseVoice Small. All others get Whisper Base. Qwen3-ASR and the additional Whisper sizes are manual, opt-in choices and are not auto-downloaded during onboarding.

Each model definition owns its repository, exact source filenames, runtime filenames, and verified file sizes. Downloads emit completion only after the installed files pass the model definition's completeness check. Small support files are required to be non-empty instead of being compared against rounded megabyte estimates.

## Auth Error Verification Pattern

Tests verify API contract correctness by expecting authentication errors from real endpoints with mock credentials.

```rust
mod mock_credentials {
    pub const API_KEY: &str = "mock_api_key_for_testing";
    pub const APP_ID: &str = "mock_app_id_for_testing";
}
```

| Error Type | Proves |
|------------|--------|
| 401/403 Auth Error | Endpoint URL, headers, and request body are correctly formed |
| 400 Bad Request | Request body or headers are malformed |
| 404 Not Found | Endpoint URL is incorrect |

## Test Requirements by Engine Type

| Engine Category | Test Requirement | Location |
|-----------------|------------------|----------|
| Local engines (SherpaOnnxBufferingEngine) | send_chunk accumulation, finish transcription, empty input, complete long Whisper segmentation | `src/stt_engine/` inline + `tests/` |
| Cloud STT engines | Real API calls with mock credentials to verify auth errors | `tests/cloud_provider_api_test.rs` |
| Cloud Polish engines | Real API calls with mock credentials to verify auth errors | `tests/cloud_provider_api_test.rs` |
| Response parsing (all vendors) | Mock server or recorded responses | `tests/common/mock_server.rs` |
| Consumer task lifecycle | send_chunk × N → finish produces correct result | `src/stt_engine/` inline |

## Test Locations

| Purpose | Location |
|---------|----------|
| Cloud STT API contract tests | `tests/cloud_provider_api_test.rs` |
| Cloud STT integration tests | `tests/cloud_stt_test.rs` |
| Streaming client tests | `tests/volcengine_streaming_test.rs`, `tests/volcengine_streaming_mock_test.rs` |
| Streaming lifecycle tests | `tests/cloud_stt_streaming_lifecycle_test.rs` |
| Model recommendation tests | `src/stt_engine/models.rs` (inline) |
| Engine manager tests | `src/stt_engine/unified_manager.rs` (inline) |
| VAD tests | `src/audio/stream_processor.rs` (inline) |

## Core Testing Principle

For cloud engines, tests must verify the API contract without requiring valid credentials or successful API responses. An auth error proves the endpoint, headers, and request body are correctly formed.
