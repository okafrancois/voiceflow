# Privacy retention controls 0.1.0

Status: completed

## Problem

Voice Flow writes every captured recording to a WAV file before it knows whether the user wants audio retained. The successful-transcription path then deletes that file but leaves its path in SQLite. Startup removes history rows older than 90 days without removing the WAV files referenced by those rows. Users cannot choose or verify how long text and audio remain on disk.

## Goal

Give users independent, backend-enforced retention policies for transcription text and captured audio. The backend must apply the selected policy during recording finalization, at startup, and immediately after a retention setting changes.

## First-principles model

- Text and audio are separate data classes. Deleting one must not implicitly delete the other during scheduled retention cleanup.
- A WAV file is retained only when a durable database record tracks it.
- A database record must not claim that an audio file exists after that file has been deleted.
- If filesystem deletion fails, the backend keeps the database reference and retries cleanup later. It must not create an untracked orphan.
- Explicit deletion of an entry or all history deletes both the selected text record and its attached audio.

## Information architecture

The Privacy card exposes two selectors:

1. Text history retention
2. Audio recording retention

Both selectors offer: Never, 7 days, 30 days, 90 days, and Forever. The UI explains that successful audio is deleted by default and that settings apply only to local files on this device.

## Data contract

```text
RetentionPolicy = never | days_7 | days_30 | days_90 | forever

AppSettings.text_retention: RetentionPolicy
AppSettings.audio_retention: RetentionPolicy
```

Defaults:

- `text_retention = days_90`, preserving the previous effective history duration.
- `audio_retention = never`, preserving the privacy-first successful-recording behavior without dangling paths.

SQLite schema version 3 adds an audio asset registry:

```sql
retained_audio(path TEXT PRIMARY KEY, created_at INTEGER NOT NULL)
```

The registry owns audio retention independently from transcription rows. A history row may reference a registered asset. An asset can remain registered after its text row expires when the audio policy is longer.

## Acceptance criteria

- Settings deserialize old files with the documented defaults and serialize both fields.
- The backend rejects unknown retention values through typed deserialization.
- A successful transcription with `audio_retention = never` deletes its WAV and stores no `audio_path`.
- A successful transcription with retained audio keeps the WAV and registers it.
- `text_retention = never` stores no dictated text. If audio is retained, only the audio registry remains.
- Failed recordings obey audio retention. A failure entry never points to an audio file that policy deleted.
- Startup uses both configured policies; no hard-coded 90-day cleanup remains.
- Text cleanup does not delete audio allowed by its independent policy.
- Audio cleanup deletes the file first, then clears history references and its registry row in one database transaction.
- A failed file deletion preserves the registry row and history reference for a later retry.
- Existing v2 audio paths migrate into the registry.
- Startup removes untracked WAV files from the application recordings directory.
- Changing either retention setting runs cleanup immediately after the new settings are persisted.
- The Privacy card exposes both selectors in every shipped locale.

## BDD scenarios

### Default successful dictation

Given default settings
When a successful dictation is finalized with a WAV file
Then the final text is stored for 90 days
And the WAV file is deleted
And the history row has no audio path

### Independent audio retention

Given text retention is Never and audio retention is 30 days
When a successful dictation is finalized
Then no raw or final dictated text is stored
And the WAV remains registered for audio cleanup

### Scheduled cleanup

Given a 31-day-old history row and WAV
And text retention is 90 days
And audio retention is 30 days
When retention cleanup runs
Then the history row remains
And the WAV is removed
And the row audio path is cleared

### Filesystem failure

Given an expired registered audio path that cannot be deleted
When retention cleanup runs
Then cleanup reports the failure
And the database still tracks the path

### Legacy orphan cleanup

Given an untracked WAV file in the application recordings directory at startup
When orphan cleanup runs
Then the WAV is deleted

## Verification

- Rust unit tests for policy conversion, settings defaults/migration, schema migration, independent cleanup, deletion failure, orphan cleanup, and finalization behavior.
- React component test for both selectors and backend setting keys.
- Desktop frontend build and typecheck.
- Full Rust test, Clippy, and formatting checks before feature completion.

## Verification evidence

- `cargo check --lib` passed on 2026-08-24.
- `cargo test history::retention::tests --lib`: 2 passed.
- `cargo test history::store::tests --lib`: 14 passed.
- `cargo test retention_ --lib`: 3 passed, including the two settings contract tests.
- `cargo test services::transcription_finalize::tests --lib`: 7 passed with access to the application data directory.
- `vitest run src/components/Home/__tests__/GeneralSettings.test.tsx`: 4 passed.
- `vitest run` for `ModelSettings.test.tsx` and `PolishSection.test.tsx`: 4 passed.
- `tsc --noEmit` passed.
- `npm run check:i18n` passed for every desktop and website locale.
- `rustfmt --check` passed for every Rust file changed by this feature.
- Repository-wide Cargo tests, Clippy, and builds remain part of the parent B1–P13 integration pass because parallel feature files were still changing during this verification.

## Processing choices (2026-09-05)

Retention is independent of the [local/cloud processing choice](../../../product-simplification/0.1.0/prd/erd.md). Choosing Local only disables cloud transcription and cloud polish. Choosing configured cloud STT does not opt into cloud polish, change stored retention, or grant context access.
