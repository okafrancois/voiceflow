---
title: Managed MLX local text polish
type: feat
status: completed
date: 2026-10-24
---

# Managed MLX local text polish

## Overview and problem frame

Deliver app-owned offline MLX text correction via the existing dictation dispatch. Current native and cloud polish remain intact. Contract: [feature specification](../../feat/mlx-polish/0.1.0/prd/erd.md).

## Scope boundaries

Dedicated polish runtime and curated multi-model catalogue; installation/download/readiness, backend dispatch and lifecycle tests; reactive existing UI. The implementation agent does not launch/install/quit applications or change settings/permissions. Parent acceptance additionally covers the signed development bundle, safe replacement, launch, and settings preservation. No production application, Git operations, capability changes, STT dependency changes, arbitrary checkpoint support, or unverified native UI/release claims.

## Implementation units

1. [x] Record authoritative dependency/API/model evidence; add failing catalogue, worker, dispatch, and runtime tests.
2. [x] Implement a dedicated hashed private runtime and bounded offline JSON-lines worker using existing lifecycle patterns.
3. [x] Integrate complete manifests, download/delete/cancel, compatibility, and unified backend dispatch without removing working engines.
4. [x] Run real French inference under `/tmp` through dictation's manager/service; verify cancellation/unload and model switching.
5. [x] Run targeted and broad regression checks; extract canonical workflow and record limits before parent review.
6. [x] Independent parent review and verified development bundle; final coordinated regression after parent-owned STT fixture hardening.

## System-wide impact

New polish engine variant and model IDs affect manager dispatch, backend catalogue, download commands and cache lifecycle. Existing prompt construction and output safety remain shared. A dedicated environment avoids any STT lock or readiness change. The bundled bootstrap resource is reused without broadening STT runtime duties.

## Risks and dependencies

- Current MLX-LM API and checkpoint manifests require primary-source verification and real hardware inference.
- Small multilingual model quality is not guaranteed by successful loading; French correction must be observed.
- Caller cancellation must stop a worker even if its async owner is dropped while inference is running.
- Apple Silicon native polish remains available during additive rollout; parent explicitly confirmed this policy, with no implicit settings migration. A later MLX-only migration needs a separate verified contract.
- Parent owns final review, rebuild, application installation and native UI verification.

## Verification evidence

- Initial inspection: existing STT uses private Python 3.12.15, MLX Audio 0.6.1 and MLX 0.32.4; no modifications made.
- Direct upstream retrieval confirmed MLX-LM 0.32.2 metadata and two immutable Qwen3 checkpoint manifests. Web tool returned empty responses; HTTPS API retrieval supplied evidence.
- Red: `python3 -m unittest discover -s apps/desktop/src-tauri/tests -p 'mlx_polish_worker_test.py'` failed because the managed polish worker did not exist. `cargo test --test managed_mlx_polish_test` failed with missing `polish_engine::mlx` and `PolishEngineType::Mlx`.
- First worker slice green: six injected-loader/generator Python tests cover prompt roles, disabled thinking, switching/unload, traversal/symlink rejection, incomplete-generation rejection, protocol stdout isolation and request validation.
- Dependency lock resolution initially exposed the upstream macOS 14 wheel floor; re-running with `MACOSX_DEPLOYMENT_TARGET=14.0` produced a complete hashed lock. This is a reproducibility requirement, not a reason to weaken wheel/platform validation.
- Pinned MLX-LM 0.32.2 wheel SHA256 `5c0991248cfff5872503d74f466d2532dd8cb7fb3670cb9be7a06d7305dadc04` matches PyPI metadata. Inspection confirms `load(..., tokenizer_config=..., trust_remote_code=False)`, `stream_generate(..., max_tokens=..., sampler=...)`, `make_sampler(temp=0)`, and final `finish_reason` values `stop`/`length`.
- Parent requested a production-edit/Cargo pause for the urgent STT language regression and development bundle. Only unreferenced new polish source and tests/docs existed at pause; no existing Rust/UI/settings/STT files had been modified. Resume only after the parent's explicit release.
- Both pinned checkpoints were retrieved into `/tmp/voiceflow-mlx-polish-validation/models` without inference: 0.6B has nine manifest files / 351,383,618 bytes; 1.7B has nine files / 984,013,244 bytes. Every downloaded file's exact byte count and SHA256 matched the curated manifest, including both weight files. This proves manifest integrity, not model execution quality.
- Existing STT Python worker baseline remains green (six tests). No STT files or runtime resources changed.
- Real offline worker inference through production `UnifiedPolishManager::polish` passed repeatedly for both checkpoints, using shared transcript preparation and output acceptance. Warm reuse reports zero reload; switching, stop/restart and final cache unload pass. The final real test requires one checkpoint to produce the complete exact expected French sentence, not merely contain one corrected word.
- Qwen3 1.7B produced `Bonjour, je suis allé à Paris hier et j'ai acheté trois pommes pour ma sœur.` Qwen3 0.6B produced `bonjour je suis aller a paris hier et j'ai acheté trois pommes pour ma soeur`: a real but partial correction. Both passed output acceptance; they are not equally validated for quality. The initial complete-correction assertion failed on 0.6B and is retained here as a discovered model limitation. Full input/output and one-sample timing caveats are in the feature specification; raw latest evidence is `/tmp/voiceflow-mlx-polish-validation/inference-evidence.json`.
- Selected availability red: an installed runtime incorrectly marked a missing selected model ready in the UI. Backend tests also failed on absent `available`. Green: backend emits selected model+runtime availability; UI renders it fail-closed. An async-flushed UI regression prevents a false green from initial pending state.
- Honest catalogue red: inherited MLX `fast`/`balanced` and template recommendations failed the new no-invented-ratings contract. Green: MLX profiles serialize `unmeasured`, recommendation/caution lists are empty, and UI omits empty template claims. Memory thresholds remain conservative compatibility guidance, not benchmarks.

## Verification commands and results

Run from the repository root unless prefixed with the Cargo directory. No normal desktop `pnpm build` or bundle build was run; the frontend-only command does not execute `cargo clean`.

This table records the original implementation handoff. The independent-review follow-up below supersedes its targeted counts and real-replay timing; parent owns the next combined full gates.

| Command | Observed result |
|---------|-----------------|
| `python3 -m unittest discover -s apps/desktop/src-tauri/tests -p 'mlx_polish_worker_test.py'` | Six passed; initial missing-worker red recorded. |
| `python3 -m unittest discover -s apps/desktop/src-tauri/tests -p 'mlx_worker_test.py'` | Existing STT six passed; no STT changes. |
| `cd apps/desktop/src-tauri && cargo test --test managed_mlx_polish_test` | Nine passed, including complete manifests, platform rejection, lock/runtime separation, readiness and unmeasured metadata. |
| `cd apps/desktop/src-tauri && cargo test --lib polish_engine::mlx` | Twelve lifecycle/integrity/installer tests passed; also included in full suite. |
| `cd apps/desktop/src-tauri && cargo test` | Latest completed full rerun passed: 1,068 passed / 44 ignored across 32 suites; library 788 passed / 8 ignored. An intervening run exposed the unrelated parent-owned STT PID-fixture race below; post-hardening final coordination remains parent-owned. |
| `cd apps/desktop/src-tauri && cargo clippy --all-features -- -D warnings` | Passed; upstream `block 0.1.6` future-incompatibility notice remains. |
| `cd apps/desktop/src-tauri && cargo fmt -- --check` | Passed globally, after formatting touched files only. |
| `cd apps/desktop/src-tauri && VOICEFLOW_MLX_POLISH_TEST_ROOT=/tmp/voiceflow-mlx-polish-validation cargo test --lib real_managed_mlx_polish_corrects_french_through_dictation_dispatch -- --ignored --nocapture` | Passed repeatedly with both real models; last exact-output run completed in 54.11 seconds including cached download/validation steps. |
| `pnpm --filter @voiceflow/desktop test` | 134 passed across 23 files; includes six PolishSection regressions and parent's selector/onboarding cleanup. |
| `pnpm --filter @voiceflow/desktop run build:local-install-frontend` | TypeScript/Vite build passed; existing chunk-size warning remains. |
| `pnpm --filter @voiceflow/shared typecheck` | Passed. |
| `pnpm check:i18n` | Passed; parent's removed STT explanatory keys remain removed; no new locale text added. |
| `pnpm check:md-links` | Passed. |

### Test race and coordination

After an exact-output real inference success, a full suite returned 787 passed / one failed / eight ignored in the library: `stt_engine::mlx::engine::tests::cancelled_worker_startup_preserves_runtime_readiness`, `engine.rs:618`, `ParseIntError { kind: Empty }`. Inspection showed its fixture waits for PID-file existence before parsing, while the shell can create the file before writing the PID. This is a test-publication race, not a polish inference failure. Preserve `/tmp/voiceflow-polish-cargo-stt-fixture-race.log`; exact isolated rerun `/tmp/voiceflow-polish-stt-fixture-rerun.log` passed. Parent owns deterministic STT fixture hardening; no STT source or tests edited by this task. The already-started full rerun was allowed to finish; no further full Cargo run starts before parent's fixture-ready release.

That already-started rerun finished successfully: 1,068 tests passed / 44 ignored, then clippy and global fmt passed. No Cargo, GPU, build, installer, or preview jobs from this task remain active. Source is ready for independent review; parent coordinates the post-fixture final regression and next bundle. No application/configuration changes or Git operations were performed.

### Evidence paths

- Full Cargo: `/tmp/voiceflow-polish-cargo-test.log`; clippy/fmt: `/tmp/voiceflow-polish-clippy.log`, `/tmp/voiceflow-polish-fmt.log`.
- Real dispatch: `/tmp/voiceflow-polish-real.log`; raw outputs: `/tmp/voiceflow-mlx-polish-validation/inference-evidence.json`.
- Worker/runtime: `/tmp/voiceflow-polish-python-green.log`, `/tmp/voiceflow-polish-runtime-tests.log`, `/tmp/voiceflow-polish-stt-baseline.log`.
- Availability red: `/tmp/voiceflow-polish-availability-red.log`, `/tmp/voiceflow-polish-availability-backend-red.log`; UI green: `/tmp/voiceflow-polish-availability-green.log`.
- Unmeasured claims red: `/tmp/voiceflow-polish-unmeasured-red.log`, `/tmp/voiceflow-polish-unmeasured-ui-red.log`; catalogue green: `/tmp/voiceflow-polish-unmeasured-green.log`.
- Frontend: `/tmp/voiceflow-polish-frontend-tests.log`, `/tmp/voiceflow-polish-frontend-build.log`, `/tmp/voiceflow-polish-shared-typecheck.log`, `/tmp/voiceflow-polish-i18n.log`.
- Documentation: `/tmp/voiceflow-polish-md-links.log`.

## Implementation file inventory

Paths below are relative to the repository root; no STT module, capabilities, locale file, or installed application was modified by this task.

New runtime/model/worker files:

- `apps/desktop/src-tauri/src/polish_engine/mlx/mod.rs`
- `apps/desktop/src-tauri/src/polish_engine/mlx/models.rs`
- `apps/desktop/src-tauri/src/polish_engine/mlx/runtime.rs`
- `apps/desktop/src-tauri/src/polish_engine/mlx/process.rs`
- `apps/desktop/src-tauri/src/polish_engine/mlx/engine.rs`
- `apps/desktop/src-tauri/src/polish_engine/mlx/engine_tests.rs`
- `apps/desktop/src-tauri/src/polish_engine/mlx/download_tests.rs`
- `apps/desktop/src-tauri/src/polish_engine/mlx/integrity.rs`
- `apps/desktop/src-tauri/src/polish_engine/mlx/integrity_tests.rs`
- `apps/desktop/src-tauri/src/polish_engine/mlx/worker.py`
- `apps/desktop/src-tauri/src/polish_engine/mlx/requirements.in`
- `apps/desktop/src-tauri/src/polish_engine/mlx/requirements.lock`
- `apps/desktop/src-tauri/tests/managed_mlx_polish_test.rs`
- `apps/desktop/src-tauri/tests/mlx_polish_worker_test.py`

Integrated existing files:

- `apps/desktop/src-tauri/Cargo.toml` and `apps/desktop/src-tauri/Cargo.lock`: direct SHA256/process-group dependencies already present transitively.
- `apps/desktop/src-tauri/src/polish_engine/mod.rs`: export new engine.
- `apps/desktop/src-tauri/src/polish_engine/traits.rs`: MLX enum/string contract and enum test count.
- `apps/desktop/src-tauri/src/polish_engine/compatibility.rs`: Apple Silicon compatibility, conservative memory guidance, unmeasured MLX latency/template metadata.
- `apps/desktop/src-tauri/src/polish_engine/local_http.rs`: expose existing thinking-block sanitizer for reuse; prompt implementation unchanged.
- `apps/desktop/src-tauri/src/polish_engine/unified_manager.rs`: register/route/catalogue/preload/cache/unload/idle/shutdown integration and ignored real dictation-dispatch test.
- `apps/desktop/src-tauri/src/commands/model.rs`: selected-model-aware catalogue/download/delete/cancel and existing progress/completion events.
- `apps/desktop/src-tauri/src/commands/model_cache.rs`: engine-specific runtime/load/availability state and async polish preload; backend availability regressions.
- `apps/desktop/src-tauri/src/lib.rs`: one polish bootstrap resource configuration call beside unchanged STT configuration.
- `apps/desktop/src-tauri/tests/polish_engine_test.rs`: platform-conditional engine count.
- `apps/desktop/src/lib/tauri.ts`: typed engine metadata, authoritative availability, unmeasured/unsupported contract values.
- `apps/desktop/src/components/Home/model/PolishSection.tsx`: reactive availability, hide unrelated HTTP controls for MLX, omit invented gauges/empty template claims.
- `apps/desktop/src/components/Home/__tests__/PolishSection.test.tsx`: MLX UI and availability regressions.

Documentation:

- `context/feat/mlx-polish/0.1.0/prd/erd.md`
- `context/plans/completed/2026-10-24-006-feat-mlx-polish-plan.md`
- `context/README.md`, `context/feat/README.md`, `context/plans/README.md`: indexed before implementation.
- `context/guides/adding-polish-provider.md`: dedicated managed-engine integration/verification workflow and signing trust boundary.
- `context/reference/providers/polish.md`: shipped dispatch/runtime contracts and quality limits.

## Handoff limits

Implementation is headless-verified, not installed-app validated. Parent retains independent review/build/install ownership. Cold runtime installation/resource discovery after code-signing, native application UI, other physical platforms, and broader French/multilingual/template quality remain unverified. No general comparative speed/accuracy claim is made. No external TCP process is required or controlled by the MLX polish path. Existing Apple/native/cloud engines and STT behavior remain intact. Keep this plan active through parent review and the coordinated next bundle.

## Independent review r1 — active follow-up

Three P2 findings are accepted within managed polish only: unbounded shared-downloader HTTP waits, same-size post-download corruption before first load, and missing macOS 14 eligibility outside installation. Add failing-first stalled-header/body cancellation/deadline/retry tests, first-load config/weights integrity/repair/cancellation tests, and old/new/malformed/unsupported OS policy tests before fixing them. Keep native/Apple/cloud and STT untouched. Parent's STT PID fixture is fixed and its combined baseline is 1,071 passed / 44 ignored. This follow-up runs targeted tests only; parent owns the next full gates and bundle.

### Resolutions and proof boundaries

- Download: a managed-only wrapper races each shared-downloader future against cancellation and a 20-minute per-file total deadline. A drop guard cleans partial artifacts without deleting preexisting complete files. Header/body stalls each have cancellation and expiry tests plus a successful retry; the existing exact `cancelled` backend event path is preserved. Shared/native download code is unchanged.
- Integrity: first worker use hashes the full allowlisted manifest. A per-engine cache stores the manifest signature plus verified size/mtime and Unix device/inode/ctime identities; changes force rehashing. Confirmed mismatch removes `.model-ready` and only the corrupt file for repair. Runtime readiness is not invalidated by checkpoint corruption, cancellation or verification expiry. Hash work checks cancellation/deadline between 64 KiB reads, including cancellation when its awaiting future is dropped. Tests cover same-size config/weight corruption, post-cache corruption, repair, preserved healthy runtime/files, and mid-hash cancellation/expiry.
- Eligibility: one pure version policy gates managed polish catalogue, compatibility, readiness, download and execution. A once-cached `NSProcessInfo` version replaces repeated process observation. macOS 12/13 and unknown/malformed versions fail closed; macOS 14+ passes; Intel/Windows/Linux remain unsupported for MLX. Native alternatives and STT's separate hardware-only contract are unchanged.
- Budget correction: the first real post-integrity replay rejected the 984,013,244-byte checkpoint at the original 30-second hash limit in a debug build. Failure is preserved in `/tmp/voiceflow-polish-r1-real-green.log` despite its preselected filename. Preparation now has a 60-second budget including the worker lock; worker preparation/startup retains a separate 30-second bound; inference gets the original caller timeout, with an additive outer cap. No debug/test integrity bypass is used. Two delayed-preparation tests failed under the old whole-request timeout and pass with separate stages.

Initial integrity red was a missing-module compile failure, not proof of a behavioral rejection. The first-load worker-dispatch test additionally has a behavioral negative control: temporarily dropping its integrity preparation reproduced the original bypass and incorrectly accepted a corrupt checkpoint. The gate was restored before green tests. Download and OS-policy red logs record actual behavioral failures.

### Review follow-up evidence paths

- Download red/green: `/tmp/voiceflow-polish-r1-download-red.log`, `/tmp/voiceflow-polish-r1-download-green.log`.
- Integrity initial red/green: `/tmp/voiceflow-polish-r1-integrity-red.log`, `/tmp/voiceflow-polish-r1-integrity-green.log`; first-load bypass negative control: `/tmp/voiceflow-polish-r1-first-load-negative-control.log`.
- OS policy/compatibility red: `/tmp/voiceflow-polish-r1-os-red.log`, `/tmp/voiceflow-polish-r1-os-compat-red.log`.
- Separated-budget red/green: `/tmp/voiceflow-polish-r1-budget-red.log`, `/tmp/voiceflow-polish-r1-budget-green.log`.
- Latest targeted suites: `/tmp/voiceflow-polish-r1-targeted-green.log`, `/tmp/voiceflow-polish-r1-compat-green.log`, `/tmp/voiceflow-polish-r1-contract-green.log`, `/tmp/voiceflow-polish-r1-status-green.log`.
- Python/type checks: `/tmp/voiceflow-polish-r1-python-green.log`, `/tmp/voiceflow-polish-r1-types-green.log`.
- Shared transcript/output transform regressions: `/tmp/voiceflow-polish-r1-transform-green.log`.
- Real replay failure and corrected green: `/tmp/voiceflow-polish-r1-real-green.log` (preserved timeout failure), `/tmp/voiceflow-polish-r1-real-replay.log` (passed); raw latest input/output/metrics: `/tmp/voiceflow-mlx-polish-validation/inference-evidence.json`.
- Scoped formatting, i18n and documentation: `/tmp/voiceflow-polish-r1-fmt.log`, `/tmp/voiceflow-polish-r1-i18n.log`, `/tmp/voiceflow-polish-r1-md-links.log`.

### Final targeted follow-up results

Run Cargo commands from `apps/desktop/src-tauri`; frontend/Python/documentation commands from the repository root. No full Cargo/clippy/build chain was started in this follow-up.

| Command | Observed result |
|---------|-----------------|
| `cargo test --lib preparation_ -- --nocapture` | Two delayed-preparation regressions: red two failures, green two passed. Success after preparation longer than caller timeout; stalled inference still expires after its separate deadline and reaps the worker. |
| `cargo test --lib polish_engine::mlx` | 27 passed, including four stalled-download tests, five model-integrity tests, first-load worker rejection and separate execution budgets. |
| `cargo test --lib polish_engine::compatibility` | Ten passed; old/new/malformed/unsupported OS policy and unchanged native alternatives. |
| `cargo test --test managed_mlx_polish_test --test polish_engine_test` | Nine managed contract tests and 12 polish integration tests passed; one preexisting native inference test ignored. |
| `cargo test --lib commands::model_cache::tests` | Two selected-availability tests passed. |
| `cargo test --lib services::text_transform` | 16 shared correction/output-safety tests passed. |
| `VOICEFLOW_MLX_POLISH_TEST_ROOT=/tmp/voiceflow-mlx-polish-validation cargo test --lib real_managed_mlx_polish_corrects_french_through_dictation_dispatch -- --ignored --nocapture` | One passed in 111.89 seconds, with real offline 0.6B and 1.7B checkpoints, production first-use integrity, warm reuse, switching, stop/restart and unload. |
| `python3 -m unittest discover -s apps/desktop/src-tauri/tests -p mlx_polish_worker_test.py` | Six passed. |
| `pnpm --filter @voiceflow/desktop exec tsc --noEmit` | Passed. |
| `rustfmt --edition 2021 --check` on the ten review-touched Rust source/test paths listed below | Passed; no STT formatting edits. |
| `pnpm check:i18n` and `pnpm check:md-links` | Passed; no locale keys changed. |

The real accepted sentence remains complete with 1.7B and partial with 0.6B; neither receives a general quality rating. Latest first-use total/load/inference milliseconds were 13,661/288/101 and 35,305/288/173 respectively. Debug total includes first-use SHA256, unlike the earlier timings; do not treat these as comparative benchmarks. Preparation remains bounded to 60 seconds and could expire on a slower machine/disk; no cross-device timing guarantee is claimed.

At follow-up handoff all jobs owned by this task have finished. Parent has started its single final combined Cargo/clippy/fmt and frontend gates and a fresh read-only review r3; this task makes no further source/formatting/GPU/Cargo changes. Earlier r2 was cancelled while timeout behavior was still under verification. Full-gate/review outcomes and the eventual application bundle belong to the parent; this plan/spec remain active.

### Review follow-up file inventory

New: `apps/desktop/src-tauri/src/polish_engine/mlx/download_tests.rs`, `apps/desktop/src-tauri/src/polish_engine/mlx/integrity.rs`, `apps/desktop/src-tauri/src/polish_engine/mlx/integrity_tests.rs`.

Modified source/tests: `apps/desktop/src-tauri/src/polish_engine/mlx/mod.rs`, `apps/desktop/src-tauri/src/polish_engine/mlx/models.rs`, `apps/desktop/src-tauri/src/polish_engine/mlx/engine.rs`, `apps/desktop/src-tauri/src/polish_engine/mlx/engine_tests.rs`, `apps/desktop/src-tauri/src/polish_engine/mlx/runtime.rs`, `apps/desktop/src-tauri/src/polish_engine/compatibility.rs`, `apps/desktop/src-tauri/src/commands/model.rs`, `apps/desktop/src/lib/tauri.ts`.

Documentation: this plan, `context/feat/mlx-polish/0.1.0/prd/erd.md`, `context/guides/adding-polish-provider.md`, `context/reference/providers/polish.md`. No dependencies/locks, worker Python, STT, UI component, settings, locale, capabilities, or application state changes are part of the review fixes. Keep this plan active pending fresh independent review, parent-owned combined gates, and coordinated bundle verification.

## Parent final combined verification

After the review fixes, separated deadlines, selector cleanup, and completed STT PID-fixture hardening, the parent ran the final frozen source through the combined gates:

| Command | Observed result and evidence |
|---------|------------------------------|
| `cargo test` | 1,087 passed, zero failed, 44 ignored across 32 standard harness result blocks; custom macOS executables also completed successfully. `/tmp/voiceflow-managed-mlx-polish-final-test.log`. |
| `cargo clippy --all-features -- -D warnings` | Passed. `/tmp/voiceflow-managed-mlx-polish-final-clippy.log`. |
| `cargo fmt -- --check` | Passed globally. `/tmp/voiceflow-managed-mlx-polish-final-fmt.log`. |
| `pnpm --filter @voiceflow/desktop test` | 134 passed across 23 files. `/tmp/voiceflow-managed-mlx-polish-final-frontend-test.log`. |
| `pnpm --filter @voiceflow/desktop run build:local-install-frontend` | TypeScript/Vite build passed; existing large-chunk warning remains. This frontend-only command does not clean Cargo. `/tmp/voiceflow-managed-mlx-polish-final-frontend-build.log`. |
| `pnpm --filter @voiceflow/shared typecheck` | Passed. `/tmp/voiceflow-managed-mlx-polish-final-shared.log`. |
| `pnpm check:i18n` | Passed. `/tmp/voiceflow-managed-mlx-polish-final-i18n.log`. |
| `node --test` for the five bootstrap/resource/signing/local-install script suites | 43 passed. `/tmp/voiceflow-managed-mlx-polish-parent-distribution.log`. |

The parent independently replayed real French manager dispatch before the review fixes; the author's final-source replay with integrity enabled is recorded above. Neither mocked worker tests nor component browser fixtures establish installed-app inference or native UI behavior.

An additional, non-required `cargo clippy --all-features --tests -- -D warnings` audit exposed existing test-only lints outside these changes in downloader/beep/Volcengine/common and older library fixtures. That broader audit is not claimed green; no unrelated test code or lint suppression was added. Evidence: `/tmp/voiceflow-managed-mlx-polish-combined-clippy.log`.

Fresh independent review r3 is pending. Review r2 was cancelled during the known integrity-timeout correction and supplies no accepted verdict. No development bundle was built or installed at this checkpoint; all parent verification jobs have finished. The installed development executable and settings hashes were recorded before replacement, without changing settings. Final signing/resource discovery, safe installation, launch, and the review disposition remain required before closing this plan.

### Review r3: service-level deadline follow-up

Review r3 closed the original three findings but identified that `services::text_transform::transform_text` still wraps the complete local manager operation in the same short-text inference allowance. The 35,305 ms first-use 1.7B observation exceeds that 20-second service deadline. Earlier real manager tests did not traverse that enclosing timeout, so they are insufficient evidence for first-use dictation completion.

The parent will preserve MLX's bounded staged deadlines at the service boundary without changing native/cloud timeout behavior. Add behavioral delayed-preparation and stalled-inference regressions at that boundary, then replay the real ignored test through the same service deadline policy and short-text allowance. Preserve the original manager-only evidence; do not relabel it as complete service verification. No bundle is built until the fix, fresh review, and combined gates pass.

The parent extracted the existing local timeout boundary into `services::text_transform::await_local_polish`, preserving its original unconditional behavior for the failing-first run. Both MLX service regressions failed behaviorally with `local polish timed out`: delayed preparation could not finish, and the service masked the engine's separate inference deadline. The fix awaits MLX's already-bounded staged manager operation directly; native local engines retain their whole-operation timeout/error and the cloud branch is unchanged. All 19 shared transform tests then passed, including the two MLX regressions and native Qwen/LFM/Gemma/GLM/Apple deadline preservation.

The existing ignored real test now applies the same service helper to every first/warm/restarted manager request with `local_polish_timeout(input)` (20 seconds for this short sentence), not its former 60-second manager-only allowance. Real offline replay passed in 97.51 seconds with first-use SHA256 enabled, both checkpoints, common output acceptance, warm reuse/switching/stop/restart/unload. This is verified service-deadline policy plus real manager inference, not live microphone or installed-app UI verification. Evidence: `/tmp/voiceflow-polish-service-deadline-red.log`, `/tmp/voiceflow-polish-service-deadline-green.log`, `/tmp/voiceflow-polish-service-deadline-real.log`; raw outputs remain in `/tmp/voiceflow-mlx-polish-validation/inference-evidence.json`.

Only `services/text_transform.rs` and the existing ignored test in `polish_engine/unified_manager.rs` changed in this follow-up, plus canonical documentation. Fresh read-only review r4 and post-fix combined Rust gates are pending; frontend source is unchanged from the final 134-test/build/typecheck/i18n pass.

## Final parent disposition and development delivery

This final disposition supersedes the historical active-handoff checkpoints above. Review r3 closed all three original findings; review r4 found no actionable issue and closed the service-level timeout finding. Both reviewed source/evidence only, without running tests or applications. Cancelled review r2 supplies no verdict.

Post-service-fix `cargo test` passed **1,090 tests, zero failed, 44 ignored**, across 32 standard harness result blocks; custom macOS executables also succeeded. Required `cargo clippy --all-features -- -D warnings` and global `cargo fmt -- --check` passed. Evidence: `/tmp/voiceflow-polish-service-final-test.log`, `/tmp/voiceflow-polish-service-final-clippy.log`, `/tmp/voiceflow-polish-service-final-fmt.log`. The already-verified 134 frontend tests, frontend TypeScript/Vite build, shared typecheck, i18n, six Python polish tests, and 43 distribution tests remain applicable; no frontend source changed in the service fix. Final documentation links are checked after plan closure. The extra test-target clippy audit remains existing debt, not a passing gate.

The parent ran `npm --prefix apps/desktop run tauri:build:mac:local-install` after all Cargo/GPU jobs finished. The development build completed, ad-hoc signed, and passed deep strict signature verification, development identifier/URL-scheme/audio entitlement checks. The actual bundled `Contents/Resources/bin/apple-silicon/uv` was present, executable, and reported version 0.13.5. Bundled Apple Silicon/Intel native runtime resources were verified and the compatible native executable smoke test passed. This does not claim notarization or cold MLX installation through the installed application's UI. Evidence: `/tmp/voiceflow-managed-mlx-polish-dev-bundle.log`, `/tmp/voiceflow-managed-mlx-polish-bundle-verification.log`.

At **2026-10-24 14:26:32 Europe/Paris** the verified bundle was installed and launched at `/Users/bernyitoutou/Applications/Voice Flow Dev.app`, identifier `com.voiceflow.voicetotext.dev`. The old development app was not running. The replacement was staged with `ditto`, verified before swapping, and the previous bundle preserved at `/Users/bernyitoutou/Applications/.Voice Flow Dev.backup-2026-10-24T12-26-29.470Z`. Installed/source executable SHA256 matched `b44c3fbdc16f2f499b8987e405e573dbbbe24f04c2b77a3430756ea5cfb73bfe`; the exact installed executable ran as PID 44681 and its CLI returned ready/idle, not recording or transcribing. Receipt: `/tmp/voiceflow-managed-mlx-polish-dev-install.json`.

The development settings SHA256 remained exactly `6bb1faddafe828bb63c4cb67558299d0bf5c22cff3ef28c0cd82af16e76ed61a` before replacement, after launch, and after the final readiness check. No settings or selected models were changed; no polish model was downloaded into real application data. Production application/configuration, permissions, capabilities, and the separate MLX-Audio installation were untouched.

The installed bundle contains both managed polish and the completed STT selector/onboarding copy cleanup. It does not add microphone/text streaming to managed STT; that path still returns a final transcription after recording stops. No fabricated performance scores were added.

Native visual verification remains limited: existing screen-capture permission was observed without requesting new permission, but the development windows were offscreen and the captured main window contained only its empty background. Reopen/activation did not provide a usable rendered UI capture. This is not accepted as a visual test and does not establish whether the offscreen capture reflects actual frontend rendering. The process/CLI/signature checks pass; installed selector/polish interaction and live microphone acceptance remain unverified. Evidence: `/tmp/voiceflow-managed-mlx-polish-native-ui-preflight.json`, `/tmp/voiceflow-managed-mlx-polish-native-window.png`, `/tmp/voiceflow-managed-mlx-polish-native-activation.log`.

Remaining explicit limits: cold runtime installation from the signed installed app, native UI/live-microphone acceptance, other physical platforms, preparation timeout on slower machines/disks, broad French/multilingual/template quality, and production/notarized release validation. The isolated real 1.7B sentence was fully corrected; 0.6B remains partial-only, not an equal-quality recommendation. All task-owned verification/build jobs have finished; the requested development application remains running.
