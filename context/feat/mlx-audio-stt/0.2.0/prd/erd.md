---
title: Managed MLX-only local transcription on macOS
version: 0.2.0
status: completed
date: 2026-10-24
---

# Managed MLX-only local transcription on macOS

## Outcome

On Apple Silicon, selecting a local transcription model is sufficient: Voice Flow installs its private runtime, downloads the model, starts inference automatically, and delivers transcripts without a separately installed Python, Terminal launcher, or HTTP server. MLX is the only local STT engine offered on Apple Silicon. Intel Macs, Windows, and Linux retain their compatible native engines.

## Contract

- Distinguish the MLX engine from its models. Offer curated MLX Whisper Turbo, Parakeet v3, and Qwen3-ASR models; do not claim that arbitrary GGUF or ONNX files work with MLX.
- Apple Silicon catalogues, recommendations, default downloads, model resolution, import, retry, preload, and dictation use only MLX. Existing native Apple Silicon selections migrate to MLX Whisper Turbo. A previously enabled `mlx-audio` HTTP connector migrates to the managed local selection; vendor cloud services remain opt-in and unchanged.
- MLX requires Apple Silicon. Intel Macs retain the compatible native model catalogue, defaults, and inference; they must never be offered MLX or invoke an ARM-only binary. Windows and Linux keep their existing behavior.
- The Rust backend owns installation, model completeness, runtime lifecycle, inference serialization, errors, and selection. The frontend renders backend state and forwards user actions only.
- Install a private, versioned Python environment with a pinned dependency lock using a checksum-verified bundled `uv` bootstrap. Do not depend on system Python, Homebrew, shell startup files, or the user's separate MLX Studio installation.
- Install the runtime as part of the existing model-download action, with observable installation state and cancellation. Do not mark a model usable before its required runtime and checkpoint files are complete.
- Inference uses a private child process and a versioned JSON-lines protocol over pipes, not a shared TCP port. Require a valid readiness handshake, bound waits, propagate worker/protocol failures, and stop/reap owned children when unloaded, switched, or shut down. A failed inference must not be retried silently or return success.
- The worker accepts only complete app-managed model directories and 16 kHz mono audio. Offline inference must not download files or execute remote model code. Preserve trailing samples, normalized language, and supported vocabulary prompts.
- Transcription preserves the spoken language instead of translating it. Do not pass internal domain/subdomain identifiers such as `it general` as MLX transcription text: they are application metadata, not vocabulary examples. Preserve explicit user prompts and glossary terms; native-engine context behavior remains compatible.
- Model switches release the previous model; multiple compatible models can be installed, but only the selected model needs to occupy memory.
- Cache identity depends on the selected MLX model, not request language. Preload and concurrent different-language requests reuse one serialized worker. User cancellation stops active MLX inference, including separately spawned decode tasks; accepted file-job cancellation prevents history and delivery.
- Changing the dictation language or deleting an unused checkpoint must not interrupt the selected MLX model. A rejected deletion must leave the cached engine untouched; deleting the cached checkpoint unloads it before removing its files. Native language-dependent cache behavior is preserved.
- Runtime readiness queries stay spawn-free. Download validates and repairs an existing environment; confirmed worker-startup failure invalidates only the readiness marker, while ordinary cancellation preserves a healthy runtime. Installer cancellation uses the existing download-cancel event contract.
- Preserve cached downloads where safe, never remove the separate MLX installation or its cache, never kill unrelated local servers, and never modify production settings merely to test the implementation.

## Acceptance evidence

Failing-first checks cover platform-specific catalogue/defaults, legacy selection migration, model dispatch/completeness, wrong-server rejection for the legacy connector, worker readiness and malformed responses, process exit/timeout cleanup, complete audio transfer, model switching, installation failure/cancellation, and UI installation/model status. Run real local French inference through the managed engine, with a text-only llama-compatible HTTP fixture occupying port 8000, plus the relevant full Rust/frontend/distribution checks.

## Scope

The explicit [Apple Silicon model policy](../../../apple-silicon-model-policy/0.1.0/prd/erd.md) additionally permits built-in Apple Speech alongside managed MLX; legacy downloadable native engines remain excluded on that hardware.

Local transcription and its app-private runtime distribution. Preserve vendor cloud transcription and text-polishing engines. MLX text generation/TTS, automatic model quality rankings, release publication, Git operations, production app replacement, and capability changes are outside this change.

## Runtime requirements

The managed runtime pins Python 3.12.15, MLX Audio 0.6.1, and a fully hashed dependency lock. Its MLX 0.32.4 wheels require macOS 14 or newer. Hardware availability remains Apple Silicon-only independently of OS version: an older Apple Silicon Mac receives upgrade guidance before installation touches directories or the network, not a native-engine fallback. Intel Mac, Windows, and Linux availability remains unchanged.

## Verified outcome

All three curated models transcribed the real French fixture through the managed runtime, including preload and concurrent different-language worker reuse, while a text-only HTTP fixture occupied port 8000 and received zero requests. Final Rust verification passed 1,043 tests with 42 ignored, plus five custom macOS checks; frontend verification passed 132 tests. Clippy, formatting, distribution/worker checks, Intel cross-compilation, and the signed local development bundle passed. Independent review findings were resolved and the final focused review found no actionable issue.

The development bundle was not installed or launched, and production settings were not modified. Interactive native UI/audio delivery, real-model user cancellation, Intel execution, and Windows/Linux execution remain unverified. See the [completed execution plan](../../../../plans/completed/2026-10-24-005-feat-managed-mlx-plan.md) for commands, evidence, and limits. This completion covers managed STT only, not MLX text polish.
