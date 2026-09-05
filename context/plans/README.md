# Execution Plans

Plans are first-class artifacts. They capture intent, approach, and progress for non-trivial work.

## When to Read This

- Read [`../README.md`](../README.md) for document routing and canonical sources
- Read [`../../AGENTS.md`](../../AGENTS.md) for the planning threshold and default iteration strategy
- Read [`../feat/README.md`](../feat/README.md) for feature intent and acceptance criteria
- Read this directory when work needs execution structure, progress tracking, or handoff continuity beyond a tight single iteration

## Plan Types

| Type | Purpose | Lifecycle |
|------|---------|-----------|
| **Fix plans** | Structured bug fixes with root cause analysis | Draft → Active → Completed |
| **Feature plans** | Implementation plans for feature specs | Draft → Active → Completed |
| **Refactor plans** | Code reorganization with safety checks | Draft → Active → Completed |

## Plan Lifecycle

| State | Meaning | Source of Truth |
|------|---------|-----------------|
| **Draft** | Plan exists but is not yet being executed | File frontmatter `status: draft` |
| **Active** | Work is in progress | File lives in `./active/` and frontmatter `status: active` |
| **Completed** | Scope is finished and verification evidence is recorded | File moves to `./completed/` and frontmatter becomes `status: completed` |

A plan is only completed when all implementation units are closed, verification evidence is captured in the plan, and the file is moved from `./active/` to `./completed/`.

## Active Plans

| Plan | Type | Date | Status |
|------|------|------|--------|
| [Original Dictation Target](./active/2026-09-04-001-feat-original-dictation-target-plan.md) | feat | 2026-09-04 | Active |
| [Logging Standardization](./active/2026-04-03-001-fix-logging-standardization-plan.md) | fix | 2026-04-03 | Active |
| [Multi-Shortcut Profiles](./active/2026-04-20-001-feat-multi-shortcut-profiles-plan.md) | feat | 2026-04-20 | Active |

## Completed Plans

| Plan | Type | Date |
|------|------|------|
| [Opt-in Vibe coding context](./completed/2026-09-05-003-feat-vibe-coding-plan.md) | feat | 2026-09-05 |
| [Dictation workspace](./completed/2026-09-05-003-feat-dictation-workspace-plan.md) | feat | 2026-09-05 |
| [Workflow library interface](./completed/2026-09-05-003-feat-workflow-library-ui-plan.md) | feat | 2026-09-05 |
| [Local macOS development installation build](./completed/2026-09-05-002-feat-dev-local-install-plan.md) | feat | 2026-09-05 |
| [Dictation-focused product simplification](./completed/2026-09-05-001-refactor-product-simplification-plan.md) | refactor | 2026-09-05 |
| [Polish Output Safety](./completed/2026-09-04-003-fix-polish-output-safety-plan.md) | fix | 2026-09-04 |
| [Paste Last Transcription](./completed/2026-08-25-001-feat-paste-last-transcription-plan.md) | feat | 2026-08-25 |
| [Dictate Polish Template](./completed/2026-09-04-002-feat-dictate-polish-template-plan.md) | feat | 2026-09-04 |
| [Voice Flow Product Opportunities B1-B5 and P1-P13](./completed/2026-08-24-001-feat-product-opportunities-plan.md) | feat | 2026-08-24 |
| [Cloud Provider Contract Alignment](./completed/2026-08-24-003-fix-provider-contract-alignment-plan.md) | fix | 2026-08-24 |
| [Complete Local Whisper Transcription for Long Recordings](./completed/2026-08-23-002-fix-long-whisper-transcription-plan.md) | fix | 2026-08-23 |
| [macOS Permissions and Local STT Downloads](./completed/2026-08-23-001-fix-macos-permissions-model-downloads-plan.md) | fix | 2026-08-23 |
| [Voice Flow Brand Cleanup](./completed/2026-08-24-002-feat-voice-flow-brand-cleanup-plan.md) | feature | 2026-08-24 |
| [sherpa-onnx STT Engine Refactor](./completed/2026-04-08-001-refactor-sherpa-onnx-stt-engine.md) | refactor | 2026-04-08 |
| [Audio Command Boundary Refactor](./completed/2026-04-13-003-refactor-audio-command-boundaries-plan.md) | refactor | 2026-04-13 |
| [Startup Permission Logging Architecture](./completed/2026-04-14-006-startup-permission-logging-architecture-plan.md) | fix | 2026-04-14 |

## Provider API Reference

Provider API docs have moved to [`reference/providers/`](../reference/README.md):
- [STT Provider APIs](../reference/providers/stt.md) — Speech-to-Text cloud providers
- [Polish Provider APIs](../reference/providers/polish.md) — Text polishing cloud providers

## Completion Procedure

When a plan moves from `active/` to `completed/`:
1. Update frontmatter `status` from `active` to `completed`
2. Add or finalize verification evidence inside the plan
3. Move the file into `./completed/`
4. Update this index and `context/README.md` if the active/completed listings changed

## Plan Format

Every plan MUST include:
1. **Frontmatter**: title, type, status, date
2. **Overview**: What and why
3. **Problem Frame**: Current state vs desired state
4. **Scope Boundaries**: What's in scope, what's out
5. **Implementation Units**: Atomic tasks with files, approach, verification
6. **System-Wide Impact**: What else is affected
7. **Risks & Dependencies**: Known unknowns
8. **Verification Evidence**: Commands run, results observed, or linked proof when completed
