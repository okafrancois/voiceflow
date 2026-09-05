---
title: "feat: Add a reproducible local macOS development installation build"
type: feat
status: completed
date: 2026-09-05
---

# Add a reproducible local macOS development installation build

## Overview

Build a complete `Voice Flow Dev.app` for local installation and macOS privacy
testing without launching the app or invoking cleanup commands.

## Problem frame

The current permission helper always opens its artifact and inherits a frontend
build that cleans unrelated output. One-off review applications also use
temporary identities and cannot serve as the durable development application.
The repository needs a build-only command with a stable bundle identifier and
an independently verifiable final signature.

## Scope boundaries

- **In scope**: build-only script mode, local-install Tauri configuration,
  package command, focused tests, contributor documentation, and bundle
  verification.
- **Out of scope**: installation outside the repository, app launch, TCC changes,
  certificate creation, application data migration, production signing, and
  capability edits.

## Implementation units

- [x] **Unit 1: Specify the build-only contract**
  - Add failing tests for the package command, local configuration, and
    `--no-open` behavior.
- [x] **Unit 2: Implement the local installation build**
  - Add a non-cleaning frontend build command and isolated URL scheme.
  - Extend the existing permission helper with build-only argument handling and
    final bundle verification.
- [x] **Unit 3: Document and verify the workflow**
  - Document the build, install, verify, and launch commands.
  - Run focused Node tests and build the native application without opening it.

## System-wide impact

Normal development, release, updater, and E2E commands keep their current
behavior. The new configuration applies only when the local-install command is
used.

## Risks and dependencies

- Ad hoc signing identifies a build by its code hash. Rebuilding can require new
  privacy grants even though the bundle identifier and installation path stay
  the same.
- Full Xcode is required because the bundled native dependencies use Apple build
  tools.
- The local app and production app should not run at the same time because both
  can register global shortcuts.

## Verification evidence

- Failing-first check: `node --test scripts/inhouse-unsigned-build.test.mjs`
  failed because the script did not export build-only argument handling.
- Focused checks: `node --test scripts/inhouse-unsigned-build.test.mjs
  scripts/tauri-macos-signing-config.test.mjs` passed 6 tests.
- Syntax check: `node --check scripts/run-macos-permission-dev.mjs` passed.
- Native build: `npm --prefix apps/desktop run
  tauri:build:mac:local-install` completed without opening or installing the app.
- The first native build exposed that the base Info plist retained
  `voiceflow://`. A failing test led to a dedicated local-install plist. The
  rebuilt app registered `voiceflow-dev://`.
- `codesign --verify --deep --strict --verbose=2` reported the final bundle valid
  on disk and satisfying its designated requirement.
- Plist and entitlement inspection confirmed `com.voiceflow.voicetotext.dev`,
  `voiceflow-dev://`, and `com.apple.security.device.audio-input=true`.
- Metadata verification parses plist values, requires the audio-input value to
  be Boolean `true`, and rejects any extra URL scheme including production
  `voiceflow://`.
