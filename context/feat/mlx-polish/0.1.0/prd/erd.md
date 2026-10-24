---
title: Managed MLX local text polish
version: 0.1.0
status: completed
date: 2026-10-24
---

# Managed MLX local text polish

## Problem and observable goal

Apple Silicon dictation already owns its MLX transcription runtime, but local text polish still depends on a native HTTP runtime. Selecting and downloading a curated polish model must be sufficient for offline correction through the same backend dispatch as dictation, without manual Python, a separately launched server, or TCP ports.

## Scope

- Add a dedicated, versioned private Python 3.12.15 / MLX-LM environment, installed by the bundled verified `uv` bootstrap with a fully hashed binary dependency lock.
- Offer multiple explicitly curated MLX safetensors checkpoints with immutable revisions and complete file manifests. Arbitrary GGUF, ONNX, or Hugging Face repositories are not supported.
- Curated MLX display names identify their engine with `· MLX`, consistently with transcription models. Existing native/Apple alternatives remain distinguishable without explanatory paragraphs or automatically changing the selected model.
- Own installation, readiness, download/cancellation, persistent JSON-lines worker, timeouts, cancellation, model switching, idle unload, and shutdown in Rust.
- Preserve templates, profile prompts, source language, untrusted screen context, contextual correction, preview callbacks, output rejection/fallback, and optional cloud polish.
- Keep compatible existing native engines on Intel macOS, Windows, and Linux. The initial additive Apple Silicon rollout is superseded by the explicit [MLX and Apple model policy](../../../apple-silicon-model-policy/0.1.0/prd/erd.md): offer only managed MLX and built-in Apple engines on that hardware, with deterministic legacy-local migration and preserved unrelated cloud settings. MLX execution is rejected authoritatively on incompatible platforms.
- Do not change STT runtime dependencies, readiness signatures, settings, lifecycle duties, production configuration, capabilities, or application installation/launch state.

## First-principles architecture

A model family and its execution format are different contracts. Dedicated MLX polish IDs route through `UnifiedPolishManager` and `PolishEngine`; the frontend renders the backend catalogue. The environment is separate from MLX Audio because changing a verified transcription environment to add language generation expands its dependency and failure surface. Only the existing bundled bootstrap resource is shared. The worker accepts application-owned model directories and offline loading only; never remote code.

## Data and information architecture

Catalogue entries retain the existing ID/name/size/downloaded/compatibility/latency contract and expose their engine. Each MLX manifest records repository, immutable revision, filenames, exact byte lengths, and hashes. Runtime-ready is published only after import/version validation. Model-ready requires all manifest files and runtime readiness. Installation progress uses the existing polish download events, with an optional runtime/model stage. Existing settings store the explicit selected model ID; no parallel frontend state is introduced.

`get_polish_model_status.available` is authoritative for the selected model and requires both model and runtime readiness. A runtime installed for another model cannot make a missing selected model available. MLX latency class/code remain `unmeasured`, with no invented numeric ratings or per-template recommendations. Memory compatibility thresholds are conservative admission guidance, not measured quality or performance ratings.

## Acceptance and BDD

1. Given a supported Mac and a curated ID, downloading installs the private runtime and complete pinned model; a cancelled/failed install never reports ready.
2. Given a complete local model, French text with grammar/accents/punctuation errors is corrected in French by the backend dispatch used by dictation, with the worker offline and without any server.
3. Given two curated IDs, the worker loads only complete known manifests and switches models without retaining both; unknown IDs, incomplete files, path traversal, bad protocol, and incompatible readiness fail closed.
4. Given a timeout, dropped request, unload, shutdown, or idle expiry, only the owned worker is stopped and reaped; a later request can start a fresh worker.
5. Given cloud selection, prompts, context, or an unsafe response, the existing cloud selection and common correction/output-safety contracts remain authoritative.
6. Given Intel macOS, Windows, or Linux, MLX is not offered or executable and native catalogue/dispatch remain compatible.
7. Existing managed STT tests and readiness signature remain unchanged.
8. Each managed checkpoint HTTP future races cancellation and a bounded deadline, including stalled response headers/body. Cancellation returns exactly `cancelled`, drops the request and cleans partial artifacts; retry remains possible without changing shared/native downloader behavior.
9. Before a checkpoint reaches its first worker load, validate every manifest hash under a bounded cancellable operation. Cache only verified file identities/metadata, revalidate changes, invalidate model readiness and remove corrupt files on mismatch. Preserve healthy runtime readiness and healthy checkpoints when cancelled.
10. The macOS 14 floor applies consistently to catalogue compatibility, readiness and execution, using a cached native OS-version observation and testable fail-closed policy. Catalogue membership is hardware-based: older Apple Silicon Macs still see managed models as unsupported/unavailable, without regaining excluded native alternatives. STT's hardware-only eligibility contract remains unchanged.
11. First-use preparation does not consume the caller's inference deadline. Bound worker-lock acquisition plus integrity preparation to 60 seconds, worker preparation/startup to 30 seconds, and the worker request separately to `PolishRequest::timeout` (60 seconds by default). Keep an additive outer safety deadline and unload/drop cancellation across all stages. The shared text-transform service must preserve those staged MLX deadlines instead of wrapping the entire operation in the short-text inference allowance; native and cloud service deadlines remain unchanged.

## Managed integrity and eligibility contract

Each checkpoint HTTP future has a 20-minute total deadline and races the existing cancel flag even while waiting for headers or the next body chunk. Dropping a failed/cancelled future removes partial download artifacts. Cancellation stays exactly `cancelled` for the existing backend event contract. Shared/native download behavior is unchanged.

Download-time hashes are not sufficient evidence for a later worker load. The engine hashes every manifest file on first use, caching only a manifest signature and file identities after successful verification. Size/mtime changes and, on Unix, device/inode/ctime changes force revalidation. A confirmed checksum mismatch removes model readiness and the mismatching file so the existing download operation can repair it. Cancellation, expiry, or checkpoint corruption never invalidates a healthy Python runtime. Cancelled/expired first-use verification does not delete healthy checkpoint files. Hashing checks cancellation/deadline between 64 KiB reads; dropping the awaiting future signals its blocking hash task to stop.

macOS eligibility uses a pure, fail-closed version policy and a once-cached native `NSProcessInfo` observation, not a subprocess on every status/catalogue query. Apple Silicon macOS 12/13, unknown/malformed versions, Intel macOS, Windows and Linux cannot download/run managed polish or report it available. Intel/Windows/Linux retain native alternatives; Apple Silicon hardware follows the MLX plus Apple-only policy regardless of OS version.

## Verification and limits

Failing-first Rust/Python contract tests precede implementation. Run targeted and broad Rust tests, clippy, fmt, frontend type/build and i18n checks without overlapping the desktop build's Cargo-clean hook with Cargo. Real small-model inference runs only under an isolated `/tmp` root, through production backend dispatch, with bounded waits and explicit unload. Native UI, permissions, production settings, app installation/launch, and release packaging are the parent's verification boundary. Do not mark complete without recording real inference evidence.

## Authoritative upstream evidence

Retrieved 2026-10-24 from primary sources (the web search transport returned empty responses; direct HTTPS retrieval was used):

- MLX-LM release metadata: https://pypi.org/pypi/mlx-lm/0.32.2/json
- MLX-LM generation implementation: https://github.com/ml-explore/mlx-lm/blob/main/mlx_lm/generate.py
- Curated 0.6B manifest: https://huggingface.co/api/models/mlx-community/Qwen3-0.6B-4bit?blobs=true — revision `73e3e38d981303bc594367cd910ea6eb48349da8`.
- Curated 1.7B manifest: https://huggingface.co/api/models/mlx-community/Qwen3-1.7B-4bit?blobs=true — revision `3b1b1768f8f8cf8351c712464f906e86c2b8269e`.

API and dependency choices must additionally be checked against the installed, pinned wheel before real inference is accepted.

## Verified implementation evidence — 2026-10-24

The pinned MLX-LM wheel SHA256 `5c0991248cfff5872503d74f466d2532dd8cb7fb3670cb9be7a06d7305dadc04` matched PyPI metadata. Its implementation confirmed `load`, `stream_generate`, `make_sampler(temp=0)`, chat-template `enable_thinking=False`, and final `finish_reason` values `stop`/`length`. The worker uses those pinned APIs, not assumptions about a moving upstream branch. Runtime readiness includes the complete lock plus Python version; changing dependencies invalidates readiness without altering STT.

Production download/installation ran only beneath `/tmp/voiceflow-mlx-polish-validation`. Both checkpoint manifests contained nine files, with every exact size and SHA256 verified: 0.6B totaled 351,383,618 bytes; 1.7B totaled 984,013,244 bytes. The worker used offline loading; no external server or TCP endpoint was involved.

The ignored Rust integration test uses `UnifiedPolishManager::polish` plus common transcript preparation and output acceptance. After review r3 exposed the enclosing service deadline, the test additionally applies `services::text_transform::await_local_polish` with the actual short-text inference allowance to every real request. This exercises the production service deadline seam, not an installed microphone session or the whole `AppState` entry point. It verifies both models, deterministic warm reuse, switching, stop/restart, and final unload. The fixed input was:

> bonjour je suis aller a paris hier et jai acheter trois pomme pour ma soeur

Observed accepted outputs:

| Model | Output | Quality limit |
|-------|--------|---------------|
| Qwen3 0.6B 4-bit | bonjour je suis aller a paris hier et j'ai acheté trois pommes pour ma soeur | Partial correction only: missed capitalization, `allé`, `à`, and `sœur`. |
| Qwen3 1.7B 4-bit | Bonjour, je suis allé à Paris hier et j'ai acheté trois pommes pour ma sœur. | Corrected this complete targeted sentence; broader corpus/template quality is not established. |

Both responses passed common output safety without fallback. An initial stricter assertion requiring complete correction from every checkpoint exposed the 0.6B limitation; the verification now requires actual targeted corrections from both and full targeted correction from at least one, rather than claiming equal quality. A single run on Mac16,6 / M4 Max / 128 GiB observed 0.6B total/load/inference 1021/271/103 ms and 1.7B 475/283/172 ms. These are diagnostic observations from one synthetic sample, not comparative benchmarks or UI ratings. The test writes reproducible raw evidence to `inference-evidence.json` under the isolated root.

Deterministic tests cover catalogue/platform/lock/readiness, file integrity, injected worker behavior, protocol mismatch, startup/request deadlines, dropped requests, active unload/restart, idle behavior, installer cancellation/drop/process-group cleanup, and bounded diagnostics. UI regressions cover backend availability and removal of irrelevant HTTP controls and invented MLX ratings/recommendations. Final commands/results are recorded in the [completed execution plan](../../../../plans/completed/2026-10-24-006-feat-mlx-polish-plan.md).

After independent-review integrity hardening, a real debug-build replay initially failed the 984,013,244-byte checkpoint's 30-second verification limit. The corrected, still-bounded execution stages passed the same offline manager-dispatch test in 111.89 seconds, including download-time rehashing, first-use hashing, warm reuse, switching, stop/restart and unload. The accepted outputs remained exactly those above. Latest first-request total/load/inference observations were 13,661/288/101 ms for 0.6B and 35,305/288/173 ms for 1.7B; total now includes first-use integrity, so these are not comparable to earlier warm-path timings and are not UI benchmarks. A slower machine/disk may still exhaust the explicit 60-second preparation limit; no cross-device performance claim is made. Source-level delayed-preparation regressions prove the caller's inference budget is preserved rather than spent hashing.

### Remaining verification boundary

Review r3 found that the shared service's old 20-second whole-operation wrapper still overrode the engine's separate preparation allowance. Two behavioral service-boundary tests reproduced this before the MLX exception was implemented; native/cloud timeouts remain unchanged. The strengthened real test then passed in 97.51 seconds with the same service deadline policy and actual 20-second short-text inference allowance on all first/warm/restarted requests. This closes the earlier manager-only evidence gap without claiming a live microphone or whole-`AppState` test. See the execution plan for red/green and raw-output logs.

Independent review closed all four findings. Final combined gates passed 1,090 Rust tests and 134 frontend tests, required clippy/global formatting, frontend build/typechecks/i18n, Python and distribution checks. The parent built, ad-hoc signed, verified, installed and launched only `Voice Flow Dev.app`; source/installed executable hashes and the ready/idle CLI were verified. The actual bundled bootstrap executed successfully. Development settings were byte-for-byte unchanged and the previous bundle preserved. Existing native/cloud selections remain intact during this additive rollout.

Installed-app UI is not visually accepted: the offscreen native window capture showed only an empty background and provided no usable interaction evidence. Cold MLX runtime installation through that signed app, live microphone/whole-`AppState` acceptance, Intel/Windows/Linux physical-host execution, broad French/multilingual/template evaluation, and production/notarized release validation remain unverified. See the completed plan for the installation receipt, explicit capture limit, evidence paths and remaining risks.
