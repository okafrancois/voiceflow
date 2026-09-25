# Apple Intelligence Polish Specification

## Version

- Feature: `apple-intelligence-polish`
- User-visible name: Apple Intelligence (on-device)
- Version: `0.1.0`
- Status: Active

## Problem Statement

The native macOS prototype polished dictations with Apple's on-device
Foundation Models (103 of its 122 dictations). The Tauri app only offers
downloadable llama-server models and cloud providers. Removing the native
prototype would lose this zero-download, on-device polish option.

## Goal

On macOS 26 or later with Apple Intelligence enabled, the user can select
"Apple Intelligence" as the local polish model. Polish then runs through
Foundation Models with no download, no local server, and no API key.

## Non-Goals

1. No command mode (spoken edit of a selection) in this version.
2. No session prewarm during recording.
3. No change to existing local or cloud polish engines, prompts, or templates.

## First-Principles Model

1. Apple Intelligence is a polish model like the others: selection, history,
   output safety (`accept_output`), and fallback to the raw transcript reuse
   the existing paths.
2. It ships with the OS: it has no file, no download, no llama runtime, and
   cannot be deleted. Availability is a backend fact exposed to the UI.
3. The Foundation Models context window is small, so long transcripts are
   polished in chunks of whole sentences (at most 250 words each) and joined
   with their original separators.
4. The system model answers dictated questions unless the transcript is framed
   as data: the user turn contains only the transcript between
   `<<<TRANSCRIPT` and `TRANSCRIPT>>>`, and the instruction to rewrite it lives
   in the system prompt. Markers copied back by the model are stripped.
5. Swift-only APIs stay behind the existing Swift bridge's C ABI.

## Architecture

- Swift bridge (`swift/AppleBridge`, formerly `AppleSpeech`): adds
  `vf_apple_llm_status()` and `vf_apple_llm_generate(instructions, prompt)`.
- Rust `polish_engine/apple`: bridge wrappers (stub without the bridge),
  prompt framing, sentence chunking, and `ApplePolishEngine` implementing
  `PolishEngine` over a text generator seam.
- `PolishEngineType::Apple` (`"apple"`), model id `apple-intelligence`.
- `UnifiedPolishManager`: registers the engine; built-in models skip the model
  file and llama runtime checks.
- Commands: `get_polish_models` lists the model when the OS supports Foundation
  Models, with `built_in: true` and `downloaded` meaning available; download
  and delete are rejected for built-in models; polish status reports the
  runtime ready for built-in models.
- UI: built-in polish models show a built-in label instead of size and no
  download or delete button; when unavailable, a hint points to Apple
  Intelligence in System Settings.

## Data Contract

- `PolishEngineType::Apple`, serialized `"apple"`.
- Polish model JSON gains `built_in: bool`.
- History rows record `polish_engine = "apple"`.

C ABI:

| Function | Result |
|----------|--------|
| `vf_apple_llm_status()` | 0 available, 1 unavailable (not enabled, not ready, device not eligible), 3 OS unsupported |
| `vf_apple_llm_generate(instructions, prompt)` | JSON `{"text": …}` or `{"error": …, "kind": "refused" \| "unavailable" \| "failed"}` |

## Acceptance Criteria

1. `get_engine_by_model_id("apple-intelligence")` returns the Apple engine.
2. The system prompt contains the core polish rules, the language rule, the
   user rules, and the transcript-framing rule; the user turn contains only the
   framed transcript.
3. Transcripts above 250 words are polished in sentence chunks, and the joined
   output keeps the original separators; shorter ones use one request.
4. Markers echoed by the model are removed from the output.
5. A refused or failed generation fails the polish, so the caller keeps the raw
   transcript.
6. Built-in polish models cannot be downloaded or deleted, and their status is
   ready without a local runtime.
7. The model is absent from the list before macOS 26 or without the bridge.

## Verification

```bash
cd apps/desktop/src-tauri && cargo test && cargo clippy --all-features -- -D warnings && cargo fmt -- --check
pnpm --filter @voiceflow/desktop exec tsc --noEmit && pnpm check:i18n
```

Manual: with Apple Intelligence enabled, select Apple Intelligence as polish
model, dictate, and check the history row `polish_engine = "apple"`.
