---
title: Wait for complete MLX worker fixture process IDs
type: fix
status: completed
date: 2026-10-24
---

## Overview

Eliminate a test-only readiness race reported by the full managed-polish verification, following the [testing contract](../../spec/testing.md).

## Problem frame

`cancelled_worker_startup_preserves_runtime_readiness` waits for `worker.pid` to exist, then immediately parses its contents. Shell output redirection creates the file before writing the PID, so a legitimate scheduling interleaving exposes an empty file and produces `ParseIntError::Empty` before cancellation is tested.

## Scope boundaries

Change only the STT test module and testing documentation. Preserve all production worker/runtime, cancellation, platform, and polish behavior. No application installation, settings, permissions, Git operations, or Cargo-clean builds.

## Implementation units

- [x] Reproduce missing/empty/incomplete process-ID publication deterministically.
- [x] Wait within a bounded interval for a complete PID record before using it.
- [x] Run focused cancellation/process tests and formatting, then release the polish verification hold.

## System-wide impact

The worker tests continue to exercise real owned subprocess cancellation. Only fixture synchronization changes; no product readiness signal is weakened and no test is ignored or removed.

## Risks and dependencies

Coordinate the active polish agent's full Cargo run. A syntactically valid PID prefix is still incomplete without its newline terminator. Unexpected filesystem errors and malformed complete records must fail explicitly rather than becoming false success.

## Verification evidence

The original full-suite failure is preserved at `/tmp/voiceflow-polish-cargo-stt-fixture-race.log`; the polish agent's isolated and full reruns passed without changing STT. Parent then reproduced the same `ParseIntError::Empty` deterministically by creating the marker before writing its contents: `/tmp/voiceflow-mlx-pid-fixture-red.log`.

- The test-only PID reader now waits for a newline-terminated record, with a bounded deadline. Missing/empty/unterminated contents stay pending; malformed complete contents, zero PID, and unexpected read failures are explicit errors. Production worker/runtime code is unchanged.
- Focused STT MLX engine module: **11 passed**, including real owned-process startup/cancellation/unload tests and new partial-publication/error regressions; `/tmp/voiceflow-mlx-pid-fixture-green.log`.
- The actual cancelled-startup regression passed **20 consecutive repetitions** using the compiled test executable; `/tmp/voiceflow-mlx-pid-fixture-repeat.log`.
- Combined `cargo test` after polish integration and fixture hardening: **1,071 passed, 44 ignored** across 32 standard test harness results; `/tmp/voiceflow-managed-mlx-polish-combined-test.log`.
- Required `cargo clippy --all-features -- -D warnings`, global `cargo fmt -- --check`, and scoped `rustfmt --edition 2021 --check .../mlx/engine.rs`: passed. Logs: `/tmp/voiceflow-managed-mlx-polish-required-clippy.log`, `/tmp/voiceflow-managed-mlx-polish-combined-fmt.log`, and `/tmp/voiceflow-mlx-pid-fixture-fmt.log`.
- An additional, stricter `cargo clippy --all-features --tests -- -D warnings` audit fails on existing test-only lints outside this change (for example `tests/downloader_test.rs`, `tests/beep_test.rs`, and `src/stt_engine/mod.rs`). Those unrelated tests were not modified or suppressed; `/tmp/voiceflow-managed-mlx-polish-combined-clippy.log`. Do not describe this extra audit as green.

## Extracted invariant

File creation and payload publication are different synchronization events. Process-lifecycle tests must consume a complete PID record rather than assume existence means readiness, while preserving explicit failure for invalid fixture data.
