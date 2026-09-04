# Dictate polish template

## Version

1.0.0

## Status

Completed and verified on 2026-09-04.

## Problem

The default shortcut model splits recording into Dictate without polish and Riff with mandatory polish. Users can enable and configure a polish model while continuing to trigger Dictate, so the application silently skips polish. The distinction is not useful enough to justify two built-in shortcuts.

## Goal

Expose one built-in Dictate shortcut. Dictate owns an optional polish template:

- `None` records and inserts the transcription without LLM polish.
- `Some(template_id)` polishes with the selected built-in or custom template.

Riff is removed as a built-in profile and is not registered at runtime.

## Data and migration

`workflow_profiles` remains the canonical backend-owned profile store. The built-in default list contains only the protected `dictate` profile. User-created advanced profiles remain supported by the workflow subsystem.

When settings contain the former built-in `riff` profile:

1. Keep Dictate's hotkey and trigger mode.
2. If Dictate has no polish template, copy Riff's selected template to Dictate.
3. Remove Riff from `workflow_profiles`, so its hotkey is unregistered on the next launch.
4. Retarget application rules that referenced Riff to Dictate.
5. Keep the fixed legacy shortcut map only as a compatibility projection for older clients. It is not a runtime registration source.

## Acceptance criteria

1. The shortcut settings page shows one built-in profile: Dictate.
2. Dictate offers `No Polish` and every available built-in or custom template.
3. Saving a Dictate template persists it through the backend and the next recording resolves that template.
4. New installations register no Riff shortcut.
5. Existing installations migrate Riff's template to Dictate when Dictate had no template, remove Riff from canonical profiles, and retarget Riff application rules.
6. Existing user-created advanced workflow profiles are preserved.
7. The selected template instructions reach both cloud and local polish engines.
8. Backend tests, frontend tests, builds, type checks, formatting, clippy, and i18n validation pass.

## Out of scope

- Rewriting built-in polish template content or changing model selection.
- Fixing Paste Last Transcription.
- Removing the legacy fixed profile map from serialized settings. That compatibility cleanup requires a separate format migration.
