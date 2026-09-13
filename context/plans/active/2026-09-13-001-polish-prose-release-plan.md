---
title: Restore conservative polish prompts and release v1.2.3
type: fix
status: active
date: 2026-09-13
---

## Overview and problem frame

Version 1.2.2 overproduces lists because shared and profile instructions repeatedly demand bullets. Inference checks lacked negative formatting cases. After three failed prompt candidates, the user approved restoring the v1.2.1 prompts, accepting their limited automatic structure. Contract: [profile specification](../../feat/polish-profile-structure/0.1.0/prd/erd.md).

## Scope boundaries

Built-in prompts, inference regression checks, documentation and signed macOS release. Preserve custom profiles, model selection and unrelated local changes.

## Implementation units

- [x] Add negative formatting cases and reproduce the failure on the installed LFM model.
- [x] Restore the six built-in and shared provider prompts exactly from v1.2.1.
- [x] Verify the delivered-prose rollback gate on LFM and Qwen.
- [ ] Run backend and release checks, bump version and publish the signed release.
- [ ] Verify the published artifacts and updater; record evidence.

## System-wide impact

Shared backend prompts affect dictation and agent workflows, including cloud calls. No IPC or settings schema change.

## Risks and dependencies

Model outputs vary. The stronger enumeration checks remain a deferred target after the user authorized rollback. Cloud providers are not exercised with private transcripts. Publication depends on signing and GitHub Actions.

## Verification evidence

Baseline real LFM inference fails the new regression checks: all six profiles turn the ordinary narrative into bullets. Command: `VOICEFLOW_QUALITY_BASE_URL=http://127.0.0.1:18083/v1 VOICEFLOW_QUALITY_MODEL=lfm2-2.6b cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib french_profile_quality -- --ignored --nocapture`.

Three prompt candidates were evaluated without weakening checks:

1. Prose-first conditional layout removes unwanted lists, but also loses required enumerations, paragraph breaks and explicit agent constraints.
2. Shorter conditional layout restores enumerations and keeps ordinary prose, but fails the two topic-change paragraph cases and the explicitly announced agent constraint list.
3. Moving format instructions after the profile reintroduces unwanted bullets in topic changes and concise prose, while still failing required paragraph breaks and agent constraints.

Logs: `/tmp/voiceflow-prose-before.log`, `/tmp/voiceflow-prose-after-lfm.log`, `/tmp/voiceflow-prose-after2-lfm.log`, `/tmp/voiceflow-prose-after3-lfm.log`. Qwen candidate 2 evaluation was started but is not a completed verification of a release candidate.

Recovery protocol triggered after three unsuccessful candidates. Production prompts restored exactly to HEAD; regression tests and specification retained. No version bump, commit, push, tag or release performed. No other local changes reverted.

Next decision: narrow the immediate hotfix to restoring a previously shipped conservative prompt, or authorize a broader layout decision outside the generative prompt. Neither option has been implemented or verified. Do not publish the current tests-only working tree as a fix.

## Authorized recovery scope

The user approved restoring the pre-1.2.2 prompts and publishing a corrective release, accepting reduced automatic structure. Restore the six built-in prompts and conservative shared provider instructions from v1.2.1. Retain the 1.2.2 correction and output safety code. Keep the stricter future structure tests, but gate this rollback on absence of spurious lists and content preservation; automatic enumeration/paragraph improvements are deferred by explicit user choice. Verify actual local inference before publication.

The rollback gate checks the delivered output (including the existing safety fallback) for three ordinary-input cases across the five non-notes profiles. Structured Notes is excluded from this release gate because the approved historical prompt explicitly requests lists; its narrative output can contain an inline bullet. The retained full quality suite checks all six profiles, raw model output and the deferred stronger transformation requirements. A dropped question mark in a Qwen professional-message diagnostic was rejected by the existing safety guard. No model-quality pass is inferred from that fallback.

Rollback verification in progress: exact source comparison confirms all six built-in definitions and both provider core prompts match v1.2.1. Rust: 937 passed, 36 ignored; Clippy all features with denied warnings passed; frontend: 111 passed; production frontend build, shared typecheck and i18n passed. LFM final delivered-prose gate: 15/15 passed without safety fallback. The first Qwen diagnostic timed out during concurrent frontend testing and two loaded models; the final gate is being run with only Qwen active.

Final rollback gate: 15/15 cases passed on LFM2 2.6B (19.38s) and 15/15 on Qwen3 4B (218.11s), with no safety fallback in either final run. Logs: `/tmp/voiceflow-rollback-final-lfm.log` and `/tmp/voiceflow-rollback-final-qwen.log`. All 22 release-contract tests, markdown links and formatting checks passed. UI E2E is not rerun for this prompt-only rollback; backend tests and real inference cover the changed behavior.
