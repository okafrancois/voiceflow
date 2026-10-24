---
title: MLX and Apple engines on Apple Silicon
version: 0.1.0
status: completed
date: 2026-10-24
---

# MLX and Apple engines on Apple Silicon

## User-visible contract

On Apple Silicon Macs, local transcription offers only the curated managed MLX models and built-in Apple Speech. Local polish offers only curated managed MLX models and built-in Apple Intelligence. Existing downloadable native ONNX/GGUF alternatives are not offered on this hardware. Intel Macs and Windows/Linux retain compatible existing engines. Explicit vendor-cloud providers are unaffected.

## Backend authority

- Apply the hardware policy consistently to catalogues, recommendations, availability, selection validation, download, load/preload and inference entry points; frontend filtering alone is insufficient.
- Preserve global legacy model lookup for migration/diagnostics/deletion, but never silently execute an excluded engine. Keep cached legacy files; do not delete user downloads or start a runtime just to list models.
- Preserve valid MLX/Apple settings. On Apple Silicon normalize a legacy native local polish selection to the managed MLX 1.7B default; require download normally, without silent inference fallback or unsolicited installation. Do not rewrite unrelated vendor-cloud routing/credentials/templates/language. Reject newly submitted excluded local model IDs before mutating settings.
- Existing STT legacy migration retains its managed MLX default, but preserves explicitly selected built-in Apple Speech. Apple Speech is distinct from Apple Intelligence; do not label speech recognition as text polish.
- Unknown model diagnostics on non-ARM platforms remain unchanged. Built-in Apple readiness still depends on its real platform/provider availability, not fabricated downloads or capability changes. MLX retains its macOS 14 runtime requirement.
- Hardware determines catalogue membership, not runtime installation eligibility: older Apple Silicon Macs still see MLX polish entries with unsupported compatibility and unavailable readiness. They cannot install/run them until the OS requirement is met.
- Initial STT catalogue IPC waits asynchronously for the actual Apple Speech status before returning, with a bounded deadline and no main-thread bridge query. A pending status must not permanently hide an available Apple option. Direct model-selection IPC rejects Apple Speech unless its current-language status is ready, before changing the selected model/engine or disabling cloud STT.
- Catalogue metadata explicitly permits Apple Speech asset installation when assets are missing; the model UI routes that action through the existing backend download command. Unsupported locales do not offer installation. Reactive catalogue rendering ignores superseded responses, including reversed response order after language changes.
- Identify managed polish model names with `· MLX`; no repetitive explanation beneath every model, invented speed/accuracy or performance ranking.

## Acceptance

Failing-first tests prove exact allowed engine sets for both local catalogues on Apple Silicon, rejection through backend operations/settings, idempotent migration preserving valid MLX/Apple/cloud settings, continued native policy off ARM, preserved downloaded legacy files, and Apple Speech selectable without native fallback. Run actual host gates; do not claim physical Intel/Windows execution from pure policy tests.

## Delivery evidence

Completed on 2026-10-24 with backend policy, migration, readiness, asset-install action and reversed-response regressions. Final combined gates passed 1,137 Rust tests and 160 frontend tests; independent R5 review reports no findings in the last availability/UI corrections. The signed DEV bundle was installed and launched while preserving existing MLX selections and the complete settings file. Physical non-ARM execution and native installed UI interaction are not claimed. Exact commands, review history and delivery receipt: [completed execution plan](../../../../plans/completed/2026-10-24-010-feat-mlx-live-transcription-plan.md).
