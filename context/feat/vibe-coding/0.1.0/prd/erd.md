# Vibe coding context specification

## Version

- Feature: `vibe-coding`
- User-visible name: Vibe coding
- Version: `0.1.0`
- Status: Completed

## Problem statement

Voice Flow accepts manually supplied editor context, but it does not identify a
known editor, infer the language from a file path, protect editor-supplied code
identifiers, or report whether coding context will affect the next dictation.
Code-aware processing is tied to individual workflow profiles, which makes the
feature hard to understand and easy to misconfigure.

## Goal

Add an explicit Vibe coding mode that enriches dictation with bounded context
provided by the existing local editor bridge. When enabled, the backend uses
the active file path, language, symbol, editor identifier, workspace label, and
identifier hints in STT and polish prompts and preserves explicit spoken code
punctuation in the final text. A status query explains exactly what context is
active.

## Non-goals

1. Do not scan workspaces, read source files, inspect arbitrary processes, or
   capture clipboard or screen content.
2. Do not start or enable the developer bridge automatically.
3. Do not send editor context to a cloud service unless the user separately
   enabled the relevant cloud transcription or polish provider.
4. Do not install an editor extension automatically. The repository-owned
   adapter is packaged for an explicit local VSIX installation.
5. Do not execute generated code or editor commands.

## Data contract

```rust
struct CodeContext {
    language: Option<String>,
    file_path: Option<String>,
    symbol: Option<String>,
    editor_id: Option<String>,
    workspace: Option<String>,
    identifiers: Vec<String>,
}

struct VibeCodingStatus {
    enabled: bool,
    context_active: bool,
    language: Option<String>,
    file_path: Option<String>,
    file_name: Option<String>,
    workspace: Option<String>,
    editor: Option<String>,
    identifiers: Vec<String>,
    updated_at_ms: Option<i64>,
    expires_at_ms: Option<i64>,
}
```

## Recognition rules

- Infer a language only when the editor did not provide one and the file has a
  known extension.
- Recognize VS Code, Cursor, Zed, JetBrains IDEs, and Xcode from their supplied
  editor identifiers. Preserve an unknown identifier without guessing a brand.
- Protect at most 64 unique identifiers. Each identifier is trimmed, stripped
  of control characters, and limited to 128 characters.
- Add the current symbol and code-shaped file stem tokens to explicit identifier
  hints. Never open the file to discover more names.
- An empty context is not active and does not alter dictation.

## Acceptance criteria

1. Vibe coding is disabled by default and persists only after an explicit
   settings update.
2. Disabling it clears the active code context.
3. Status distinguishes disabled, enabled without context, and enabled with
   recognized context.
4. The backend infers common language identifiers from file extensions and
   recognizes common editor families from supplied identifiers.
5. Identifier hints are bounded, deduplicated, and included in the code-aware
   instruction.
6. The normal recording pipeline uses code-aware prompting and final formatting
   when Vibe coding is enabled with active context. Existing code-aware profiles
   continue to work.
7. The authenticated CLI bridge accepts the extended context fields without
   gaining filesystem or external process access.
8. The editor adapter publishes only while explicitly enabled and focused,
   invalidates pending symbol requests after focus or file changes, and clears
   context when focus leaves the editor.

## BDD scenarios

### Recognize editor context

Given Vibe coding is enabled
And the editor bridge supplies a TypeScript file path, Cursor identifier, symbol,
and identifier hints
When Voice Flow sanitizes the context
Then status reports TypeScript, Cursor, the file name, and bounded identifier
hints without reading the source file

### Keep the feature opt-in

Given Vibe coding is disabled
When a normal dictation finishes
Then editor context does not change the STT prompt or final formatting

### Preserve coding tokens

Given Vibe coding is enabled with active Rust context
When the transcript contains explicit spoken command punctuation
Then the backend applies the existing code formatter before delivery
And code identifiers from the editor context appear in the preservation
instruction

## Verification

- Unit tests for defaults, language inference, editor recognition, empty-state
  status, identifier sanitization, and prompt construction.
- Audio policy tests proving the mode activates only when enabled with context.
- Settings serialization and update tests.
- Rust formatting, focused tests, and clippy for changed backend modules.
- Node unit and fake-host lifecycle tests for the editor adapter.
- VSIX archive integrity and packaged-manifest inspection.

## Verification result

Completed on 2026-09-05. Focused Rust tests passed for context policy, bridge
parsing, settings defaults, and recording-time application scoping. Rust Clippy
passed with warnings denied. After serializing the two tests that mutate the
shared editor-context store, the 672-test Rust library suite passed twice under
default parallel execution. The editor adapter passed six payload and lifecycle
tests. Its packaged VSIX passed archive validation, contains an
application-scoped CLI path setting, and was accepted by the VS Code CLI in an
isolated user-data and extensions directory. A live Extension Development Host
smoke test was not run in this implementation session.
