---
title: Live managed MLX transcription
type: feat
status: completed
date: 2026-10-24
---

## Overview and problem frame

Implement [live managed MLX transcription](../../feat/mlx-live-transcription/0.1.0/prd/erd.md). The current managed consumer buffers until recording stops; the pinned worker already transcribes arbitrary complete private WAV snapshots. The UI does not subscribe to partial transcription events.

## Scope boundaries

Managed STT live preview, backend event routing and reactive pill/home rendering. The user explicitly expanded scope to enforce [MLX and Apple only on Apple Silicon](../../feat/apple-silicon-model-policy/0.1.0/prd/erd.md) for both STT and polish; preserve Intel/Windows/Linux and vendor-cloud policies. Preserve final full-audio transcription, production app/settings, permissions and capabilities. No Git operations or dependency upgrades.

## Implementation units

- [x] Inspect pinned API options and current capture/consumer/UI boundaries.
- [x] Add failing-first bounded preview/cancellation/final-tail and event rendering regressions.
- [x] Implement a single coalesced managed live consumer and backend-only publication gating.
- [x] Render partial replacements in existing dictation surfaces without control logic.
- [x] Enforce the explicitly requested MLX + Apple hardware policy for both local catalogues, operations and settings migration, preserving other platforms/cloud.
- [x] Verify real French partial-before-finish for all checkpoints, final accuracy, lifecycle and required gates; independent review.
- [x] Extract canonical contracts, evidence and explicit delivery limits.

## System-wide impact

User clarification during implementation supersedes the initial additive polish rollout. On Apple Silicon, offer only managed MLX and built-in Apple engines for both local catalogues. Add the backend-owned `· MLX` suffix to managed polish names. Preserve valid Apple/MLX selections, migrate excluded native polish selections idempotently to `mlx-qwen3-1.7b`, and retain cached legacy files. Enforce selection/download/load/inference policy in Rust, including direct native polish entry points. Intel/Windows/Linux and unrelated cloud configuration remain unchanged. Hardware exclusion is independent of the MLX macOS 14 runtime floor; older ARM Macs must not accidentally regain native alternatives.

Dictation capture chooses the managed live consumer; import/retry remain full-audio. Existing partial events remain compatible and provisional text never enters history/clipboard/polish. Snapshot inference reuses the selected worker and model cache.

## Risks and dependencies

Preview cost depends on hardware/model; coalescing and a bounded recent window prevent backlog, not universal latency guarantees. Window-edge provisional words can change; the complete final inference remains authoritative. Dropping an in-flight decode can restart the owned worker. No model quality/rate claims without measurements. Never run the cleaning desktop build concurrently with Cargo.

## Verification evidence

### Failing-first and focused checks

- Consumer: 10 behavioral failures before implementation; 18 live-preview checks and 12 existing buffering checks pass afterward (two existing ignored). Logs: `/tmp/voiceflow-mlx-live-consumer-{red,green}.log`, `/tmp/voiceflow-mlx-live-buffering-green.log`.
- Capture: stale-event gate reproduced, then two checks pass. A pending optional context capture blocked ingestion; the behavioral regression failed before concurrent polling, then all eight capture checks pass. Logs: `/tmp/voiceflow-mlx-live-capture-{red,green}.log`, `/tmp/voiceflow-mlx-live-context-{red,green}.log`. A wrapper `.abort()` compile omission was corrected before these runs.
- Frontend: six initial failures; focused hook/pill/dashboard checks pass. Independent review identified first-line clamping that hid newly spoken words. A scroll-position regression failed, then passes after bottom-following replaces line clamping. Logs: `/tmp/voiceflow-mlx-live-pill-tail-{red,green}.log`.
- Policy: seven platform failures plus MLX display-name failure, and two migration/selection failures, reproduced before the policy. Final focused checks: 77 integration, seven settings, 43 manager, ten compatibility, three OS-floor and five command/status checks pass. Logs: `/tmp/voiceflow-policy-*.log`, `/tmp/voiceflow-settings-policy-{red,green}.log`.

### Real managed inference

`tests/managed_mlx_live_test.rs` replays resampled French audio in 200 ms chunks using isolated checkpoints/runtime, with no user settings changes or downloads:

```sh
VOICEFLOW_MLX_TEST_ROOT=/tmp/voiceflow-managed-mlx-validation \
VOICEFLOW_MLX_TEST_WAV="$HOME/Applications/MLX-Audio/samples/kokoro-fr.wav" \
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml \
  --test managed_mlx_live_test -- --ignored --nocapture
```

Passed once in 42.62 seconds across all three models. First partials arrived before later audio chunks and before stop: Whisper 2,176 ms, Parakeet 2,339 ms, Qwen 1,965 ms. These are synthetic fixture observations, not product latency benchmarks. French partials may revise mistaken early words. Each final contains the expected French phrase; Whisper collapses fixture repetitions, so this is not proof of repeat preservation or general accuracy. Final fake-decoder checks separately prove every input sample is retained in order. Log: `/tmp/voiceflow-mlx-live-real.log`.

After the policy source freeze and full gates, the same all-three-model check passed again in 41.59 seconds. First partial observations were 2,129 / 1,931 / 1,975 ms respectively, with French finals and no post-finish callbacks. Log: `/tmp/voiceflow-mlx-live-real-final.log`.

### Reactive rendering and gates

- Full frontend suite: 157 tests in 25 files pass. Safe frontend-only build, shared typecheck and i18n pass. Logs: `/tmp/voiceflow-mlx-live-{frontend-final,frontend-build,shared,i18n}.log`.
- Production pill component with mocked backend events, rendered at 240 × 140 pixels: trailing text is visible, preview stays inside the window, idle clears it and stale events do not restore it. Screenshot `/tmp/voiceflow-mlx-live-pill-green.png`; measurements `/tmp/voiceflow-mlx-live-browser-evidence.json`. This is browser rendering, not installed native acceptance. T3 snapshots twice failed with `Preview automation snapshot timed out after 15000ms`; an existing local Chromium binary supplied the fallback screenshot, without installing a browser.
- Final combined Rust gates pass: `cargo test` reports 1,130 passing / 45 ignored across 34 standard suites (library 840 passing / eight ignored); `cargo clippy --all-features -- -D warnings` and `cargo fmt -- --check` pass. The two custom macOS harnesses also pass. Logs: `/tmp/voiceflow-mlx-live-{cargo,clippy,fmt}-final.log`. An existing dependency future-incompatibility warning for `block` remains; no new Clippy warnings.
- Python worker suites: 12 tests pass. Bootstrap/resource/signing/local-build script checks: 32 tests pass. Markdown links pass. Logs: `/tmp/voiceflow-mlx-live-{python,distribution}-final.log`, `/tmp/voiceflow-mlx-live-md-links.log`.
- Signed development delivery and final review are recorded below, after the availability corrections.

## Verification limits

The real test covers chunk replay through the consumer/manager, not a live microphone or complete `AppState` workflow. No corpus benchmark, arbitrary long-recording accuracy guarantee, physical non-ARM execution or new Apple engine quality claim. Existing ignored native-specific inference tests remain native-specific and are not acceptance checks on Apple Silicon. Preserve valid Apple Intelligence settings; do not silently select MLX for a user already using Apple.

## Independent review and availability corrections

- R1 could not execute because of provider rate limits; no verdict. R2 found pill leading-line clamping; the behavioral regression and browser evidence above close it. R3 found no new live lifecycle defect, but identified three platform/availability P2s.
- R3 catalogue membership: polish used macOS runtime eligibility after hardware policy removed native models, leaving no entries on ARM macOS 13. Membership is now a pure hardware policy; macOS 13 eligibility remains rejected and catalogue metadata/readiness remain unsupported. The new catalogue seam first failed compilation (missing helper and an invalid private-helper test reference), not a claimed behavioral run on macOS 13. The final pure regression covers hardware membership plus public OS eligibility.
- R3 initial Apple Speech race: catalogue IPC now awaits current-language background status with `Notify` and a ten-second deadline. It does not block the main thread or await Apple status for a specific non-Apple engine. Behavioral red: initial query returned prematurely. Green checks include delayed readiness, confirmed unsupported status, deadline/retry, old-language cache rejection and multiple waiters. Direct model selection rejects unready/pending Apple Speech before mutation. Behavioral red: the old selector accepted `None` and changed cloud routing. Logs: `/tmp/voiceflow-mlx-policy-r3-{catalogue-red,apple-red,apple-green}.log`.
- Language changes now trigger a reactive backend catalogue reload. The component regression failed with one backend call instead of two, then passes with unavailable built-in rendering. A test-scaffolding failure expected a Download button that built-in rows do not offer; its assertion was corrected to the existing unavailable state, without changing functionality for the test. Logs: `/tmp/voiceflow-mlx-policy-r3-language-{red,green,assertion-failure}.log`. All 16 model-settings tests pass.
- R4 closed the three R3 backend objections, then found missing Apple asset-install action and out-of-order language responses. Backend-owned `ModelInfo.downloadable` now authorizes the existing install/download command only for missing Apple assets. Shared catalogue rendering exposes the authorized action without allowing built-in deletion or automatic selection. A monotonically increasing UI request sequence rejects superseded success/error responses and is invalidated on effect cleanup. These counters order rendering only; readiness and installation remain backend-owned.
- Both R4 component failures were reproduced behaviorally (16 existing checks passing / two new failures). Backend metadata initially failed compilation because the new contract field was absent. Logs: `/tmp/voiceflow-mlx-policy-r4-ui-{red,green}.log`, `/tmp/voiceflow-mlx-policy-r4-metadata-{red,green}.log`, `/tmp/voiceflow-mlx-policy-r4-types-green.log`. Final source review and combined gates are recorded below; no live consumer, worker or final decoding source changed in these availability corrections.

### Final combined gates after availability/UI fixes

- `cargo test`: 1,137 passing / 45 ignored across 34 standard suites; library 847 passing / eight ignored. The two custom macOS harnesses report five additional PASS checks. `cargo clippy --all-features -- -D warnings` and `cargo fmt -- --check` pass. Logs: `/tmp/voiceflow-mlx-live-{cargo,clippy,fmt}-final-r5.log`.
- Desktop frontend: 160 tests in 25 files pass; safe frontend-only build, shared typecheck, i18n and markdown links pass. Logs: `/tmp/voiceflow-mlx-live-frontend-final-r5.log`, `/tmp/voiceflow-mlx-live-frontend-build-r5.log`, `/tmp/voiceflow-mlx-live-shared-r5.log`, `/tmp/voiceflow-mlx-live-i18n-r5.log`, `/tmp/voiceflow-mlx-live-md-links.log`.
- Last real three-model live replay after the backend availability changes passes in 42.30 seconds: `/tmp/voiceflow-mlx-live-real-final-r4.log`. The subsequent metadata/UI slice changes no decoding, worker or sample-retention behavior.
- R5 independent source review reports no findings in Apple asset installation metadata, action routing or superseded catalogue responses. Its scope is source and existing logs, not independent test execution or installation. No production settings, permissions or capabilities have been modified.

### Signed development delivery

- Built with `npm --prefix apps/desktop run tauri:build:mac:local-install`, without Cargo cleaning or capability changes. The build verifies the DEV identifier, URL scheme, audio entitlement and strict deep signature. Build log: `/tmp/voiceflow-mlx-live-dev-build.log`.
- Installed and launched `~/Applications/Voice Flow Dev.app` on 2026-10-24 at 13:49:33 UTC. The previous idle DEV process was stopped by its exact PID and executable path. The original bundle is retained at `~/Applications/.Voice Flow Dev.backup-2026-10-24T13-49-31.228Z`; staging was copied with `ditto` and verified before replacement. Production was not targeted.
- Source and installed executable SHA-256 match: `340e7366de628468ff1c8c04c3aeaa5a4bc6d6da9e4481646799a6022f09ef34`. Installed strict deep signature verification succeeds; bundled ARM `uv --version` executes successfully. The local bundle is ad-hoc signed, not notarized.
- Installed CLI reports idle, not recording or transcribing; launched PID 32953 uses the installed executable. Settings SHA-256 is unchanged across restart: `69b76e63646fce7b09d583b35cd712a6d1876556f137fa93f2e5c9c07caa7ec4`. Existing `mlx-whisper-turbo` transcription and `mlx-qwen3-1.7b` polish selections are preserved. Receipt: `/tmp/voiceflow-mlx-live-dev-install.json`.
- Delivery verification establishes bundle identity, packaged bootstrap execution, launch and backend responsiveness. Real French audio was validated through isolated three-model chunk replay, not an installed live microphone session. Native installed UI interaction, physical Intel/Windows/Linux behavior and general language/latency benchmarks remain unverified.

### Extraction

Keep provisional snapshots replaceable and separate from authoritative final audio. Catalogue membership follows hardware policy; runtime eligibility and Apple asset readiness remain explicit backend metadata. Rendering request sequencing must not become engine-selection policy. Local DEV delivery uses the non-cleaning build and preserves app data, an original bundle backup and a machine-readable verification receipt.

## Source map

- Live lifecycle: `apps/desktop/src-tauri/src/stt_engine/{live_preview.rs,buffering_engine.rs,mod.rs}` and `commands/audio/capture.rs`.
- Reactive UI: `apps/desktop/src/{hooks/useLiveTranscription.ts,lib/tauri.ts,components/Home/Dashboard.tsx,components/Pill/PillWindow.tsx,components/Home/ModelSettings.tsx,components/Home/model/ModelCatalog.tsx,components/Home/model/VoiceInputSection.tsx}`, hook/pill/model-settings tests, and `src-tauri/tests/managed_mlx_live_test.rs`.
- Policy: `src-tauri/src/stt_engine/{models.rs,unified_manager.rs}`, `polish_engine/{traits.rs,unified_manager.rs,compatibility.rs,local_http.rs,mod.rs,mlx/models.rs}`, `commands/{model.rs,settings/mod.rs,settings/__test__/mod.rs}`.
- Policy regressions: `src-tauri/tests/{apple_silicon_policy_test.rs,managed_mlx_test.rs,managed_mlx_polish_test.rs,polish_engine_test.rs,qwen_engine_test.rs,lfm_engine_test.rs,unified_manager_comprehensive_test.rs}`. Existing native test branches remain present.
