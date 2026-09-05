# File, History, and Translation Workbench Specification

## Version

- Feature: `file-history-translation`
- User-visible name: Transcription Workbench
- Version: `0.1.0`
- Status: Completed
- Opportunity trace: P7-P9

## Problem Statement

The engine can transcribe an in-memory file and history stores useful metadata,
but the desktop UI cannot import media, export captions, inspect or replay a
record, delete it individually, retranscribe/re-polish it, or reinsert either
version. Translation is currently only an implicit prompt variation.

## Goal

Users can import supported local audio/video files, transcribe them through the
headless engine, export TXT/Markdown/SRT, and manage every result in a complete
history workbench. Translation is a first-class mode with an explicit target
language and no same-language polish ambiguity.

## Non-Goals

1. Do not upload local media unless a selected cloud provider is explicitly
   active and the user confirms that mode.
2. Do not provide frame-accurate professional subtitle editing in this version.
3. Do not infer a translation target from system locale.

## First-Principles Model

1. A file import is another backend transcription source, not a frontend-only
   special case.
2. Export is derived from persisted, timestamped segments when available.
3. Every destructive history action targets one explicit ID.
4. Retranscription consumes retained media; re-polish consumes retained raw
   text and must not require audio.
5. Translation preserves meaning in a declared target language; normal polish
   preserves the source language.

## Current surface (2026-09-05)

The [product simplification contract](../../../product-simplification/0.1.0/prd/erd.md) keeps basic history, text export, copy, reinsertion, and retry under History. Import, video decoding, explicit translation, and caption export live at Advanced → Media and captions (`/workbench`). Basic History does not initialize file jobs or drag-and-drop listeners. Further media-product expansion is deferred; the existing single-cue SRT fallback remains limited subtitle output.

## Information Architecture

- Workbench import zone and native file picker for audio/video.
- Job list with queued/running/completed/error states and cancellation.
- Per-entry details: raw/final text, metadata, audio player, delete,
  retranscribe, re-polish, copy/reinsert, and export.
- Translation mode: disabled or target language; visible in profile and result
  metadata.

## Data Contract

```rust
struct FileTranscriptionRequest {
    path: PathBuf,
    profile_id: Option<String>,
    translation_target: Option<String>,
}

struct TimedSegment {
    start_ms: i64,
    end_ms: i64,
    text: String,
}

enum ExportFormat { Txt, Markdown, Srt }
enum HistoryOperation { Retranscribe, Repolish, ReinsertRaw, ReinsertFinal }
```

The history schema adds source kind/path, translation target, optional timed
segments, last delivery outcome, and correction/error metrics through a
backward-compatible migration.

## Acceptance Criteria

1. P7: supported WAV, MP3, M4A, FLAC, OGG, MP4, MOV, and WebM files can be
   selected or dropped; unsupported/missing files fail before engine work.
2. P7: jobs expose progress/state and can export TXT, Markdown, and valid SRT;
   SRT falls back to one bounded segment when the provider has no timestamps.
3. P8: history displays raw/final text, engine/model/language/timings/source,
   audio playback when retained, and explicit delivery/error state.
4. P8: individual delete removes the row and managed audio; retranscribe,
   re-polish, reinsert raw/final, and copy raw/final are functional.
5. P9: translation requires a target language different from the known source
   language and records it in the result; normal polish is instructed to keep
   the source language.
6. All operations are backend-owned and exposed through typed IPC.
7. File paths are canonicalized and restricted to the selected file; exports
   never overwrite without explicit confirmation.

## BDD Scenarios

### Import an audio file

Given a readable supported audio file
When the user drops it into the workbench
Then the backend validates and decodes it
And a completed history entry identifies `file` as its source.

### Export SRT without provider timestamps

Given a completed file transcription with duration but no timed segments
When SRT export is requested
Then Voice Flow emits a valid index, timestamp range, and text block.

### Re-polish without retained audio

Given a history entry with raw text and deleted audio
When re-polish is requested
Then the selected polish configuration processes the raw text
And the updated final text is persisted without STT.

### Translate explicitly

Given French source text and target `en`
When translation mode runs
Then the result metadata records `en`
And the prompt requests English meaning preservation rather than French polish.

## Verification

- Validation, decoder, export, translation-policy, and history migration tests.
- Backend command tests using temporary media and output directories.
- Frontend tests for drop states, details, action availability, and confirmation.
- Black-box E2E import/history/export smoke flow.

## Completion Evidence

- Media validation/import, bounded jobs, progress/cancellation, TXT/Markdown/SRT
  export, detailed history actions, retained audio playback, and explicit
  translation targets are implemented in the backend with typed IPC and UI.
- Focused workbench, history store, command, finalization, retry, and shared
  audio pipeline suites passed. The history UI suite passed 7 tests.
- Orphan cleanup has a regression test proving that imported source media inside
  the recordings directory is preserved.
- The native Tauri smoke test opened History and verified its search workbench;
  the full Rust and desktop test/build gates passed.
