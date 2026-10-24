---
title: Visual model selectors
type: feat
status: completed
date: 2026-10-24
---

## Overview and problem frame

Implement the approved compact model selector with aligned gauges and useful filters. Existing selectors use large nested cards and text-only ratings.

## Scope boundaries

Transcription and polish model settings, existing backend catalogue metadata, translations and local Dev installation. No new inference engine, benchmark, model download or permission capability is required.

## Implementation units

- [x] Record the [contract](../../feat/model-catalog/0.1.0/prd/erd.md) and failing behavioral checks.
- [x] Share compact catalogue presentation, filters and active summary across both selectors.
- [x] Preserve native/service routing, downloads, built-in protections and runtime warnings.
- [x] Translate new text and verify frontend, backend and responsive visual behavior.
- [x] Build, verify and install the updated Dev app, then document evidence.

## System-wide impact

Additive STT language metadata; no inference or audio pipeline changes. View filters remain local. Settings changes retain existing IPC contracts. Existing uncommitted MLX integration remains intact.

## Risks and dependencies

Built-in availability differs from download state; service selection must not be confused with the native fallback. Runtime readiness and compatibility warnings must survive the redesign. The approved tighter radii and muted sage catalogue accent are a scoped exception to the older general design-system radius guidance.

## Verification evidence

- Red phase: three new component scenarios failed against the previous selectors; the Rust serialization check failed on absent language metadata.
- `pnpm --filter @voiceflow/desktop test`: 119 passed across 23 files.
- `cargo test`: 990 passed, 41 ignored integration/environment-dependent checks, zero failures.
- `cargo clippy --all-features -- -D warnings` and `cargo fmt -- --check`: passed.
- `pnpm --filter @voiceflow/shared typecheck`, `pnpm check:i18n`, and markdown link validation: passed.
- Local-install frontend TypeScript/Vite build and `pnpm --filter @voiceflow/desktop tauri:build:mac:local-install`: passed; installed bundle validated with `codesign --verify --deep --strict`.
- Native macOS inspection confirmed both selectors, real catalogue data, current MLX service summary, and combined size/installed filtering without changing the active engine. Shortened polish template labels after spotting excessive wrapping.
- The actual React catalogue was also rendered with shipped CSS and French strings at 728px and 320px; controls and columns fit without horizontal overflow. Both light and native dark presentations were inspected.
- Updated `~/Applications/Voice Flow Dev.app`; production application was not changed. The original Dev bundle remains in the task workspace for rollback.

## Local runtime limitation

Recompiling the ad-hoc signed Dev application made macOS report Microphone and Accessibility as not granted. The user explicitly asked not to pursue all permissions, so permission work was stopped and no further audio recording test was performed. Existing settings and selected engines were retained. This is independent of the selector implementation.
