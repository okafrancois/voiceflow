# Wispr Flow official feature and interface evidence

Research snapshot: 2026-09-05. Sources are limited to Wispr Flow's public website and Help Center. This inventory records documented behavior; it does not imply that every feature was independently exercised in the product.

## Evidence labels

- **Documented UI/behavior**: the Help Center names the screen, control, action, limits, or state.
- **Vendor claim**: Wispr describes an outcome without public independent validation here.
- **Availability**: taken from each article's current `Available on` label and plan notes.
- **Uncertain**: official pages conflict, or rollout language says only some users see it.

## Product surfaces

### Desktop Hub and Flow Bar

**Documented UI/behavior.** Mac and Windows pair a management window, the Hub, with a floating Flow Bar used during dictation. The Hub sidebar documents Home, Dictionary, Snippets, Style, Settings, and newer Transforms surfaces. Home contains word count, the active dictation shortcut, history grouped by date, search, and a statistics card. The Flow Bar can be shown or hidden, repositioned, opened for microphone/history/paste actions, and displays recording/processing states. The menu bar or system tray provides quick access to Flow, the last transcript, shortcuts, microphone, languages, help, support, and feedback.

Source: [Navigating the Wispr Flow App](https://docs.wisprflow.ai/articles/5096240724-navigating-the-wispr-flow-app-desktop-ios-and-android)

### iOS app and Flow Keyboard

**Documented UI/behavior.** The app uses Home, Dictionary, Snippets, Style, and Scratchpad/Notes tabs. Dictation inside other apps runs through the Flow Keyboard. Its top bar includes settings, undo/redo, Add to Dictionary, microphone, and writing-style controls. iOS also documents a Live Activity/Dynamic Island timer, Control Center entry points, Siri Shortcuts, Action Button support, and direct note capture.

**Availability.** iOS. Some hardware entry points depend on iPhone model and iOS version.

Source: [Navigating the Wispr Flow App](https://docs.wisprflow.ai/articles/5096240724-navigating-the-wispr-flow-app-desktop-ios-and-android), [Set up Flow shortcuts for iPhone](https://docs.wisprflow.ai/articles/1986921789-how-to-set-up-flow-shortcuts-for-iphone)

### Android app and Flow Bubble

**Documented UI/behavior.** Android uses a floating, movable bubble over other apps. Settings include bubble size and opacity. The bubble requires overlay and accessibility permissions and can be snoozed for ten minutes. History cards include transcript state and retry/report actions.

**Availability.** Android is described as beta in the current product overview. Feature parity is unclear; see conflicts below.

Source: [What is Flow?](https://docs.wisprflow.ai/articles/2772472373-what-is-flow), [Delete transcripts and history](https://docs.wisprflow.ai/articles/4465314211-delete-transcripts-and-history-in-wispr-flow)

## Dictation, formatting, and context

### Capture and insertion

**Documented UI/behavior.** Desktop offers hold-to-talk, a toggleable hands-free session, cancel, and automatic paste into the focused text field. A desktop session warns at 19 minutes and stops at 20 minutes. If insertion fails, Flow retains a recovery path through the clipboard, History, or Paste Last Transcript. The previous clipboard is normally restored after a successful desktop paste. iOS inserts through its keyboard; Android inserts through accessibility services.

**Vendor claim.** Wispr says Flow is about four times faster than typing and learns vocabulary over time. These are product claims, not measurements established by this review.

Source: [What is Flow?](https://docs.wisprflow.ai/articles/2772472373-what-is-flow), [Use Flow hands-free](https://docs.wisprflow.ai/articles/6391241694-use-flow-hands-free), [Fix text not pasting](https://docs.wisprflow.ai/articles/7971211038-fix-text-not-pasting-after-dictation)

### Smart Formatting and Backtrack

**Documented UI/behavior.** Smart Formatting is on by default. Desktop and iOS expose a toggle; Android documents it as always on. It adds punctuation and capitalization, creates numbered lists from spoken sequences, recognizes spoken punctuation and line-break commands, and adjusts spacing/case at the insertion point. Messaging apps may lose a trailing period according to style and text context. Desktop History exposes Undo AI edit and Redo AI edit for an individual transcript.

Backtrack removes filler, false starts, and self-corrections. Phrases such as “actually,” “scratch that,” and “never mind,” or a restated thought, tell Flow to revise the current dictation using the whole utterance.

**Availability.** Mac, Windows, iOS, Android. Style behavior has language caveats; Wispr says Writing Styles work best in English, while Smart Formatting has broader multilingual support.

Source: [Smart Formatting and Backtrack](https://docs.wisprflow.ai/articles/5373093536-how-do-i-use-smart-formatting-and-backtrack)

### Context Awareness

**Documented UI/behavior.** Flow categorizes the focused app or website as Personal messaging, Work messaging, Email, or Other. It reads nearby text and applies app-category style, casing, spacing, and punctuation. It can use names near the cursor, preserve proper nouns, remember IDE file names, recognize code variables, and tag files in supported editors.

The detailed privacy section says a dictation request can include app information, text before/inside/after the selection, on-screen text, code symbols and filenames, app user identifier, session apps, a screenshot, and conversation history. The page says this context is collected locally and sent with the request unless Privacy Mode is on; per-dictation context is cleared after the request.

**Availability.** Full on Mac, partial on Windows, limited to the focused text field on iOS. Windows lacks several Mac behaviors, including conversation context, element descriptions, and IDE-specific handling. Android is not listed on the article's availability line.

Source: [Context Awareness](https://docs.wisprflow.ai/articles/4678293671-Context-Awareness), [Use Flow with Cursor, VS Code, and other IDEs](https://docs.wisprflow.ai/articles/6434410694-use-flow-with-cursor-vs-code-and-other-ides)

### Styles and transforms

**Documented UI/behavior.** Styles are selected per app category. Formal uses capitalization and punctuation; Casual reduces punctuation; Very Casual removes capitalization and more punctuation but is limited to Personal; Excited adds exclamation marks and is unavailable for Personal. Flow detects known native apps and websites and users can assign more apps.

Transforms are a separate selected-text rewrite surface on desktop. Users can create named prompts with shortcuts, apply built-in rules, review an inline diff, accept, undo, retry, copy, and give feedback. Auto-apply can run a selected transform after dictation. Public docs still label Transforms as beta, and the wand is not shown to every user even though shortcuts and context-menu entry remain documented.

**Availability.** Styles: Mac, Windows, iOS; optimized for English; not documented on Android. Transforms: Mac and Windows, beta, with partial iOS Polish behavior mentioned in the beta article. Command Mode is Mac/Windows and requires a paid plan.

Source: [Set up Flow Styles](https://docs.wisprflow.ai/articles/2368263928-how-to-setup-flow-styles), [Configure Transform shortcuts and custom prompts](https://docs.wisprflow.ai/articles/2719941210-how-to-configure-polish-shortcuts-and-custom-prompts), [Use Transforms (Beta)](https://docs.wisprflow.ai/articles/8068950331-how-to-use-transforms-beta), [Use Command Mode](https://docs.wisprflow.ai/articles/4816967992-how-to-use-command-mode)

## Personal vocabulary and reusable text

### Dictionary

**Documented UI/behavior.** Users add words or phrases to boost recognition. “Correct a misspelling” creates an explicit wrong-to-right replacement. Desktop supports search, sort, starred priority, bulk deletion, personal/team filters, and experimental CSV import. Entries sync across devices. Personal items outrank team items; starred words get recognition priority. Some entries may carry origin badges for contact import or automatic learning.

**Availability.** Personal dictionary is described across desktop, iOS, and Android in the dedicated article. Team sharing requires Team, Business, or Enterprise. Bulk import is desktop and experimental.

Source: [Teach Flow your words with the dictionary](https://docs.wisprflow.ai/articles/4052411709-teach-flow-your-words-with-the-dictionary), [Bulk import dictionary items and snippets](https://docs.wisprflow.ai/articles/8955301725-how-do-i-bulk-import-for-dictionary-and-snippets)

### Snippets

**Documented UI/behavior.** A snippet maps a spoken trigger to saved replacement text. Matching is case-insensitive, while inserted casing is preserved from the saved expansion. Personal snippets win over team snippets on a duplicate trigger. Desktop supports search, sorting, personal/team filters, bulk deletion/sharing, and experimental JSON import up to 1,000 items under 3 MB. The documented limits are 60 characters for a trigger and 4,000 characters for expansion text.

**Availability.** Personal snippets work on Basic according to the dedicated page. Team sharing requires Team, Business, or Enterprise. Desktop and iOS are documented; Android availability is unclear.

Source: [Create and use snippets](https://docs.wisprflow.ai/articles/5784437944-create-and-use-snippets)

## History, recovery, and statistics

### History

**Documented UI/behavior.** Desktop Home groups transcripts by day and supports search, copy, flag/report, retry/recover, and per-entry AI-edit undo/redo. The current deletion article says desktop has no per-transcript delete and clears local history on sign-out, while the navigation article says deleting a transcript asks for confirmation. iOS groups history by date and exposes copy, audio playback, flag/report, retry, and delete through tap, swipe, or long press. Android cards show time/date, failures, report, retry when audio exists, and delete.

Saved audio enables retry after network/server failure. Desktop audio is documented as expiring after 14 days while transcript text remains. iOS audio lasts until manual or automatic deletion. Android prunes older audio under storage pressure.

**Availability.** Mac, Windows, iOS, Android, with materially different actions and retention behavior.

Source: [Navigating the Wispr Flow App](https://docs.wisprflow.ai/articles/5096240724-navigating-the-wispr-flow-app-desktop-ios-and-android), [Retry failed transcriptions](https://docs.wisprflow.ai/articles/2503460374-retry-failed-transcriptions), [Delete transcripts and history](https://docs.wisprflow.ai/articles/4465314211-delete-transcripts-and-history-in-wispr-flow)

### Statistics

**Documented UI/behavior.** Desktop shows streak, average words per minute, and total words; the product overview says the card compares word counts with familiar text lengths. iOS unlocks stats after 500 dictated words and shows Words Spoken, Days Used, and Words Per Minute. Android keeps local word count, streak, WPM, and apps-used data; signing out resets it.

Source: [Navigating the Wispr Flow App](https://docs.wisprflow.ai/articles/5096240724-navigating-the-wispr-flow-app-desktop-ios-and-android), [Delete transcripts and history](https://docs.wisprflow.ai/articles/4465314211-delete-transcripts-and-history-in-wispr-flow)

## Privacy, local storage, and cloud processing

**Documented UI/behavior.** Desktop provides separate controls for model improvement, dictation cloud storage, and local transcript storage. Local choices are Store data locally, Auto-delete after 24 hours, and Never store data locally. Turning cloud storage off prevents server-side history storage but is separate from desktop local retention. Snippets and dictionaries are stored by Wispr and sync regardless of Privacy Mode or dictation cloud storage.

The vendor states that third-party AI processors operate under zero-retention agreements. Turning “Improve the model for everyone” off prevents Wispr from using audio, transcripts, and edits for training. Dictation cloud storage can store transcripts, audio, history, Notetaker data, and personalized speech models on Wispr servers. The security FAQ says customer data is processed and stored in the United States and that dictation processing is server-side. This means Flow's locally retained history should not be described as local transcription.

Enterprise controls can lock model training, cloud storage, Context Awareness, and zero-retention settings. A HIPAA BAA disables model training. The current Android requirements say offline transcription is unavailable and an active internet connection is required. A separate Android deletion article says only metadata is uploaded, never audio or transcript text. The public docs do not explain how those two statements fit together. iOS can insert a rough on-device draft during a cloud failure, but the dedicated article says every dictation still goes to the cloud for the full transcription. Public docs do not establish a fully local production transcription path.

Source: [Privacy Mode and cloud sync](https://docs.wisprflow.ai/articles/4709791908-understanding-privacy-mode-and-cloud-sync), [Security and compliance FAQ](https://docs.wisprflow.ai/articles/3467817258-security-and-compliance-faq), [Context Awareness](https://docs.wisprflow.ai/articles/4678293671-Context-Awareness), [Turtle Mode on iOS](https://docs.wisprflow.ai/articles/2752539613-turtle-mode-on-ios-on-device-drafts-and-tap-to-insert), [Android system requirements](https://docs.wisprflow.ai/articles/6344532666-android-system-requirements), [Delete transcripts and history](https://docs.wisprflow.ai/articles/4465314211-delete-transcripts-and-history-in-wispr-flow)

## Shortcuts and commands

**Documented UI/behavior.** Desktop actions include Push to Talk, Hands-free, Enter rebind, Paste Last Transcript, Copy Last Transcript, Command Mode, Note, Cancel, and Open Scratchpad. Defaults differ by OS. Users can bind keyboard combinations and supported mouse buttons; one action can have multiple bindings. Bindings are checked for collisions and OS-reserved combinations. Double-tapping push-to-talk locks hands-free mode.

Command Mode runs a spoken instruction rather than inserting dictated text. It has a separate shortcut, cancel behavior, and hands-free gesture. The Help Center describes it under Experimental and requires a paid plan. iOS uses Action Button, Back Tap, Control Center, and Siri Shortcuts rather than desktop global hotkeys.

Source: [Route dictation with keyboard shortcuts](https://docs.wisprflow.ai/articles/5298382595-route-dictation-directly-to-slack-email-or-calendar-with-keyboard-shortcuts), [Supported and unsupported hotkeys](https://docs.wisprflow.ai/articles/2612050838-supported-unsupported-keyboard-hotkey-shortcuts), [Use Command Mode](https://docs.wisprflow.ai/articles/4816967992-how-to-use-command-mode), [Set up Flow shortcuts for iPhone](https://docs.wisprflow.ai/articles/1986921789-how-to-set-up-flow-shortcuts-for-iphone)

## Official-document conflicts and rollout uncertainty

1. The navigation and security pages cap dictionary entries at 30 characters, while the dedicated dictionary page says 60 characters on all platforms. Treat the exact limit as unverified.
2. The cross-device sync article says Android lacks Dictionary, Snippets, and Styles. A newer dedicated dictionary article gives a full Android flow. Dictionary may be a recent or staged rollout; Snippets and Styles still lack equivalent Android evidence.
3. Desktop transcript deletion conflicts: the navigation page describes a confirmation for deleting a transcript, while the dedicated deletion page says desktop has no per-transcript delete. The latter is more specific, but the behavior should be checked in the installed build before using it as a product benchmark.
4. Shortcut documentation conflicts on the number of bindings per action: one page says up to four; another says there is no fixed limit. Do not copy either limit without product verification.
5. The navigation article calls context “limited, relevant text,” while the dedicated Context Awareness page lists screenshots, surrounding text, session apps, identifiers, and conversation history among possible request inputs. The detailed page is the safer basis for privacy comparison.
6. Android privacy documentation conflicts. Recent setup/navigation pages show Data & Privacy and Dictation cloud storage controls, while the security FAQ and retry article say Android has no such section or retention toggle.
7. Android's requirement for online transcription is difficult to reconcile with another official statement that audio and transcript text are never uploaded. The sources do not disclose enough transport/processing detail to resolve this.
8. Transforms are marked beta, the Flow Bar wand is shown only to some users, and several features are plan-gated or controlled by enterprise policy. Public documentation proves the designed surface, not universal account availability.

## Selectable feature groups for Voice Flow discussion

These groups are intentionally separable so a later product decision can select behavior without copying Wispr's cloud assumptions.

1. **Core capture and recovery**: push-to-talk, hands-free, cancel, visible recording states, automatic insertion, Paste/Copy Last Transcript, saved-audio retry, and clear failure states.
2. **Deterministic formatting controls**: spoken punctuation and line breaks, list detection, insertion-point casing/spacing, Smart Formatting toggle, and per-transcript undo/redo.
3. **Self-correction during speech**: Backtrack for false starts and restatements, with inspectable rules or a local model path.
4. **Personal vocabulary**: word boosting, explicit misspelling replacement, search/sort/star, import/export, and visible precedence rules.
5. **Reusable snippets**: voice triggers, exact expansion text, conflict handling, search, bulk import/export, and optional sharing kept separate from personal storage.
6. **Context profiles**: user-selected app categories and styles first; nearby-text, screenshot, conversation, and IDE context as separate opt-ins with a precise preview of what leaves the device.
7. **History and privacy**: local searchable history, per-entry deletion, configurable retention, explicit audio lifetime, retry eligibility, and separate toggles for local history, cloud processing, cloud storage, and training use.
8. **Stats**: local word count, WPM, active days, and streak, with a way to disable or reset them.
9. **Shortcut system**: configurable multi-binding actions, mouse buttons, collision validation, platform defaults, and a clear distinction between dictation, rewrite, and command execution.
10. **Selected-text transforms**: named rewrite rules, keyboard activation, inline diff, accept/undo/retry, and custom prompts. This is adjacent to dictation and can remain optional.

For a local-first open-source product, groups 1, 2, 4, 5, 7, and 9 map most directly to user control and reliability. Context capture and cloud rewrite features need a separate privacy decision because Wispr's behavior sends much richer material than the phrase “nearby text” suggests.
