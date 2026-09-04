---
title: Dictate polish template
type: feat
status: completed
date: 2026-09-04
---

## Overview

Replace the built-in Dictate/Riff split with one Dictate shortcut whose polish template is optional.

## Problem frame

Dictate always skipped polish while Riff required it. Users could configure polish and unknowingly bypass it. Dictate now exposes `No Polish` and every available template.

## Scope boundaries

Completed: Dictate template selection, removal of built-in Riff from the UI and canonical defaults, migration of existing Riff settings, local template propagation, runtime registration behavior, tests, translations, and documentation.

Deferred: rewriting template content, final punctuation policy, Paste Last crash, and removal of the serialized legacy compatibility map.

## Implementation units

1. Added failing backend tests for one-profile defaults and Riff migration.
2. Added a failing frontend test for the single Dictate card and optional template selector.
3. Updated backend defaults, migration, validation, and legacy projection.
4. Simplified the shortcut settings UI and updated translations.
5. Passed selected template instructions to the local polish runtime.
6. Ran focused and repository-wide verification.

## System-wide impact

- Existing Riff hotkeys stop registering after settings migration.
- Dictate inherits the former Riff template when it previously had no template.
- Application rules targeting Riff migrate to Dictate.
- Advanced user-created workflow profiles remain intact.

## Verification evidence

- `cargo test`: 646 passed, 0 failed; live-provider tests remained ignored as designed.
- `cargo test commands::hotkey::tests::`: 2 passed, 0 failed.
- `cargo clippy --all-features -- -D warnings`: passed.
- `cargo fmt -- --check`: passed.
- `npm --prefix apps/desktop test`: 89 passed, 0 failed.
- `npm --prefix apps/desktop run build`: passed.
- `npm --prefix packages/shared run typecheck`: passed.
- `npm run check:i18n`: passed for all locales.
- `npm run check:md-links`: passed.
