# Dictation-focused product simplification

Status: Completed. Approved by the user on 2026-09-05 through "apply all of that", referring to the [simplification review](../../../../brainstorms/2026-09-05-product-simplification-review.md).

The later [Dictation workspace](../../../dictation-workspace/0.1.0/prd/erd.md) revises the Home and navigation requirements (criteria 6–7): summary statistics, recent history and direct personalization pages return. Other simplification invariants remain in force.

## Outcome

Voice Flow opens on dictation readiness and recovery. Optional writing, media, profile, and integration tools remain available under Advanced. Core behavior uses a single profile model, shared transformation policy, and complete-result delivery.

## Acceptance criteria

1. Local-only processing disables cloud STT and cloud polish atomically. Processing choices never change retention or context consent. Missing microphones produce setup guidance, not a cloud recommendation.
2. Recording, retry, history re-polish, and workflow actions share provider dispatch, timeouts, and intent-specific output acceptance. Unsafe cleanup falls back to original text; translation and intentional rewriting have explicit policies.
3. Stream previews never type. Remove the direct-stream setting, UI, dead insertion helpers, and finalization branches. Legacy settings still load.
4. Remove Undo last insertion from IPC actions, UI, and delivery journal. Keep copy, paste last, cancellation, and history recovery.
5. Persist only workflow_profiles. Legacy maps are input migrations; no live projection or Riff accessors remain. Preserve existing profile IDs, shortcuts, and templates.
6. Home displays backend readiness, current shortcut, last result, and actionable failure state. Remove habit charts, estimated savings, and unused chart dependencies.
7. Templates, workflows, media import/caption export, code context, and diagnostics are advanced options. Basic history and text export remain accessible. Static snippets remain available with dictionary tools.
8. Developer bridge is off by default and starts only when explicitly enabled. Existing backend commands remain available. Background target delivery is labeled experimental until native compatibility evidence exists.
9. Shared template preservation instructions replace repeated prompt text without changing stable IDs. Basic template choices are No Polish and Clean Dictation; existing advanced selections remain visible.
10. Ghost drafts remain deferred and are removed from shipped architecture descriptions. Canonical docs reflect the reduced product and verification limits.

## Verification

Failing-first tests cover privacy state transitions, output policies across callers, legacy migration, removed actions/settings, home state, and advanced navigation. Run Rust tests, Clippy, formatting, desktop tests/build, shared typecheck, i18n and Markdown checks. Verify the native/browser interface where the environment permits; distinguish automated adapter checks from third-party field compatibility.

## Completion evidence

The [completed execution plan](../../../../plans/completed/2026-09-05-001-refactor-product-simplification-plan.md) records the implementation, failing-first checks, full Rust suite, final library regressions, 94 frontend tests, build/type/i18n/lint checks, dependency validation, and native UI checks. Local logs and screenshots were inspected during execution. The review app used isolated settings and was closed afterward.

Accuracy and meaning preservation remain bounded by the selected models and documented guard heuristics. The 34 explicitly ignored live-provider/model/hardware tests, Windows native behavior, bridge activation cycling, and third-party background insertion are not claimed as verified. Background insertion remains experimental; optional writing/media expansion and Ghost features remain deferred.
