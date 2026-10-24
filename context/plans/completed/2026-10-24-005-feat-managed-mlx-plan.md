---
title: Managed MLX-only macOS transcription
type: feat
status: completed
date: 2026-10-24
---

## Overview

Implement [managed MLX-only local transcription](../../feat/mlx-audio-stt/0.2.0/prd/erd.md). The user explicitly requests MLX only on Apple Silicon, compatible native engines on Intel Macs and Windows, and several selectable MLX models.

## Problem frame

The current MLX HTTP connector assumes an independently running server at port 8000. The app's own text-only llama-server occupies that port and passes the overly permissive model-list check, then returns HTTP 501 for audio. MLX has no managed runtime, model catalogue, or native import/retry integration.

## Scope boundaries

Add app-managed MLX STT on Apple Silicon and migrate its local selections. Preserve cloud STT, text polish, Intel Mac/Windows/Linux engines, separate MLX Studio files, production application/settings, permissions, and Git state. Do not publish or replace the production app.

## Implementation units

- [x] Verify upstream model APIs and distribution; specify platform behavior.
- [x] Add failing platform, migration, protocol, installation, and catalogue checks.
- [x] Implement private runtime installation and persistent inference worker.
- [x] Wire MLX into existing local model downloads, selection, import, retry, and preload.
- [x] Render runtime installation/model state and update all affected translations.
- [x] Verify real inference, lifecycle, Rust/frontend checks, and distribution resources.

## System-wide impact

Reuse the unified STT manager and buffering pipeline, model downloader, typed IPC boundary, and existing catalogue. Apple Silicon model resolution must not retain an alternative-engine fallback; Intel behavior remains compatible. Backend-only callers must obtain the same behavior as the UI.

## Risks and dependencies

MLX supports Apple Silicon, not Intel Mac GPU inference. First installation needs network and disk space. Dependency and Python downloads must be pinned/verified and cancellation-safe. Native Python dependencies require actual installed-runtime verification, not just mock tests. Do not confuse an available checkpoint with a working runtime. Curated model capabilities differ, and indicative rankings must not invent benchmarks.

The pinned MLX 0.32.4 runtime requires macOS 14+. Keep hardware policy independent of OS detection: older Apple Silicon receives upgrade guidance rather than native alternatives. MLX text polish remains outside this STT scope.

## Verification evidence

Initial diagnosis: production v1.5.0 logs contain `MLX Audio HTTP 501 Not Implemented` with `The current model does not support audio input.` `lsof` identifies app-owned llama-server on 127.0.0.1:8000, and `/v1/models` reports Qwen/Gemma with `owned_by=llamacpp`. No MLX server process runs.

Upstream verification on 2026-10-24: PyPI MLX Audio 0.6.1; upstream model repository manifests for Whisper Turbo (revision `624c19c9af5603fa73b83bce14d4aeea96156d18`), Parakeet v3 (`ed2b7e8c15f9aaa0b5772e2efb986255eaef7e15`), and Qwen3-ASR 0.6B 8-bit (`89e96d92ba34aca20b3e29fb10cc284097d1219f`). Official uv 0.13.5 macOS ARM archive SHA-256: `743f85035a1996ecb350659e01cd9faea9709829be09b1b4b03c73eaf3c9cd73`.

Real isolated installation and inference passed for all three curated checkpoints using `~/Applications/MLX-Audio/samples/kokoro-fr.wav`. A text-only HTTP fixture occupied 127.0.0.1:8000 throughout the final run and received zero requests; transcripts contained the expected French voice/Mac phrase. Runtime, models, and temporary audio remained under `/tmp/voiceflow-managed-mlx-validation`; the separate MLX installation was read-only. Log: `/tmp/voiceflow-managed-mlx-real-final.log`.

Independent review identified same-model language cache invalidation, user-facing cancellation propagation, runtime cancellation event normalization, and repair of broken installed dependencies. These are fixed with regression coverage. A second review identified overly broad invalidation on language-setting changes and deletion of unused checkpoints; three failing-first checks reproduced them, then all 22 manager tests passed after selective invalidation was implemented. The third read-only review closed both objections with no actionable finding.

Final verification on Apple Silicon, 2026-10-24:

| Verification | Result |
|--------------|--------|
| Full `cargo test` | 1,043 passed, 42 existing/opt-in ignored; five additional custom macOS checks passed |
| `cargo clippy --all-features -- -D warnings` | Passed |
| `cargo fmt -- --check` | Passed |
| Desktop frontend suite | 132 passed across 23 files |
| Desktop build, shared typecheck, `pnpm check:i18n` | Passed; final development bundle rebuilt the frontend with TypeScript/Vite |
| Distribution/bootstrap/signing Node checks | 39 passed across five relevant test files |
| Python worker protocol tests | Six passed |
| Intel macOS `cargo check --target x86_64-apple-darwin --lib` | Passed using the rustup toolchain containing the Intel target |
| Real managed inference | Whisper Turbo, Parakeet v3, and Qwen3-ASR transcribed the French fixture; preload and concurrent French/auto-language requests reused each model's worker |
| HTTP independence | Text-only fixture on 127.0.0.1:8000 received zero requests |
| Local development bundle | Built, ad hoc signature/deep strict verification passed; bundled ARM `uv` signature and `uv --version` smoke checks passed |

The last real-inference run is recorded in `/tmp/voiceflow-managed-mlx-real-verified.log`; the initial isolated install and all-model run is also recorded in `/tmp/voiceflow-managed-mlx-real-final.log`. Final logs are `/tmp/voiceflow-managed-mlx-rust-final.log`, `/tmp/voiceflow-managed-mlx-clippy-final.log`, `/tmp/voiceflow-managed-mlx-fmt-final.log`, `/tmp/voiceflow-managed-mlx-intel-final.log`, `/tmp/voiceflow-managed-mlx-node-final.log`, `/tmp/voiceflow-managed-mlx-python-final.log`, `/tmp/voiceflow-managed-mlx-dev-bundle-final.log`, and `/tmp/voiceflow-managed-mlx-bundle-smoke-final.log`. Logs and isolated downloads are local validation artifacts, not shipped resources.

Build command from the repository root: `npm --prefix apps/desktop run tauri:build:mac:local-install`. The verified bundle is `apps/desktop/src-tauri/target/debug/bundle/macos/Voice Flow Dev.app`. It remains uninstalled and unopened; production application, settings, permissions, and the separate MLX installation were not changed. No Git operation or release publication was performed.

Verification limits: no interactive native UI/microphone/paste run, no real-model user-cancellation exercise, no Intel runtime execution, and no Windows/Linux build or runtime execution. Cancellation boundaries use controlled futures and owned-worker subprocess fixtures; real model inference covers the manager and Python worker. Existing optional native inference checks remain ignored on this Apple Silicon host. The dependency `block` 0.1.6 still reports an unrelated future-Rust compatibility warning. MLX text polish remains an explicitly separate, unimplemented follow-up.

The documented desktop build invokes `cargo clean`; an accidental parallel invocation invalidated one Rust build. This was an orchestration error, not a source failure. The frontend build then passed and all subsequent Rust validation ran without that cleanup race. Follow the existing sequential workflow in `context/guides/testing.md`.
