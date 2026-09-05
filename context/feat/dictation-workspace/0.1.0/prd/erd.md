# Dictation workspace

Status: Completed. User approved 2026-09-05 after native Wispr review.

## Outcome

Returning users see usage and recent transcription history on Home, a dedicated Statistics page, and direct editors for Dictionary, Snippets, Styles, and Vibe coding. Local processing and existing history recovery remain available.

## Contracts

- Rust owns statistics, history, style rules, snippets and editor-context policy. Frontend presents typed IPC results.
- Home loads five newest history entries (including failed captures so recovery is visible), a seven-day usage summary, and compact setup guidance. Setup failure does not hide history or statistics.
- Statistics offer seven days, thirty days and retained history. Counts concern successful dictations, not imported files. Durations and daily activity use backend aggregates; missing audio duration is not invented. Deleted history reduces aggregates. No estimated time saved, percentile ranking or sample activity.
- Reuse the full history entry presentation and recovery actions on Home. Preserve raw/final comparison, audio playback, copy, retry and delete. A result delivery failure remains visible.
- Dedicated Snippets and Styles expose existing deterministic expansion and explicit app rules; they do not implicitly enable cloud processing.
- Vibe coding uses actual bounded editor context and backend policy, with explicit context access and visible setup/status. See its dedicated spec for editor support and verification.
- Transforms, Scratchpad, live meetings, cloud sync and accounts remain outside this approved slice.

### Statistics aggregate contract

`get_history_statistics` accepts `7d`, `30d`, or `all`. Every aggregate includes
only retained rows whose status is `success` and whose source kind is
`recording`; media imports and failed captures do not count as dictations.
`word_count` uses Unicode word boundaries over the final output. Audio duration
sums known, non-negative `audio_duration_ms` values and does not estimate missing
durations. Local/cloud counts use the persisted STT `is_cloud` value.

Seven- and thirty-day periods use the host's local calendar: today through the
previous six or twenty-nine dates, ending at the time of the request. Their
daily trend includes zero-value dates. Retained-history totals include all
qualifying rows through the request time; its daily trend contains active dates
only, so a long history does not create thousands of empty bins. The response
returns the exact millisecond range and `YYYY-MM-DD` local dates. Deleting or
expiring history therefore reduces later aggregates.
The retained-history chart labels this sparse representation as active dates;
equally spaced points do not imply that adjacent points are consecutive days.

The daily activity visualization uses a smooth curve with a subtle area fill,
not bars. The curve passes through the supplied values without overshooting
below zero or above neighboring values. A single date displays a point.
Exact values remain accessible through point labels and the daily table.

## Acceptance

- Home displays summary and newest entries even when microphone permission is absent.
- Copying or recovering a result targets that result's identifier.
- Statistics period changes cannot display an older request's results as the selected period.
- Empty/error/loading states are distinct. Charts are accompanied by textual values.
- Each primary destination is reachable from navigation and usable in the minimum desktop window.
- New copy is translated in English/French; locale key parity remains valid.

## Verification

Verified: 110 frontend tests, production frontend/native builds, shared TypeScript, i18n and Markdown checks. Rust aggregate/retention and app-only profile registration tests pass; the root execution plan records the final integrated suite and test isolation correction.

Native Voice Flow Dev was installed with strict signature verification. Home rendered recent history beside setup guidance; Statistics returned real retained-history aggregates and changed periods; Dictionary, Snippets, Styles creation/application selection and Vibe coding rendered in French. Screenshots were reviewed for layout. No private transcript contents were copied into documentation.

The editor VSIX passed six Node tests and installed through the actual VS Code CLI in isolated temporary user-data/extensions directories. Live editor-host dictation and microphone delivery remain unverified because the installed dev app lacks Microphone and Accessibility grants. No cloud request or context-sharing setting was enabled for the UI review.
