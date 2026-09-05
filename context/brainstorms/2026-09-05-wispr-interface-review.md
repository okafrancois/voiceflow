# Wispr Flow: interface review and selection brief

Date: 2026-09-05. Interface review complete; selection pending. Product implementation is pending the user's feature and interface choices.

## User direction

The user prefers a useful statistics summary and transcription history on the home screen over the recently introduced readiness-only page. Keep local model choice and open-source implementation as Voice Flow's distinguishing requirements. Review Wispr Flow before selecting what to port. Do not interpret the review as approval to implement every candidate.

## Evidence boundaries

- The installed macOS Wispr Flow app, version 1.6.774, was inspected through native accessibility data and selected screenshots. Three GPT-5.6 Sol medium agents covered official research, repository feasibility, and the local development build.
- Automatic approval review initially blocked access because account and transcript content was visible. The user explicitly authorized incidental viewing for this analysis. No private content is copied into this report.
- Native observations below record interface structure only. No transcript text, account identifier, payment details, or recording content is retained here.
- Public behavior is inventoried separately in [official documentation research](./2026-09-05-wispr-official-research.md). Public documentation and native observation must remain distinguishable.

## Installed macOS interface: observed

| Surface | Observed structure | Implication for Voice Flow |
| --- | --- | --- |
| Main navigation | Dictation, Notetaker, Insights, Dictionary, Snippets, Style, Transforms, Scratchpad | The installed version separates dictation, meeting capture, usage analysis, vocabulary, reusable text, style, rewriting, and notes. Do not combine these into one generic workflow page without considering their different tasks. |
| Dictation home | Welcome/header, a style setup card, transcript history grouped by date, timestamps, history search | The page is primarily a working history, with onboarding shown as a secondary card. Recent results should occupy the main space in Voice Flow. |
| Transcript row | Transcript body with playback, copy, feedback, and a more-options menu | Recovery actions belong beside the relevant result. Feedback transmission is separate from recovery and is not required for a local-first product. |
| Usage summary | Total words, words per minute, day streak | Useful for a compact summary, but each metric needs a clear definition. Counts should not be presented as measured time savings. |
| Secondary navigation | Settings, Help, team/referral entry points | Settings/help serve the core task; commercial growth/account controls do not belong in a local-first port by default. |

## Additional native observations

| Surface | Observed interface and controls | Port assessment |
| --- | --- | --- |
| Insights | Usage tab; three summary cards for speaking rate, corrections, and total words; app-category bars; activity heatmap and streaks; share control. | Keep factual totals and trends. Comparative typing claims, book equivalents, and streak rewards are optional and need defined assumptions. |
| Dictionary | All/personal/team tabs, search, add/edit/delete/star; add dialog switches between vocabulary and explicit misspelling-to-correction pairs. | Existing local vocabulary and correction memory are a strong foundation. Stars need a real engine effect before porting. |
| Snippets | Searchable trigger-to-expansion list; add dialog with rich-text formatting and character limit; personal/team distinction. | Expose the existing deterministic backend. Rich-text delivery is an additional contract, not an existing capability to assume. |
| Style | Personal/work messages, email, other; wizard with side-by-side Formal/Casual/Very casual examples. | Use preview cards and explicit app assignments; automatic app categories add maintenance and assumptions. |
| Auto Cleanup | Separate global control with None/Light/Medium levels; interface describes preserving the original dictation and recovering it from history. | Separate recognition from rewriting. Restore raw text within history; do not revive blind operating-system Undo. |
| Transforms | Named prompts, shortcuts, view-changes action, create form with name/shortcut/prompt and autosave indication. | Existing rewriting engine can support preview/copy first. Inline replacement and reliable undo need additional design and verification. |
| Scratchpad | New note, recent notes, search, shortcut, optional Flow Bar entry and cloud-sync banner. | Local notes require their own persistence; history records are not editable notes. |
| Notetaker | Upcoming meetings, calendar connection, new recording note, personal/shared past meetings, search and introduction previews. | A separate capture and document lifecycle. Do not equate imported audio transcription with a complete live meeting recorder. |
| Notetaker settings | Meeting reminders/detection, maximum recording duration, stop on call end, screen-share hiding, live transcript, sharing, note import. | Consider only after choosing meeting capture as product scope. No meeting was recorded. |
| General/shortcuts | Microphone/languages; push-to-talk, handsfree, Enter, command mode, paste/copy last result, scratchpad, meeting and transform shortcuts, Esc cancellation. | Keep a legible shortcut editor; prioritize recording, cancel, and recovering the last result. |
| System | Login/dock/Flow Bar visibility, sounds/music muting, notifications, scratchpad behavior, correction auto-learning, creator label, reset. | Keep practical controls. Marketing notifications and creator branding do not serve the core local dictation task. |
| Vibe coding | Variable recognition setup for supported IDEs; file tagging option. | Potential later feature; requires concrete IDE context integrations, not only a different prompt. |
| Experimental | Command Mode, spoken Enter, stacked messages, bulk import. | Import can be useful; auto-submit and splitting messages require explicit delivery semantics. |
| Connectors/MCP | Calendar and Slack connections; MCP exposes meeting notes/transcripts to AI apps, explicitly excluding dictations. | Defer with meetings; no account or cloud integration is needed for the proposed core. |
| Data and Privacy | Separate model-improvement, cloud-storage and context controls; local retention selector; notes sync and data-controls links. | Distinguish processing location, storage, retention and context access. Local storage does not prove local inference. |

Forms were inspected without saving entries, changing persistent settings, connecting services, or running transcription/transformation requests. Native observation verifies interface presence, not execution quality. Transform diff/accept/retry behavior comes from official documentation, not an executed native transform. Account, billing and team management were not investigated beyond navigation labels.

## First-principles reduction

- The core job is accurate dictation into another application, with recoverable results and understandable local processing.
- Restore a useful Home and a dedicated Statistics page as the user requests. A readiness-only page fails the returning-user task; use a compact conditional warning instead.
- Aggregate statistics in Rust from existing history records. Define periods, word counting and retention effects before implementation. Put latency/error diagnostics in Advanced; keep personal usage statistics readable.
- Reuse history rows and queries for Home instead of creating a second history store or lifecycle.
- Expose existing dictionary, snippets and app profiles through direct editors before adding new engines or generalized configuration layers.
- Defer accounts, teams, referrals, cloud sync, commercial prompts, comparative productivity claims, auto-submission and live meetings unless explicitly selected.
- Raw-text recovery is bounded and useful. Document-wide Undo without verified selection state is not a safe substitute.

## Local development installation verified

A complete Voice Flow Dev application was built and installed at `/Users/bernyitoutou/Applications/Voice Flow Dev.app`. Strict deep signature verification passed after copying. Its identifier is `com.voiceflow.voicetotext.dev`, its only registered URL scheme is `voiceflow-dev`, and the microphone entitlement is Boolean true. The scheme isolates registration; this does not establish working dev deep-link automation.

The installed application was launched and its Permissions page rendered. Microphone and Accessibility remain ungranted; Screen Recording is labeled optional. End-to-end dictation is therefore not yet verified. Avoid running production/Review and Dev together when testing global shortcuts. Ad hoc signing can require permissions again after rebuilding. Production installation was not overwritten, and no TCC reset or capability edit was performed.

Rebuild: `pnpm --filter @voiceflow/desktop tauri:build:mac:local-install`. This command builds without installing or opening the artifact. See the [completed build plan](../plans/completed/2026-09-05-002-feat-dev-local-install-plan.md).

## Proposed information architecture for discussion

1. **Home / Dictation**: period-selectable summary statistics followed by recent transcription history. A compact readiness banner appears only when setup or delivery needs attention.
2. **Statistics**: a dedicated usage page for volume, audio duration, activity trends, and local/cloud split; latency and failures stay in Advanced diagnostics. WPM may be included with its definition; estimated time savings should remain excluded unless measured.
3. **History**: full search/filtering and raw/final comparison, playback when audio exists, copy, retry, delete, and retention controls. Home can reuse this data and row presentation without duplicating business logic.
4. **Dictionary and snippets**: separate, direct editors for recognition vocabulary and exact spoken-trigger expansion. Existing backend capabilities can support most of the work.
5. **Styles by application**: a focused interface over existing templates, profiles, and application rules, with explicit local/cloud processing choice.
6. **Optional writing tools**: transforms with source/result comparison and explicit replacement. Meeting notes and scratchpad remain separate decisions.

These are candidate interfaces, not a finalized navigation or implementation commitment.

## Decision points

- Home with compact statistics and recent history is the user's stated direction. Choose whether full Statistics is a separate primary page or opens from the summary cards.
- Choose whether dictionary/snippets/styles get direct primary navigation or remain grouped under personalization.
- Choose whether selected-text transforms belong in the first port or a later iteration.
- Decide separately on Scratchpad and Notetaker. They introduce document/meeting lifecycles beyond dictation into another app.
- Select interactions and layout patterns; do not copy Wispr's brand assets, billing, account, team, referral, or mandatory cloud model assumptions.
