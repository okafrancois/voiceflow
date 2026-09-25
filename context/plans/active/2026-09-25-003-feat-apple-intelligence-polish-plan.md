---
title: Apple Intelligence polish
type: feat
status: active
date: 2026-09-25
spec: context/feat/apple-intelligence-polish/0.1.0/prd/erd.md
---

# Apple Intelligence Polish

## Overview

Port the native prototype's Foundation Models polish to the Tauri app as an
"Apple Intelligence" local polish model, so the native prototype can be
removed without losing it.

## Problem Frame

- Current: local polish needs a downloaded GGUF model and the llama runtime.
- Desired: on macOS 26 with Apple Intelligence enabled, polish runs on the
  system model with no download and no runtime.

## Scope Boundaries

In scope: bridge functions, `polish_engine/apple`, manager routing, model
commands, polish status, polish UI and i18n, rename of the Swift package to
`AppleBridge` (it now hosts speech and language model entry points).

Out of scope: command mode, session prewarm, retry on rejected output (the
existing `accept_output` safety falls back to the raw transcript).

## Implementation Units

1. Swift: `Shared.swift` (blocking helper, string ownership,
   `vf_apple_bridge_free`), `LanguageModelBridge.swift` (`vf_apple_llm_status`,
   `vf_apple_llm_generate`, temperature 0.2).
2. Rust bridge wrappers with stub, `prompt.rs` (framing, sentence chunks of at
   most 250 words, marker unwrapping), `engine.rs` (`ApplePolishEngine` over a
   generator seam).
3. `PolishEngineType::Apple`; manager registration; built-in models skip the
   model file and llama runtime checks.
4. Commands: list entry with `built_in`, download and delete rejected, status
   ready without runtime; compatibility and latency entries.
5. UI: built-in label, no download or delete button, Apple Intelligence hint.

## Risks & Dependencies

- Content-filter refusals are detected from the error description because the
  error types changed between the macOS 26 and 27 SDKs.
- No retry on rejected output, unlike the native prototype.

## Verification Evidence

2026-09-25:

- Real model (ignored test, macOS 27.2): a 38-word French dictated question
  came back rewritten in 1.4 s ("…parce qu'il n'est même pas sur la maquette
  et je ne comprends pas pourquoi on l'a ajouté"), not answered.
- `cargo test`: 972 passed. Clippy clean with the bridge and with the stub.
  `cargo fmt -- --check` clean.
- Frontend: `tsc && vite build` OK, i18n OK, vitest 113 passed.

Still open: manual dictation with Apple Intelligence polish in the app.
