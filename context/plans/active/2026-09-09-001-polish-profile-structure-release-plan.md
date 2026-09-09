---
title: Improve polish profile structure and release v1.2.2
type: fix
status: active
date: 2026-09-09
---

## Overview and problem frame

Make profile selection produce observable changes in dictated text while retaining all distinct content. See [spec](../../feat/polish-profile-structure/0.1.0/prd/erd.md).

## Scope boundaries

Built-in prompts, provider core instructions, bounded repetition-aware length validation, French quality checks and v1.2.2 publication. No frontend business logic, audio pause inference, capability changes or model replacement. Existing unrelated working-tree changes are excluded.

## Implementation units

- [x] Add failing policy regression and actual-inference structure checks.
- [x] Rewrite profiles and align local/cloud core rules; narrowly adjust length baseline.
- [x] Run French model checks and required Rust/frontend/release checks.
- [x] Update canonical docs, versions and changelog.
- [ ] Commit, tag, push and verify signed release publication.

## System-wide impact

Shared Rust paths affect recording, history and workflow transformations. IDs and custom templates remain compatible. Existing saved built-in IDs resolve the new prompts.

## Risks and dependencies

Model quality varies; real inference requires installed model/runtime. Exact repetition detection is a bounded heuristic, not semantic equivalence. Release depends on GitHub Actions signing/notarization. Do not include pre-existing generated artifacts or the earlier release-plan move in this commit.

## Verification evidence

Initial origin master matches local e004d80; latest published version is v1.2.1; v1.2.2 tag is unused.

## Recovery checkpoint

Publication is blocked by model quality, not release permissions. No version was changed, no commit/tag/push was made, and no release was started.

Three actual-inference evaluation rounds failed the target contract:

1. First candidate on installed Qwen3-4B-Q4_K_M: Notes/Agent produce lists, but Clean Dictation misses lists and topic breaks; spoken self-correction remains unresolved.
2. More explicit layout/correction markers on the same Qwen model: Clean Dictation emits inline hyphens rather than separate lines; Notes output can be rejected by the existing 55% guard after removing spoken ordinal markers. Self-correction still fails.
3. Same candidate on installed LFM2-2.6B-Q4_K_M: lists improve, but a dictated security question is answered and an Agent output invents verification requirements. These outputs are unacceptable regardless of layout.

Per AGENTS.md recovery protocol, the unverified built-in prompt and provider-core changes were reverted. The opt-in real inference suite remains, including a correction of an over-specific uncertainty assertion (equivalent uncertain wording is valid). The separately passing adjacent-duplicate-sentence length-baseline change remains for review. The newly added failing list-marker regression remains unimplemented, as required by the rule against deleting failing tests. Do not publish this working tree.

Evidence: [quality investigation](../../feat/polish-profile-structure/0.1.0/prd/quality-investigation.md).

Independent verification: desktop production build, shared typecheck and locale checks passed using cached pnpm 8.15.0; release-contract Node tests passed. The standard desktop build clears the Rust target cache; subsequent Rust inference verification rebuilt it. Full final Rust/clippy verification is pending because the candidate was rejected.

Next decision: identify the target model/runtime and narrow a new implementation strategy before further prompt iterations. Options include model-specific prompt evaluation and stronger output validation. Do not replace the user's model automatically or publish the failed candidate.

## Resumed scope

User approved model-specific instructions and stronger output checks. Start by isolating request placement / model behavior; then implement French answer guards and the failing list-marker case. Keep profile identity and provider selection stable. The recovery checkpoint above describes the previous candidate, not the resumed release state.

## Resumed verification

- Thirteen shared text-transform tests passed, including failing-first list acceptance, French assistant replies, added questions and lost corrections.
- The Qwen reasoning HTTP contract failed before implementation and passed with explicit thinking fields.
- Desktop frontend: 111 tests passed. Release contract suite: 22 tests passed.
- Actual inference on the installed Qwen3 4B showed that reasoning resolves the previously ignored enumeration, topic-break and self-correction instructions. An unbounded full-suite attempt timed out; the shipped llama.cpp source confirms `thinking_budget_tokens`, now capped at 1,536. Full bounded evaluation is running.
- LFM editing-envelope probes preserve dictated questions but still mishandle some self-corrections. Keep that limitation visible; do not describe its model quality as equivalent to Qwen.

## Release candidate verification

- Version fields and Cargo lock entry agree on 1.2.2.
- Final Rust suite: 937 passed, 35 ignored across 29 suites.
- All-feature Clippy with warnings denied passed; Rust formatting passed.
- Final desktop TypeScript/Vite production bundle, shared typecheck and i18n checks passed. The standard desktop build had also passed earlier; the final bundle was rebuilt without repeating its unrelated `cargo clean` step.
- Frontend unit suite: 111 passed. Release contracts: 22 passed.
- Real French inference: 11/11 LFM2 2.6B and 11/11 Qwen3 4B cases passed. No cloud/user audio was used.
- Markdown links and Git whitespace checks passed.
- Original remote master remains e004d80 and v1.2.2 is unused.

The earlier recovery checkpoint is resolved. This candidate is ready for the signed tag workflow. UI E2E is not rerun for this backend-only change; the real inference path and shared service tests cover the changed behavior. Pre-existing local generated artifacts and the earlier release-plan move remain excluded.
