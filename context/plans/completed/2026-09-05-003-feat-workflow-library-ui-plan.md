---
title: Workflow library interface
type: feat
status: completed
date: 2026-09-05
---

## Overview

Create focused Snippets and Styles editors over the existing workflow backend. This implements the bounded UI slice from the approved Wispr interface selection without changing backend contracts.

## Problem frame

The backend already owns snippets, profiles, and app rules. The current Advanced page exposes all of them in one technical editor. Dedicated pages should make the common tasks direct and leave advanced controls intact.

## Scope boundaries

In scope: two page components, component tests, reuse of current workflow IPC types and commands, and one bounded Dictionary alias-update command.

Out of scope: routes, navigation, locale files, automatic application detection, new style semantics, and dictionary synchronization.

## Implementation units

1. Add failing tests for snippet list, search, create, edit, toggle, delete, and backend errors.
2. Implement the dedicated Snippets page with row-local mutation results so saving one row preserves other drafts.
3. Add failing tests for profile creation/editing and application assignment CRUD.
4. Implement the dedicated Styles page while preserving hidden profile fields.
5. Run focused tests and TypeScript verification. Record route and locale integration needs for the parent change.
6. Add a failing backend test and typed IPC command for replacing aliases on an existing manual dictionary term, then expose it in the current Dictionary page.

## System-wide impact

The new components are inert until the parent change registers routes and navigation. Existing Advanced Workflows behavior and serialized settings stay unchanged.

## Risks and dependencies

- Profile style and shortcut settings share one backend record. The focused editor must preserve fields it does not expose.
- The protected default profile requires a shortcut. App-only styles may omit one and must not be globally registered.
- The backend only accepts application identifiers entered as text. App discovery needs a separate command.
- A profile without a writing preset bypasses polish only when translation and code-aware instructions are also absent.
- Locale keys need all ten translations before the application i18n check can pass.
- Alias replacement changes the serialized custom dictionary string, so the backend must normalize the same way as CSV import and preserve unrelated entries.

## Verification evidence

- Initial component tests failed because the dedicated pages did not exist.
- Initial Rust alias tests failed because no alias replacement helper existed.
- `apps/desktop/node_modules/.bin/vitest run src/components/Home/__tests__/DictionaryPage.test.tsx src/components/Home/__tests__/SnippetsPage.test.tsx src/components/Home/__tests__/StylesPage.test.tsx` from `apps/desktop`: 10 passed.
- `cargo test --lib correction_learning::commands::tests:: -- --test-threads=1`: 4 passed.
- `cargo test --lib services::product_workflows::tests::`: 21 passed, including app-only profile resolution and shortcut registration rollback.
- `node apps/desktop/node_modules/typescript/bin/tsc --noEmit -p apps/desktop/tsconfig.json`: passed.
- `/private/tmp/workflow-library-locales.json`: parsed successfully as JSON. It contains the required English and French strings for parent integration.

Integrated completion: 110 frontend tests and 923 Rust tests passed. Direct routes, English/French text, common macOS app selection and native form inspection are complete. Nonprotected styles may omit global shortcuts; default protected profile validation, unregistration transitions and rollback are tested.
