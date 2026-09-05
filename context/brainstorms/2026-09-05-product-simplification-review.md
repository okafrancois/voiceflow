# Voice Flow simplification review

Date: 2026-09-05. This document preserves the findings at review time. The user subsequently approved all reductions with “apply all of that”. See the [implementation specification](../feat/product-simplification/0.1.0/prd/erd.md) for the current contract and verification. Statements below about unchanged code and open gaps describe the original review, not the subsequent implementation.

## Product test

The product's job is to turn speech into accurate text in the intended field, with little setup and reliable recovery. The [onboarding guide](../guides/onboarding.md) describes a local-first voice keyboard. The repository priority is accuracy, then stability, then experience, then speed.

Judge each feature by whether it improves transcription, preserves meaning, delivers to the right place, or recovers lost work. Usage volume, configuration breadth, and the number of integrations do not establish those outcomes.

The current implementation has grown beyond that job. Several additions also have different definitions of success. A reply generator may invent text; a dictation cleaner must preserve it. A subtitle exporter needs useful timing; a text exporter does not. Treating all of these as small dictation options hides their separate verification costs.

## Findings, in priority order

### 1. The Private preset does not enforce a private pipeline

In [apply_setup_preset](../../apps/desktop/src-tauri/src/commands/platform_quality.rs), the preset changes `cloud_stt_enabled` but never changes `cloud_polish_enabled`. Starting with cloud polish enabled, choosing Private leaves it enabled. A recording that uses polish can still send text to the cloud.

The broader preset model rests on weak assumptions. [build_diagnostic_report](../../apps/desktop/src-tauri/src/services/platform_quality.rs) recommends Maximum Accuracy when the microphone is unavailable. A cloud provider cannot repair microphone access. The same module bundles longer retention and cloud STT under Maximum Accuracy without measuring their effect on transcription accuracy. Its hardware recommendation uses memory and CPU thresholds; the latency sample does not influence that recommendation.

Proposed simplification: replace the three broad presets with explicit local/cloud processing choices and independent retention controls. A local-only choice must cover both STT and polish. Keep microphone readiness as a prerequisite. Do not label cloud processing or longer retention as an accuracy guarantee.

Proof needed: begin with both cloud stages enabled, apply local-only, and verify neither stage makes a cloud request. An unavailable microphone should produce a setup instruction, independent of provider choice.

### 2. Polish safety is duplicated and incomplete across entry points

Recording calls `accept_polish_result` in [audio/polish.rs](../../apps/desktop/src-tauri/src/commands/audio/polish.rs). It checks language changes, excessive shortening, and question answering. History's `polish_workbench_text` in [history/commands.rs](../../apps/desktop/src-tauri/src/history/commands.rs) and workflows' `polish_text` in [product_workflows.rs](../../apps/desktop/src-tauri/src/commands/product_workflows.rs) only reject empty results after inference. History re-polish persists that output. Workflow actions put it in a preview, which limits immediate damage but does not make their acceptance policy equivalent.

Proposed simplification: one backend transformation service owns provider selection, timeout handling, and output acceptance. Callers pass the operation's intent. Cleanup must preserve language and substance; translation, shortening, and reply generation need different acceptance rules. Keep these differences explicit instead of making exceptions depend on template IDs such as `concise`.

The present 55%/30% length thresholds and French/English word heuristics are useful regression guards, not proof of meaning preservation. Keep the fallback and original text. Test representative names, numbers, negation, questions, and multilingual inputs before expanding automatic rewriting.

Proof needed: the same unsafe cleanup result must be rejected through recording, retry, history re-polish, and quick re-polish. Explicit translation must still work.

### 3. Delete the direct-stream typing feature and its dead machinery

[PerformanceSection.tsx](../../apps/desktop/src/components/Home/model/PerformanceSection.tsx) still exposes and persists `polish_stream_direct_typing_enabled`. [shared.rs](../../apps/desktop/src-tauri/src/commands/audio/shared.rs) calls `should_type_direct_stream_delta(false, ...)`, so the control cannot enable typing. This is a visible no-op, not an advanced performance option.

Delete the switch, update handler, active setting/type field, translation keys, delta insertion helpers, typed-character mutex, insertion atomic, and `direct_stream_inserted` propagation. Trace and remove the obsolete `DirectStreamInserted` finalization branches and wrapper argument as their callers are migrated. Old serialized input may be ignored during loading; it should not remain an operational setting.

Keep provider streaming and pill preview. They provide feedback without inserting unvalidated model output. Final delivery can then follow one rule: accept the complete result, then deliver it once.

Proof needed: legacy settings still load, streamed previews never call the injector, and accepted/fallback text is delivered exactly once for every output action.

### 4. Remove the current Undo last insertion action

In [run_quick_control](../../apps/desktop/src-tauri/src/commands/product_workflows.rs), Undo activates the recorded application and sends its undo shortcut. The [delivery journal](../../apps/desktop/src-tauri/src/services/product_workflows.rs) remembers text and an `undone` flag, but does not verify the focused document, current field contents, or intervening edits.

If the user edits after dictation, the shortcut can undo that later edit. Returning the journal's text does not prove that text was removed. Existing tests verify one-shot journal behavior and keyboard failure handling, not the application's undo stack.

Delete this action until a verified target-specific reversal exists. Keep copy raw/final, paste last, history, and cancellation. They address recovery with clearer contracts. Do not build a general editor transaction system merely to retain this button.

### 5. Delete the estimated time-saved claim; reconsider the habit dashboard

[Dashboard.tsx](../../apps/desktop/src/components/Home/Dashboard.tsx) assumes 30 output units per minute for typing and subtracts audio duration. It excludes transcription, polish, correction, and delivery time, and does not measure the user's typing speed. Output units also approximate words or characters depending on the text.

Delete this metric. More broadly, the [dashboard specification](../feat/home-dashboard/0.1.0/prd/erd.md) assumes users need streaks and usage rhythm on the home screen. That assumption is not supported by the implementation or the reviewed verification evidence.

My recommendation is to replace the habit dashboard with readiness, the current shortcut, the last result, and actionable failures. Keep detailed latency and failure metrics under diagnostics. `Dashboard.tsx` is the only `recharts` importer found under desktop source, so removing its charts would also make that dependency a removal candidate. Check the full dependency graph before removing it.

### 6. Keep legacy profile migration, delete the ongoing second model

The recent [Dictate simplification](../plans/completed/2026-09-04-002-feat-dictate-polish-template-plan.md) correctly replaced the built-in Dictate/Riff split with one shortcut and an optional template. But settings still retain the fixed legacy map alongside `workflow_profiles`. [project_legacy_profiles](../../apps/desktop/src-tauri/src/services/product_workflows.rs) projects changes back into that old map, and [settings updates](../../apps/desktop/src-tauri/src/commands/settings/mod.rs) manage both representations and their rollback.

Migrate old input once, then use one canonical profile list. Move remaining callers to it before deleting the live projection and Riff accessors. Preserve existing custom profiles, hotkeys, and template assignments. Deleting migration itself would trade implementation simplicity for broken upgrades.

## Feature decisions

These are product recommendations, not conclusions about adoption. No user-usage dataset or customer interviews were available in this review.

| Feature | Recommendation | Reason |
| --- | --- | --- |
| Dictation, cancellation, local STT, explicit cloud STT | Keep and prioritize | These implement the primary job. Do not remove provider adapters solely to reduce their count. |
| Correct-target delivery | Keep reliable delivery; keep background mode experimental pending native evidence | Correct placement is central. The [active plan](../plans/active/2026-09-04-001-feat-original-dictation-target-plan.md) still lacks representative third-party field checks. |
| Raw text, paste last, history recovery, retention | Keep | Users need to recover a failed insertion and control stored material. |
| Model downloads, permissions, readiness diagnostics | Keep | These determine whether dictation can work at all. |
| Dictionary and bounded correction learning | Keep narrowly scoped and inspectable | These can improve recurring transcription errors. Do not turn corrections into unrestricted habit profiling. |
| Polish templates | Default to No Polish or Clean Dictation; put other templates in advanced settings | Six built-ins repeat preservation instructions. Share one policy prompt and keep only the formatting differences in templates. Preserve existing template IDs during migration. |
| Custom profiles and application rules | Keep as advanced options with one profile editor | Repeated per-app choices can justify them. They need not dominate basic setup. |
| Static snippets | Keep as an advanced dictionary feature | Deterministic phrase expansion has a bounded contract. Clipboard/selection variables add separate context requirements. |
| Selected-text shorten/translate/reply actions | Defer expansion and remove from the primary dictation flow | These form a writing assistant with preview, replacement, and distinct meaning-preservation rules. |
| File import, video decoding, SRT export | Move out of the primary experience; defer further expansion | These form a media transcription product. [SRT fallback](../../apps/desktop/src-tauri/src/services/transcription_workbench.rs) emits the entire transcript as one cue, which is valid syntax but weak subtitle utility. Keep basic text export. |
| Translation mode | Advanced, explicit target, no automatic inference | It has a legitimate use but a different output contract from cleanup. |
| Code mode and developer bridge | Advanced and enabled deliberately | Preserve the backend command interface. The current [startup path](../../apps/desktop/src-tauri/src/lib.rs) starts the authenticated loopback bridge unconditionally; headless service design does not require every user to run its listener. The transport is line-delimited JSON over TCP, not HTTP. |
| Quality metrics | Keep diagnostics; consolidate their UI with setup | Failure and latency evidence help maintenance. Users should not have to interpret a telemetry console to dictate. |
| Ghost-Action and Ghost-Language | Keep deferred; remove from the active product narrative | Both specs are drafts. Autonomous computer use and broad habit learning create separate products and verification obligations. This is roadmap pruning, not a claim that shipped modules were deleted. |

## Smallest useful follow-up sequence

1. Fix local-only privacy behavior and share polish acceptance across entry points. These are correctness gaps.
2. Remove the no-op streaming feature and current undo action. Preserve safe previews and recovery.
3. Finish the single-profile-model migration. Combine repeated cleanup prompt instructions without adding a template framework.
4. Replace the home dashboard with readiness and recovery. Put optional workflows and diagnostics behind advanced navigation.
5. Validate native delivery and transcription quality before adding more modes. Revisit deferred features only when a concrete repeated user task justifies their cost.

The resulting implementation should have one canonical profile model, one immutable recording snapshot, one transformation policy decision, and one final delivery decision. Avoid building new generic workflow or plugin frameworks during this reduction.

## Verification and limits

Executed during this review:

- `cargo test --lib services::platform_quality::tests:: -- --test-threads=1`: 18 passed.
- `cargo test --lib commands::audio::polish:: -- --test-threads=1`: 18 passed.
- `cargo test --lib product_workflows:: -- --test-threads=1`: 20 passed.
- ModelSettings, WorkflowPage, and PlatformQualityPage Vitest suites: 10 passed across 3 files. `pnpm` was unavailable; the installed Vitest entry point was run with Node and `--root apps/desktop`.
- `node scripts/check-md-links.mjs`: passed.

These checks establish the current focused baseline. They do not reproduce the new findings end to end, establish feature adoption, measure model accuracy, or verify native application behavior. No code fixes, feature deletions, full build, live cloud calls, or release were performed.

The documentation also needs narrower completion claims. The [polish safety spec](../feat/polish-output-safety/0.1.0/prd/erd.md) describes recording-path protection; the separate history/workflow paths need their own closure evidence. The [quality index](../quality/README.md) contains historical grades, including a Windows implementation note contradicted by the newer [Windows delivery spec](../feat/windows-text-injection/0.1.0/prd/erd.md). Use dated, workflow-specific evidence rather than treating those grades or navigation smoke tests as whole-feature verification.
