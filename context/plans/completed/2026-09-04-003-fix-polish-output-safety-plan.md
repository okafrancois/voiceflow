---
title: Polish output safety
type: fix
status: completed
date: 2026-09-04
---

# Polish output safety

## Overview

Stop local Polish models from replacing French dictation with copied English examples or heavily shortened text, then publish a corrective desktop release.

## Problem frame

Version 1.1.4 began passing the selected built-in template to local models. Gemma 2B IT copied English examples from those templates and the backend accepted the result. The request language was stored on `PolishRequest` but was not added to the local or cloud prompt.

## Scope boundaries

In scope: built-in prompt content, explicit source-language instructions, backend output validation, regression tests, documentation, and a patch release.

Out of scope: STT changes, automatic model replacement, and unrelated local worktree changes.

## Implementation units

1. Added failing tests for language mismatch, destructive shortening, valid cleanup, concise output, and prompt language instructions.
2. Replaced example-heavy templates with shorter instruction-only templates.
3. Added the source-language rule to every Polish request.
4. Reject unsafe output before persistence or insertion and fall back to the raw transcript.
5. Disabled direct insertion of unvalidated streamed Polish text.

## System-wide impact

The Rust backend remains the policy owner. Both local and cloud engines return through the same acceptance function, so the frontend contains no output-safety business logic.

## Risks and dependencies

- Length checks tolerate more aggressive shortening only for the Concise template.
- French/English detection requires enough evidence to avoid rejecting mixed technical text.
- Explicit translation workflows remain exempt from the language-change rejection.

## Verification evidence

- `cargo test`: passed, including 655 library tests and all integration suites; network-backed live-provider tests remain intentionally ignored.
- `cargo clippy --all-features -- -D warnings`: passed.
- `cargo fmt -- --check`: passed.
- `pnpm --filter @voiceflow/desktop build`: passed.
- `pnpm --filter @voiceflow/shared typecheck`: passed.
- `pnpm check:i18n`: passed.
