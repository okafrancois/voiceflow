---
title: Voice Flow brand cleanup
version: 0.1.0
status: completed
owner: desktop
---

# Voice Flow brand cleanup

## Problem

The repository still mixes the former product identity with Voice Flow in package names, application identifiers, build artifacts, runtime keys, documentation, links, and generated website files. A few source comments and English UI strings also contain Chinese text. This makes the installed application and its development tooling look like two different products, and it can expose Chinese text while another interface language is active.

## Product requirements

1. Every current product-facing name is `Voice Flow`.
2. Package and crate identifiers use `voiceflow`; JavaScript workspace packages use the `@voiceflow/*` scope.
3. Build artifacts, app bundles, temporary paths, logging names, local-storage keys, event names, environment variables, and protocol-owned fields use `voiceflow` or `VOICEFLOW`.
4. The repository contains no former product token in file contents or file names once generated and cached outputs are excluded.
5. Existing user preferences stored under former browser keys are read once and migrated to the new key. Existing application data is migrated to the new application data directory without overwriting newer data.
6. Chinese and Japanese locale files remain valid translations. Chinese language labels remain allowed inside the Chinese translation itself, but Chinese text must not be hard-coded into language-neutral React, Rust, shell, or English locale sources.
7. Links point to the current `okafrancois/voiceflow` repository. Links to a former custom domain are removed until a Voice Flow domain is configured.

## Engineering requirements

- The backend remains the owner of application paths and migration behavior.
- Compatibility code must describe old locations as `legacy` and may assemble the old token from neutral fragments so the retired brand is not exposed in current source or logs.
- Renaming must not change the public STT or polish behavior.
- The true `zh` and `ja` locale files are excluded from the hard-coded Han-character check.
- Generated website output must be rebuilt from the renamed source rather than edited by hand.

## Acceptance tests

- A repository branding test fails on the former token in a text file or file name.
- The test asserts Tauri product names and bundle identifiers.
- The test asserts package scopes, crate/bin names, current repository URLs, browser storage/event keys, and canonical logo file names.
- The test scans language-neutral UI and scripts for Han characters while excluding the Chinese and Japanese translation files.
- Theme and onboarding unit tests prove that legacy browser preferences migrate to the Voice Flow keys.
- Rust path tests prove the current default path and the non-destructive legacy-data migration.
- Desktop, website, shared-package, script, Rust, and i18n verification pass.

## Scope boundaries

In scope: product identity, internal identifiers owned by this repository, documentation, generated website output, assets, links, and safe local compatibility migrations.

Out of scope: registering a new public domain, changing provider behavior, changing retention/history behavior, editing Tauri capability permissions, or removing supported Chinese and Japanese localizations.

## Verification status

Completed on 2026-08-24. The repository brand scan, desktop typecheck and Vite build, website static build, i18n check, Markdown-link check, browser-theme migration tests, backend path-migration tests, local-runtime compatibility tests, streaming timing tests, local HTTP tests, and runtime-context normalization tests pass.
