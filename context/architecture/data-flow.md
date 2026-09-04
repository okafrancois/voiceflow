# Data Flow

## Primary User Workflow

```
User holds hotkey
  → GlobalShortcut registered (tray.rs → commands/)
  → Backend snapshots original-target settings and the macOS target when enabled
  → Recording starts (audio/recorder.rs)
  → SttEngine created (local or cloud — recorder doesn't care)
  → Audio captured at device sample rate
  → StreamAudioProcessor: resample → 16kHz mono → denoise (if on) → Silero VAD
  → VAD-filtered chunks sent via mpsc channel
  → Spawned consumer task: while let Some(chunk) = rx.recv() { engine.send_chunk(chunk) }
User releases hotkey
  → Recording stops → mpsc channel closed
  → Consumer task: AWAIT engine.finish() — returns final transcription
  → Transcription result received
  → If polish enabled: text polished (local LLM or cloud API)
  → Final text delivery:
      target disabled → inject at current cursor
      foreground mode → activate and verify original app → inject at its cursor
      background mode → write to captured Accessibility field without activation
      targeted failure → do not use current focus; retain text and record failure
  → Session text and audio retained according to their independent local policies
```

## Last-transcription recovery flow

```text
User chooses Paste Last Transcription in the system tray
  → Tray dispatches the backend history action without focusing the main window
  → History selects the newest successful row with non-blank final text
  → text_injector inserts that final text into the focused target
  → History records inserted_keyboard, inserted_clipboard, or failed
```

The action reads only retained history. When text retention leaves no successful
result, it returns an error and does not keep a separate transcript cache.

## Local retention flow

The Rust backend owns both retention policies. The frontend only reads their current values, updates them through typed IPC, and renders the backend’s storage counts.

```text
Recording finalization
  → audio policy is Never?
      Yes → do not write a WAV, or delete an already-created WAV
      No  → register the WAV in retained_audio
  → text policy is Never?
      Yes → do not insert dictated text into transcription_history
      No  → insert the history row and optional registered audio path

Startup, hourly cleanup, or retention setting update
  → delete each expired audio file
  → after filesystem success, clear history.audio_path and delete retained_audio in one SQLite transaction
  → if filesystem deletion fails, keep the durable reference for the next cleanup
  → delete text rows expired under the independent text policy
```

Schema migration v3 copies existing non-empty history audio paths into `retained_audio`. Startup also removes WAV files in the application recordings directory that have no registry row. See [`../feat/privacy-retention/0.1.0/prd/erd.md`](../feat/privacy-retention/0.1.0/prd/erd.md) for the policy contract.

## Engine Selection Flow

```
Settings
  → cloud STT active?
      Yes → StreamingSttClient (WebSocket-based, impls SttEngine)
      No  → SherpaOnnxBufferingEngine (buffer-based, impls SttEngine)

All engines implement unified SttEngine:
  send_chunk(pcm_16khz_mono)  — per-chunk, async
  finish() → Result<String>    — called after channel closes

Consumer task (identical for all engines):
  while let Some(chunk) = rx.recv().await {
      engine.send_chunk(chunk).await?;
  }
  let text = engine.finish().await?;

Local Engine (via SherpaOnnxBufferingEngine):
  → send_chunk() pushes Vec<i16> to internal buffer
  → finish() flattens buffer → f32 → SherpaOnnxEngine transcribe
  → Whisper input >28s: balanced low-energy ranges → decode each → join text
  → Zero file I/O

Cloud Streaming Engine (via StreamingSttClient):
  → send_chunk() forwards to internal mpsc → WebSocket
  → finish() drops internal sender, awaits WebSocket final result

Cloud Non-Streaming:
  → send_chunk() accumulates pcm in buffer
  → finish() writes temp WAV → HTTP API → String
```

## State Machine

```
RecordingState

Idle
  │
  │ hold hotkey
  ▼
Recording
  │
  │ release hotkey
  ▼
Processing
  │
  │ transcription complete
  ▼
Injecting
  │
  │ injection complete OR injection failed
  ▼
Idle

Error States:
  RecordingFailed     — mic access denied, device error
  TranscriptionFailed — engine error, invalid audio
  InjectionFailed    — clipboard, activation, focus, or Accessibility target error
```

## IPC Communication

### Frontend → Backend

All calls go through `src/lib/tauri.ts`:

```typescript
invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T>
```

Key commands:
- `start_recording` — begin audio capture
- `stop_recording` — end capture and transcribe
- `get_settings` — retrieve current settings
- `update_settings` — persist settings changes
- `get_history` — retrieve transcription history

### Backend → Frontend

Events emitted through `events/` module:

| Event | Payload | Trigger |
|-------|---------|---------|
| `recording-state-changed` | `RecordingState` | Recording session state transitions only |
| `audio-level` | `number` (0.0-1.0) | Level meter updates |
| `transcription-complete` | `TranscriptionResult` | Successful transcription |
| `transcription-error` | `string` (error message) | Engine failure |
| `retry-state-changed` | `{ entry_id, task_id, status }` | History retry state transitions |
| `retry-complete` | `{ entry_id, task_id, text }` | Successful history retry |
| `retry-error` | `{ entry_id, task_id, error }` | Failed history retry |
| `model-download-progress` | `{ progress: number, model: string }` | Model download |
| `polish-progress` | `string` | Polish operation update |

Frontend subscribes via:

```typescript
listen<T>(event: string, handler: (event: T) => void): Promise<UnlistenFn>
```

## Data Contracts

Language fields use full IETF BCP 47 tags. Use `language-REGION` values such as `en-US`, `zh-CN`, `ja-JP`, and `ko-KR`, not bare language codes like `en` or `zh`.

### TranscriptionRequest

```typescript
interface TranscriptionRequest {
  audio_source: AudioSource;    // File path or in-memory samples
  language?: string;            // BCP 47 language tag (e.g., "zh-CN")
  denoise_mode: "on" | "off" | "auto";
  vad_enabled: boolean;
  cloud_config?: CloudSttConfig; // Cloud provider settings
}

type AudioSource =
  | { File: string }            // Path to WAV file (legacy / cloud non-streaming)
  | { Memory: number[] };       // f32 16kHz mono samples (local STT, zero I/O)
```

### TranscriptionResult

```typescript
interface TranscriptionResult {
  text: string;                 // Final transcribed text
  engine_type: "whisper" | "sensevoice" | "cloud";
  duration_ms: number;          // Processing time
  audio_duration_ms?: number;   // Original audio length
  preprocess_ms?: number;       // Audio preprocessing time
  inference_ms?: number;        // Model inference time
}
```

### CloudSttConfig

```typescript
interface CloudSttConfig {
  provider_type: "volcengine" | "qwen_omni" | "elevenlabs" | "deepgram";
  api_key: string;
  app_id?: string;              // Volcengine requires this
  base_url: string;             // WebSocket or HTTP endpoint
  model: string;                // Provider-specific model name
  language?: string;            // BCP 47 language tag
}
```

### PolishResult

```typescript
interface PolishResult {
  polished_text: string;        // Refined text
  original_text: string;        // Original transcription
  duration_ms: number;          // Processing time
  engine_type: "lfm" | "qwen" | "anthropic" | "openai" | "custom";
}
```

## Audio Pipeline

```
Microphone (system audio capture, device sample rate)
  → Audio Recorder (raw PCM callback)
  → Chunk buffer (0.5s accumulation)
  → i16 → f32 conversion
  → Stereo → mono downmix (if needed)
  → StreamAudioProcessor:
      ├── Denoise: RNNoise at 48kHz (if enabled)
      ├── Resample to 16kHz
      └── VAD: Silero at 16kHz (512-sample windows)
  → VAD-filtered pcm_16khz_mono sent via mpsc channel
  → Spawned consumer task:
      while let Some(chunk) = rx.recv().await {
          engine.send_chunk(chunk).await;
      }
      text = engine.finish().await;
  → Engine internals:
      ├── SherpaOnnxBufferingEngine → buffer Vec<i16> → finish: flatten → transcribe
      │   └── Whisper input >28s → balanced low-energy ranges → decode each → join text
      └── StreamingSttClient → forward to WebSocket → finish: await final result
```

## Multi-Window Model

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Main      │     │    Pill     │     │    Toast    │
│  Window     │     │  (floating) │     │ (transient) │
│ main.tsx    │     │  pill.tsx   │     │  toast.tsx   │
└──────┬──────┘     └──────┬──────┘     └──────┬──────┘
       │                   │                   │
       └───────────────────┼───────────────────┘
                           │
                   settings context
```

- **Main window** — Settings dashboard, history browser
- **Pill** — Floating indicator during recording
- **Toast** — Transient notifications (errors, completion)
