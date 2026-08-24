---
title: Voice Flow Product Opportunities B1-B5 and P1-P13
type: feat
status: completed
date: 2026-08-24
---

# Voice Flow Product Opportunities Implementation Plan

## Overview

Deliver all approved B1-B5 foundations and P1-P13 product capabilities as
backend-owned, testable vertical slices. Deferred D items remain out of scope.

## Problem Frame

Voice Flow already records, transcribes, polishes, and inserts speech, but its
retention promises, provider contracts, Windows delivery, context model,
profiles, history, file workflows, integrations, onboarding, and quality
feedback are incomplete or inconsistent. Product branding and non-Chinese UI
also contain legacy names and leaked CJK text.

## Scope Boundaries

### In scope

- B1-B5 and P1-P13 from the approved opportunity audit.
- Complete headless contracts, typed IPC, reactive UI, migrations, unit and E2E
  coverage, local builds, and native macOS smoke testing.
- Voice Flow branding and non-Chinese UI text cleanup.

### Out of scope

- Every opportunity labelled D in the audit.
- Claims of Windows native verification without a Windows runner.
- External network provider calls that require credentials not present locally.

## Implementation Units

1. Completed: privacy retention and orphan cleanup (B1-B2).
2. Completed: Windows delivery reliability and evidence matrix (B3).
3. Completed: Voice Flow branding and language hygiene (B4).
4. Completed: provider contract alignment and mock verification (B5).
5. Completed: structured context, application rules, voice actions, snippets, quick
   controls, and named profiles (P1-P6).
6. Completed: file transcription/export, history workbench, and translation mode (P7-P9).
7. Completed: diagnostics/onboarding, developer bridge, code-aware mode, and quality
   dashboard (P10-P13).
8. Completed: full verification, production build, native launch, and UI smoke tests.
9. Completed: documentation gardening and evidence capture.

Each feature unit creates or updates its versioned specification before its
first failing test. Units are integrated in vertical slices to keep the backend
usable without the frontend.

## System-Wide Impact

- Settings and migrations gain privacy, context, profile, rule, and mode data.
- Recording and history pipelines gain explicit delivery and lifecycle actions.
- New typed commands expose headless features to the UI and developer bridge.
- Desktop navigation and settings gain corresponding reactive surfaces.
- Package metadata, docs, and assets move to Voice Flow naming.

## Risks & Dependencies

- Windows native behavior can be compiled and unit-tested on macOS but needs a
  Windows host for final application compatibility evidence.
- Live cloud checks depend on user credentials and network access; deterministic
  contract tests are the completion gate when credentials are absent.
- Accessibility, microphone, and automation prompts may limit native smoke
  testing without explicit permission approval.
- Broad shared settings and IPC files require serialized integration across
  otherwise parallel work.

## Verification Evidence

### Backend and contracts

- `cargo test -q -- --test-threads=1`: 629 library tests passed and every
  non-ignored integration suite passed. Credential-dependent live provider
  checks remained explicitly ignored.
- A failing-first regression test proved orphan cleanup could delete an imported
  source inside the recordings directory; the cleanup query now protects both
  retained audio and history source paths, and both orphan tests pass.
- `cargo clippy --all-features -- -D warnings`: passed.
- `cargo fmt -- --check`: passed.
- Deterministic provider mock suites cover Volcengine `bigmodel_nostream`,
  Aliyun, ElevenLabs, Anthropic, and OpenAI contracts.

### Desktop, shared package, website, and language hygiene

- Full desktop Vitest run: 14 files, 85 tests passed.
- Desktop TypeScript no-emit check: passed.
- Shared package typecheck: passed.
- Desktop locale parity: all 10 locale catalogs passed.
- Desktop branding regression suite: 8 tests passed; the source scan found no
  legacy product-name leak. CJK text remains only in intentional language
  resources/tests, not neutral English/French interface copy.
- Desktop Vite production build: passed.
- Website production build: passed with 14 pages.
- Markdown link validation: passed before final documentation closure and was
  rerun after the plan/spec index updates.

### Native application and developer bridge

- Release-optimized app-only Tauri bundle passed and produced
  `apps/desktop/src-tauri/target/release/bundle/macos/Voice Flow Dev.app`.
- `codesign --verify --deep --strict`: passed.
- The bundle registers `voiceflow://`, contains both `voiceflow` and
  `voiceflow-cli`, and embeds the Apple Silicon STT runtime plus llama.cpp
  server and dynamic libraries.
- Bundled `llama-server --version`: 9568; the running application detected it,
  started it, and reached the local-runtime-ready state.
- The authenticated developer bridge bound to `127.0.0.1`; its endpoint file
  permissions were `0600`.
- Both bundled CLI entry points returned a successful live `status`; URL-scheme
  status routing passed; an unknown command exited non-zero; stdin code-aware
  formatting returned `cargo test --workspace`.
- Native Tauri E2E navigation passed across Dashboard, History, Dictionary,
  Polish Templates, Workflows, Quality, About, and Changelog.
- The release app and its local llama child process both terminated cleanly
  after the smoke test.

### Explicit limits

- Windows delivery was compiled and deterministically tested, but no native
  Windows host was available for application-level execution.
- Live cloud provider calls were not made because user credentials were not
  available; local protocol contract tests are the completion evidence.
- The macOS `.app` bundle is complete and runnable. The optional DMG packaging
  helper failed in the headless environment, so delivery evidence is for the
  signed application bundle rather than a disk image.
