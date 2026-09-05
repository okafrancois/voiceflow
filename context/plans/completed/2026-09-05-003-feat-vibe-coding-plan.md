---
title: "feat: Add opt-in Vibe coding context"
type: feat
status: completed
date: 2026-09-05
---

# Add opt-in Vibe coding context

## Overview

Turn the existing editor context bridge into an explicit coding mode that
recognizes supplied IDE and file metadata, protects identifiers, affects the
real audio pipeline, and reports its active state.

## Problem frame

The current code context is a small global value with four optional strings.
It has no mode setting or status model, performs no file/editor recognition,
and changes recording output only when a workflow profile separately enables
code-aware behavior.

## Scope boundaries

- **In scope**: backend context enrichment, opt-in setting, status commands,
  audio policy integration, CLI bridge fields, an explicit VS Code-compatible
  adapter, tests, and developer bridge docs.
- **Out of scope**: filesystem scanning, source-file reading, editor process
  inspection, automatic bridge enablement, automatic extension installation,
  locale strings, and capability changes.

## Implementation units

- [x] **Unit 1: Specify context recognition and activation**
  - Add failing tests for bounded identifiers, language/editor recognition,
    status, and the disabled/default state.
- [x] **Unit 2: Implement the backend policy**
  - Enrich and sanitize `CodeContext` using only supplied metadata.
  - Add persisted opt-in state and typed status commands.
- [x] **Unit 3: Integrate the recording pipeline**
  - Apply existing code-aware prompt and formatter behavior when the mode is
    enabled with active context.
  - Preserve existing workflow-profile behavior.
- [x] **Unit 4: Verify and document**
  - Extend CLI/editor bridge documentation and run focused backend verification.

## System-wide impact

The backend remains headless. The frontend receives typed state and renders it;
it does not infer languages, recognize editors, or decide whether context affects
dictation.

## Risks and dependencies

- Editor adapters must keep context current. The backend expires context after five minutes and scopes it to the recording target; the editor adapter clears context on focus loss.
- Context can reach configured cloud providers when the user also enabled cloud
  processing. The Vibe coding toggle does not change provider selection.
- Shared settings, command registration, and audio modules require coordination
  with concurrent UI integration.

## Verification evidence

- `cargo test services::vibe_coding::tests --lib`: 4 passed.
- Focused bridge, settings-default, and recording application-scope tests: all
  passed.
- `cargo clippy --lib --all-features -- -D warnings`: passed.
- Full Rust library suite: 672 tests passed twice under default parallel
  execution after the shared editor-context test writers were serialized.
- `npm test` in `extensions/vscode-vibe-coding`: 6 passed.
- VSIX archive validation passed; packaged manifest confirmed
  `voiceFlowVibe.cliPath` has application scope.
- The VS Code CLI accepted the VSIX using isolated user-data and extensions
  directories under `/private/tmp`.
- Live testing in a VS Code, Cursor, or Windsurf Extension Development Host was
  not performed in this session.

- Actual VS Code CLI installed the VSIX successfully into isolated temporary user-data and extension directories. This proves package acceptance, not live dictation accuracy.
