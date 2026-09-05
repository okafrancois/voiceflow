---
title: Dictation-focused product simplification
type: refactor
status: completed
date: 2026-09-05
---

## Overview

Apply the approved [review](../../brainstorms/2026-09-05-product-simplification-review.md) under the [specification](../../feat/product-simplification/0.1.0/prd/erd.md).

## Problem frame

Core privacy, polish acceptance, and recovery guarantees differ across paths. Obsolete streaming and profile representations remain active configuration. Optional tools dominate the main experience.

## Scope boundaries

Implement all approved reductions. Preserve saved user data and useful advanced capabilities. Do not expand deferred features, change provider protocols, release, or perform Git operations.

## Implementation units

1. Complete: replace broad presets with explicit processing choices and fix local-only privacy.
2. Complete: shared transformation service and prompt policy; remove direct-stream machinery and unsafe undo.
3. Complete: canonical workflow profile migration and caller conversion.
4. Complete: readiness/recovery home, advanced navigation, optional bridge, template and media placement.
5. Complete: full automated verification, UI checks, documentation closure.

## System-wide impact

Rust services, settings migrations, IPC, frontend navigation and settings, locale catalogs, dependency metadata, tests, and product/architecture documentation.

## Risks and dependencies

Preserve hotkey registration rollback and old config loading. Native Accessibility tests depend on permissions and third-party apps. Model/provider accuracy checks require installed models or credentials; deterministic policy and protocol checks do not establish native accuracy.

## Verification evidence

All checks ran on macOS arm64 on 2026-09-05.

| Check | Result |
| --- | --- |
| Full `cargo test` | 905 passed, 0 failed, 34 ignored across 29 test summaries. Ignored cases explicitly require live providers, installed models, hardware, or media fixtures. |
| Final `cargo test --lib` after readiness tightening and the recording-provider regression | 655 passed, 0 failed. |
| `cargo clippy --all-features -- -D warnings` | Passed, including the development mock exporter after retiring its dashboard-stats output. |
| `cargo fmt -- --check` | Passed. |
| Desktop Vitest | 94 passed across 17 files. Final diagnostics text cleanup also passed its 4 component tests. |
| Desktop TypeScript and production Vite build | Passed. Vite still reports large chunks; no new chunk-size claim is made. |
| Shared TypeScript typecheck | Passed. |
| `node scripts/check-i18n.mjs` | Passed for desktop’s 10 locales and website parity. |
| Workspace lockfile validation | All importer ranges match manifests after root overrides; all 734 package entries are reachable. Removed Recharts and 36 orphaned package entries. |
| Markdown links | Passed, including closure and index updates. |
| Isolated native Tauri app | Built and opened with identifier `com.voiceflow.simplification-check`; separate seed settings prevented legacy-data migration. Home, Advanced, basic History, media workbench, and Diagnostics rendered. |
| Native microphone prerequisite | Home showed setup guidance. Running diagnostics without microphone permission returned Not ready and no processing recommendation. No permission was granted. |
| Native bridge default | Advanced showed the bridge off. `lsof` found no TCP listener for the review app’s process. |

Failing-first evidence was captured for processing-choice privacy, shared output policies, canonical profile serialization/migration, and readiness/recovery home behavior. Provider-response tests now exercise unsafe language changes through recording, history polish, and workflow polish; explicit translation remains accepted. Retry uses the recording transformation path. Existing provider streaming and final-delivery tests remain intact. Tests for deliberately removed streaming/undo/statistics behavior were retired with those features; recovery and backend-error assertions remain.

The new Advanced test confirms optional tools remain addressable and the bridge control sends a settings command without deciding its own state. Hotkey tests confirm basic choices and preservation of an existing advanced template. Basic history tests confirm it does not initialize file jobs.

`pnpm` is unavailable in this environment. Installed Node entry points ran Vitest, TypeScript, Vite, shared typechecking, and repository validation scripts. The desktop wrapper’s unrelated Cargo clean and capability-cleanup hooks were bypassed; no capability files were edited. The native app used the installed Tauri CLI with already-built frontend assets, an isolated identifier, no signing, and no release publication.

The `agent-browser` CLI was unavailable, so native CUA controls supplied visual and accessibility-tree verification. The review app was closed after inspection. Native logs show the expected missing Input Monitoring permission; the new home prerequisite includes microphone hardware, microphone permission, keyboard permissions, and configured/downloaded STT resources.

## Verification limits

These changes do not establish model accuracy, semantic equivalence of transformed text, Windows native compatibility, or background insertion into representative third-party fields. The output guards remain bounded language/length/question heuristics. Native bridge enable/disable cycling was not exercised; its default-off native state, configuration default, UI dispatch, parser/authentication, and existing CLI transport contracts were checked. Background insertion stays experimental and Ghost proposals remain deferred.

Local evidence logs were captured under `/tmp/voiceflow-*.log` during execution; the results above are the durable record.

## Documentation and workflow extraction

The architecture now distinguishes provider adapters from the shared transformation policy. Profile maps are input migrations only; no chart-derived usage model remains. Updated feature contracts explain Advanced placement, independent processing/retention choices, opt-in TCP automation, retired undo, and original-text fallback. Historical dashboard and Ghost documents are explicitly superseded or deferred.

Use a current failing behavior as the next change boundary. Do not restore a feature merely to satisfy a test of deliberately retired behavior, and do not treat historical domain grades or provider mocks as native compatibility evidence.
