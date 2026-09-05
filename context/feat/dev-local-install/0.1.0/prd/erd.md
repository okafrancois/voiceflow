# Local macOS development installation specification

## Version

- Feature: `dev-local-install`
- User-visible name: Voice Flow Dev local installation
- Version: `0.1.0`
- Status: Completed

## Problem statement

The existing macOS permission command builds and immediately opens an ad hoc
signed application. It also inherits the normal frontend build command, which
cleans Rust output and the generated E2E capability before building. Review
bundles created for one-off visual checks may have different identifiers or
invalid signatures, so they are unsuitable for persistent macOS privacy grants.

## Goal

Provide one repository command that builds a complete, entitled, ad hoc signed
`Voice Flow Dev.app` without launching it or changing application data. The
artifact must keep the stable development bundle identifier, use a development
URL scheme, disable updates, and pass code-signature verification before the
command succeeds.

## Non-goals

1. Do not install or launch the application automatically.
2. Do not create certificates, reset TCC, or change System Settings.
3. Do not modify Tauri capabilities, production application data, or development
   application data.
4. Do not promise that privacy grants survive an ad hoc signed rebuild. That
   requires a persistent certificate-backed signing identity.

## Acceptance criteria

1. `npm --prefix apps/desktop run tauri:build:mac:local-install` builds only the
   macOS application bundle and does not open it.
2. The build uses `Voice Flow Dev`, `com.voiceflow.voicetotext.dev`, and the
   inhouse icon.
3. The frontend build runs local `tsc` and `vite build` commands without the
   cleanup scripts.
4. The local install configuration disables the updater and registers
   `voiceflow-dev://` instead of the production `voiceflow://` scheme.
5. The final bundle contains the repository entitlements and passes
   `codesign --verify --deep --strict`.
6. The script prints the absolute application bundle path for the separate
   installation step.

The development scheme only prevents the installed development app from
claiming production links. The current developer-bridge parser accepts
`voiceflow://` only, so `voiceflow-dev://` links are not a supported automation
interface in this version.

## BDD scenario

Given a macOS checkout with project dependencies, Rust, and full Xcode installed
When the local installation build command completes
Then an unopened `Voice Flow Dev.app` exists under the debug bundle directory
And its bundle identifier is `com.voiceflow.voicetotext.dev`
And its signature and audio-input entitlement verify successfully
And no production or development settings have been read or written by the app

## Verification

- Node tests for argument handling, configuration composition, and package
  command wiring.
- A native debug application build without launch.
- `codesign`, `PlistBuddy`, and entitlement inspection of the final bundle.

## Completion evidence

- The focused Node suite passed all five tests, including build-only argument
  handling and local configuration checks.
- The native debug build completed without launching or installing the app.
- The final bundle passed strict deep signature verification, used bundle
  identifier `com.voiceflow.voicetotext.dev`, registered only
  `voiceflow-dev://`, and contained the audio-input entitlement.
