# Context Workflows Specification

## Version

- Feature: `context-workflows`
- User-visible name: Context, Actions, Snippets, Controls, and Profiles
- Version: `0.1.0`
- Status: Completed
- Opportunity trace: P1-P6

## Problem Statement

Voice Flow can apply one polish template to a recording, but it cannot reliably
describe the focused application and field, route behavior by application,
transform selected text, expand voice snippets, recover the latest delivery, or
create more than one custom profile. Current OCR context is expensive and lacks
structured provenance.

## Goal

The headless backend captures explicit, consented context and resolves every
recording through deterministic application rules and an unlimited named
profile. Users can run selected-text actions, snippets, and quick recovery
controls without losing the source or clipboard content.

## Non-Goals

1. OCR is an explicit fallback, never an invisible default for selected text.
2. Do not execute deferred autonomous computer-use actions.
3. Do not put routing, prompt selection, or recovery state in React.

## First-Principles Model

1. Context is typed data with source and timestamp, not an unlabelled prompt.
2. More sensitive context requires a narrower opt-in and shorter lifetime.
3. Application rules choose a profile; profiles describe behavior.
4. Selected text is never overwritten before a preview or reversible snapshot.
5. Snippet expansion is deterministic before any model is called.
6. Recovery controls operate on a backend-owned last-delivery journal.

## Information Architecture

- Context settings: application identity/title, focused field, selection,
  clipboard, and explicit OCR fallback toggles.
- Application rules: ordered matchers for bundle/process identifier and title,
  each selecting a profile.
- Profiles: unlimited named records with shortcut, trigger, language, template,
  translation target, output action, and code-aware mode.
- Voice actions: shorten, translate, reply, list, plus custom template; always
  preview or explicitly replace.
- Snippets: named spoken triggers with static text and supported variables
  `date`, `clipboard`, and `selection`.
- Quick controls: undo last insertion, reinsert/copy raw or final text,
  re-polish, submit Enter, and cancel the active task.

## Data Contract

```rust
struct CapturedContext {
    application_id: Option<String>,
    application_name: Option<String>,
    window_title: Option<String>,
    focused_field_role: Option<String>,
    selected_text: Option<String>,
    clipboard_text: Option<String>,
    ocr_text: Option<String>,
    sources: Vec<ContextSource>,
    captured_at_ms: i64,
}

struct WorkflowProfile {
    id: String,
    name: String,
    hotkey: String,
    trigger_mode: ShortcutTriggerMode,
    language: Option<String>,
    polish_template_id: Option<String>,
    translation_target: Option<String>,
    output_action: OutputAction,
    code_aware: bool,
}

struct ApplicationRule {
    id: String,
    application_id: String,
    title_contains: Option<String>,
    profile_id: String,
    enabled: bool,
}

enum OutputAction { Insert, Preview, Copy }
enum VoiceActionKind { Shorten, Translate, Reply, List, Custom }
```

Persisted settings are versioned and migrate the existing Dictate, Riff, and
optional custom map into a profile list without changing their hotkeys.

## Acceptance Criteria

1. P1: backend context exposes application identity/name, title, field role,
   selection, optional clipboard, and explicitly enabled OCR fallback with
   provenance and bounded lengths.
2. P2: ordered enabled application rules resolve to an existing profile;
   invalid profile references fall back to the requested/default profile.
3. P3: selected-text actions produce a preview containing source and result;
   replacement journals the source for undo.
4. P4: static snippets and `date`, `clipboard`, `selection` variables expand in
   the backend; missing required context returns a clear error.
5. P5: quick controls cover undo, raw/final copy or reinsertion, re-polish,
   submit Enter, and task cancellation with explicit unavailable states.
6. P6: users can create, rename, update, and delete unlimited profiles while a
   protected default remains; duplicate IDs/hotkeys are rejected.
7. Settings, rules, profiles, snippets, previews, and the last-delivery journal
   are accessible through typed commands without a frontend process.
8. Sensitive clipboard and selection values are not written to logs.

## BDD Scenarios

### Resolve a profile for an editor

Given an enabled rule matching an editor bundle identifier
And that rule references the “Code” profile
When recording starts in the editor
Then the backend snapshots the “Code” profile into the session.

### Preview a selected-text action

Given a focused field with selected source text
When the user requests “shorten” with preview output
Then the source remains unchanged
And the backend returns a preview with source, transformed text, and action.

### Expand a voice snippet

Given a snippet `meeting-note` containing `{{date}}` and `{{selection}}`
When the spoken trigger matches and selection is available
Then the backend expands both variables before delivery.

### Undo the last insertion

Given the latest successful insertion journal contains the delivered text
When undo is requested
Then the backend sends the platform undo shortcut once
And marks that journal entry undone so a second request is rejected.

### Preserve migrated profiles

Given legacy Dictate, Riff, and custom settings
When settings migrate
Then all three become named profiles with the same hotkeys, triggers, and
template assignments.

## Verification

- Pure resolver, migration, snippet, action, and journal unit tests.
- Platform adapter tests for context and quick-control key sequences.
- Typed IPC tests and frontend component tests.
- Black-box desktop E2E for profile CRUD, rule routing, snippet expansion, and
  preview/replace behavior where the automation harness can observe it.

## Completion Evidence

- Context privacy, application rules, selected-text actions, snippets, quick
  controls, profile migration, atomic shortcut updates, and recording-pipeline
  resolution are implemented in the backend and exposed through typed IPC.
- Focused workflow Rust suites passed, including 20 service/command tests; the
  workflow UI component suite passed 4 tests.
- The native Tauri navigation smoke test opened the Workflows page and verified
  its profile controls.
- Desktop type checking, locale parity across 10 locales, the full Rust suite,
  Clippy, rustfmt, and the production desktop build passed.
