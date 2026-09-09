---
title: Improve polish profile structure and release v1.2.2
type: fix
status: completed
date: 2026-09-09
---

## Overview and problem frame

Built-in profiles did little beyond punctuation cleanup. Explicit enumerations stayed as prose, local models mishandled self-corrections, and the length guard could discard valid lists. The delivered change gives profiles explicit transformations and combines model-aware requests with bounded backend preparation and output checks. See the [specification](../../feat/polish-profile-structure/0.1.0/prd/erd.md) and [quality evidence](../../feat/polish-profile-structure/0.1.0/prd/quality-investigation.md).

## Scope boundaries

Built-in/default prompts, local request behavior, shared text preparation and output validation, French inference checks and macOS release publication. No frontend business logic, audio-pause inference, automatic model replacement or cloud transmission of local transcripts. Pre-existing local artifacts and the earlier release-plan move were excluded.

## Implementation units

- [x] Reproduce the missing lists and self-correction failures using real local inference.
- [x] Specify and rewrite the six profiles and unify example-free family defaults.
- [x] Enable bounded Qwen3 4B reasoning and add the LFM editing envelope.
- [x] Resolve clear weekday/numeric corrections before inference and strengthen output checks.
- [x] Verify Rust, frontend, release contracts and both installed local models.
- [x] Publish v1.2.2 and validate the downloaded package and live updater endpoint.

## System-wide impact

Recording, history and workflow transformations use the shared Rust service. Raw text is retained for history/fallback. Profile IDs and custom template storage remain compatible. Qwen3 4B uses a bounded reasoning phase; other Qwen models retain their previous non-thinking policy.

## Risks and dependencies

Model quality remains variable. Correction, length and question guards are bounded heuristics, not semantic equivalence. Only explicit weekday/numeric correction patterns are resolved deterministically. Qwen3 4B may take longer. External local servers may ignore the llama.cpp budget extension; the request deadline still applies. Other model families and cloud models were not evaluated with real inference in this task.

## Verification evidence

- Rust: 937 passed, 35 ignored across 29 suites.
- Clippy with all features and denied warnings: passed. Rust formatting: passed.
- Frontend: 111 tests passed; production TypeScript/Vite build, shared typecheck and i18n passed.
- Release contracts: 22 passed. Markdown links and whitespace checks passed.
- Real French inference: 11/11 LFM2 2.6B cases and 11/11 Qwen3 4B cases passed.
- Backend preparation tests cover correction chains, punctuation, Unicode, retained unrelated values and ambiguous/literal syntax. HTTP tests cover explicit thinking fields, the LFM envelope and hidden reasoning deltas.
- UI E2E was not rerun for this backend-only change; real inference and shared service tests cover the changed behavior.

## Publication evidence

Commit `dfa62e9751bad62bc29e19f9f2943852ff6cd85b` and annotated tag `v1.2.2` were pushed atomically. [Workflow 34345614806](https://github.com/okafrancois/voiceflow/actions/runs/34345614806) succeeded: macOS build and bundled-runtime verification in 18m47s, publication in 43s.

[Voice Flow v1.2.2](https://github.com/okafrancois/voiceflow/releases/tag/v1.2.2) was published on 2026-09-09 at 11:46:49 UTC. The published DMG, app archive, signature and manifests are present. Both macOS updater entries point to the published archive and match its signature sidecar. The live `releases/latest/download/latest.updater.json` endpoint serves version 1.2.2.

The downloaded application passed `codesign --verify --deep --strict` and `xcrun stapler validate`. Its bundle version is 1.2.2 and `lipo -archs` reports `x86_64 arm64`. The package was inspected without installing or launching it.

## Documentation extraction

Canonical provider and safety documentation now explains model-specific reasoning, transcript preparation and the bounded guards. Default family prompts no longer duplicate stale examples. The inference suite reports real missing prerequisites as failures and accepts equivalent French phrasing rather than enforcing one exact paraphrase. The initial failed prompt-only investigation remains summarized in the quality evidence; it is superseded by the verified implementation.
