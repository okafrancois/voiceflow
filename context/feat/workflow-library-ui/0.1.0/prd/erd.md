# Workflow library interface

Status: Completed

## Version

- Feature: `workflow-library-ui`
- Version: `0.1.0`

## Problem

Voice Flow already has backend-owned snippets, writing profiles, and application rules, but their only editor is the broad Advanced Workflows screen. Users must understand internal IDs and unrelated workflow settings to manage reusable phrases or assign writing behavior to an application.

## Outcome

Dedicated Snippets and Styles pages expose the existing typed workflow contracts as focused editors. Snippets present spoken triggers and replacement text. Styles present writing profiles and explicit application assignments. Advanced Workflows remains the place for context capture, output routing, and other low-level settings.

## Scope

### Snippets

- Load snippets through `workflowCommands.getSettings`.
- Search by spoken trigger or expansion text.
- Add a snippet without asking the user to create an internal ID.
- Edit the trigger and replacement text, enable or disable the snippet, and delete it with confirmation.
- Keep supported variables visible: `{{date}}`, `{{clipboard}}`, and `{{selection}}`.

### Styles

- Load profiles and application rules through `workflowCommands.getSettings`.
- Create a named profile with a writing preset and an optional unique shortcut. Profiles without shortcuts remain available to application rules.
- Edit a profile's name, polish template, and code-aware setting without changing its shortcut or output contract.
- Assign an application ID and optional window-title match to an existing profile.
- Enable, disable, edit, and delete an assignment.
- Keep profile creation and shortcut management in Advanced because the backend profile contract requires a unique shortcut.

### Dictionary aliases

- Let users edit the explicit "heard as" phrases attached to a manual dictionary term.
- Normalize whitespace and duplicates in the backend while preserving the term and frequency.

## Boundaries

- No new backend state, raw `invoke` calls, routing, sidebar, or locale edits. One typed command updates aliases on an existing manual dictionary entry.
- No automatic app categorization or app discovery.
- No cross-device sync.
- React does not resolve rules, expand snippets, or mutate settings optimistically as product truth. It submits complete records to backend commands and reloads the backend snapshot.

## Acceptance criteria

1. Each page has a loading state and reports load/save errors.
2. Snippet search filters existing records without changing backend state.
3. Creating a snippet derives a stable unique ID from its trigger and sends the full record to `upsertVoiceSnippet`.
4. Saving, toggling, and deleting a snippet use typed workflow commands and refresh from the backend afterward.
5. Styles show existing profiles and assignments without exposing profile IDs as the primary label.
6. Profile edits preserve hotkey, trigger, language, translation target, output action, protection state, and ID.
7. Application assignments reference an existing profile and use typed upsert/delete commands.
8. Component tests cover create/edit/delete paths, preserved profile fields, assignment changes, empty states, and errors.
9. Manual dictionary aliases can be replaced, including removal, without changing the canonical term or frequency.

## Verification

- Focused Vitest component suites for both pages.
- Desktop TypeScript check or production frontend build after integration with routes and locale keys.

Integrated verification: 110 frontend tests passed. Native French Snippets and Styles forms, optional-shortcut creation and labeled application selection were inspected. Alias and app-only shortcut registration behavior has focused Rust coverage.
