---
title: Voice Flow brand cleanup
type: feature
status: completed
date: 2026-08-24
---

# Voice Flow brand cleanup

## Overview

Apply one product identity across the desktop app, website, workspace packages, build tooling, runtime-owned identifiers, and documentation. Preserve local user data through explicit migration code and remove Chinese text that is hard-coded outside supported locale files.

## Problem frame

The installed application already displays Voice Flow in several places, but source identifiers, paths, artifacts, links, package scopes, generated files, and prose still use the retired product token. Some language-neutral UI sources also contain Chinese comments or examples.

The desired state is measurable: the branding scan has no former-token hits, current package and bundle identifiers are coherent, migrations are tested, and language-neutral UI sources contain no Han characters.

## Scope boundaries

In scope: the files and behavior listed in the [feature specification](../../feat/voice-flow-brand-cleanup/0.1.0/prd/erd.md).

Out of scope: provider contracts, retention/history behavior, capability permission changes, and registering a public domain.

## Implementation units

1. Strengthen `scripts/desktop-branding.test.mjs` to scan current source identifiers, files, and language-neutral UI text. Run it red.
2. Rename workspace packages, Rust crate/bin identifiers, build scripts, artifacts, repository links, and assets. Update their focused tests.
3. Rename runtime-owned keys and paths. Add non-destructive compatibility migrations for browser preferences and application data.
4. Translate hard-coded Chinese comments and English-locale examples. Keep the Chinese and Japanese locale files intact.
5. Rebuild generated website output and run all branding, script, i18n, frontend, and Rust checks.
6. Record verification evidence and move this plan to `completed/`.

## System-wide impact

- Workspace package imports and lockfile package snapshots change together.
- Rust integration tests import the renamed library crate.
- Bundle and release artifact names change to Voice Flow.
- Local preferences and application data retain compatibility through migration.
- Existing release URLs move to the current repository.

## Risks and dependencies

- External automation may still export the former environment variable prefix. Compatibility reads are needed during the transition.
- A custom Voice Flow domain has not been provided, so source links use GitHub or relative paths.
- Generated website output can only be refreshed after the website source builds.
- Shared backend, provider, and retention files may be under concurrent edit; changes there require coordination.

## Verification evidence

- `node --test scripts/desktop-branding.test.mjs`: 6/6 passed. The scan covers source contents, file names, package/crate identifiers, Tauri identity, logo identity, and language-neutral Han text.
- `npm run build:website`: passed; Next.js generated 14 static pages and `scripts/sync-website-export.mjs` refreshed `docs/`.
- `apps/desktop/node_modules/.bin/tsc --noEmit --project tsconfig.json`: passed.
- `apps/desktop/node_modules/.bin/vite build`: passed (5,690 modules transformed); Vite reported only its existing large-chunk and stale Browserslist warnings.
- `node scripts/check-i18n.mjs`: passed for all desktop locales and the website Chinese locale.
- `node scripts/check-md-links.mjs`: passed.
- `npm test -- --run src/lib/__tests__/theme.test.ts src/components/Pill/__tests__/PillWindow.test.tsx`: 8/8 passed.
- Focused release/build script suites passed after package, repository, artifact, and environment-prefix renames. The combined script run also exposed an unrelated desktop-CI policy assertion changed by B3 and a runtime smoke test that cannot bind localhost inside the sandbox; neither failure comes from the branding changes.
- `cargo test utils::paths --lib`: 2/2 passed, including non-destructive legacy-directory migration.
- `cargo test polish_engine::streaming --lib`: 5/5 passed, including current and legacy timing metadata.
- `cargo test derives_the_legacy_environment_variable_name --lib`: passed.
- `cargo test polish_engine::local_http --lib`: 9/9 passed with localhost permission; the first sandboxed attempt failed only because WireMock could not bind a port.
- `cargo test runtime_context::window --lib`: 11/11 passed. The normalizer now emits and deduplicates the multi-word `Voice Flow` product term, including OCR-glued variants.

The authorized capability change was limited to the human-readable `description` in `capabilities/default.json`. No capability identifier, window list, or permission changed. `gen/schemas/capabilities.json` was aligned with that description.

The former custom-domain `CNAME` files were removed from the root, context, website public files, and generated `docs/` output. No replacement domain was invented; current links use the configured `okafrancois/voiceflow` repository.
