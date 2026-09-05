# Changelog

All notable changes to the desktop application will be documented in this file.

## v1.2.0 (2026-09-05)

### Features

- Restore Home usage summaries and recent transcription history
- Add retained-history statistics with selectable periods and local/cloud totals
- Add direct Snippets and Styles editors with app-only styles
- Edit dictionary recognition aliases
- Add opt-in Vibe coding with a VS Code-compatible editor adapter
- Add a reproducible signed local development installation build

### Fixes and simplification

- Share transcription rewriting and output-preservation rules
- Keep processing local by default and developer integrations opt-in
- Preserve recovery actions while removing unsafe cross-app Undo and direct
  streaming insertion
- Keep select menus usable inside settings dialogs

### Notes

- The editor adapter shares bounded filename and symbol metadata, not source
  file contents. Context follows the selected local or cloud processing engine.
- Automatic @file tagging is not included.

## v1.1.5 (2026-09-04)

### Bug Fixes

- Preserve the dictation language and reject destructive Polish output

## v1.1.4 (2026-09-04)

### Features

- Unify dictation polish settings (e879c34)

## v1.1.3 (2026-09-04)

### Features

- Preserve original dictation target (8bdc63a)

## v1.1.2 (2026-08-25)

### Features

- Paste latest transcription from the tray (fc2ab9f)

## v1.1.1 (2026-08-24)

### Bug Fixes

- Bundle universal developer CLI (e4925eb)

## v1.1.0 (2026-08-24)

### Features

- Add contextual voice workflows (ae8d826)

## v1.0.7 (2026-08-23)

### Features

- Support updater (a98fdb0)

### Bug Fixes

- Prevent long recordings from being truncated (ced1d10)
- Exclude E2E tool from release bundle (a9fbe79)
- Embed manifest in Windows tests (d972d55)
- Restore macOS permissions and model downloads (fd0df15)
- Allow fn modifier chord hotkeys (802ad3a)

## v1.0.4 (2026-07-02)

### Features

- Refine transcription polish and UX (6f31bfd)
- Expose correction dictionaries (474a157)
- Make settings and local polish easier to use (6f9e07e)
- Make polish model risk diagnosable (9e54520)
- Support windows (adb73ec)
- Verify cloud setup and keep polish output plain text (e9a807f)
- Customize pill appearance (a83e1f4)
- Support deliberate recording finish triggers (e9ee0d2)
- Add voice-writing polish templates (acde99b)
- Add window context capture via OCR to improve polish accuracy (ac6a493)
- Optimize UI for better experience (14eaad3)
- Optimize onboarding guide (48c59d5)
- Optimize text inject performance (ee9cb14)
- Implement multi-shortcut profiles and update UI (388d909)
- Add changelog viewer in about page (813614b)
- Add audio command boundary (aa0aada)
- Add single-instance plugin and handle silent recordings (a8144cd)
- Add inhouse dev variant with custom icons and hotkey labels (b970923)
- Improve audio chunking (9e2f54d)
- Add recording cancellation and enhanced retry experience (a4ae864)
- Add transcription retry functionality (ac33fca)
- Refactor hotkey (c2a744d)
- Add model file size validation and metal headers (5c0743d)
- Add custom template management and improve VAD (fdc3940)
- Add Gemma 2B IT local model support (2a3769b)
- Add history dashboard, cloud service UI and improve design (bc1bc8a)

### Bug Fixes

- Release modifiers after windows paste injection (e1f2319)
- Stt sidecar should not lanuch terminal (6744698)
- Llm sidecar should not lanuch terminal (1e8f2b7)
- Select popover can not be clicked (fde0e5d)
- Learn compact corrections safely (5392943)
- Pill/tooltip position in windows (4dd9664)
- Preserve modal close transitions (2e85e3e)
- Allow production changelog loading (8347bed)
- Surface settings nav items that need attention (0782ca1)
- Keep local polish bounded and downloads complete (4741850)
- Make Windows shortcuts usable across platforms (6c8a06a)
- Keep tray window reachable from dock (8b87607)
- Prevent shortcut slash leaks (66d534a)
- Keep recording feedback and shortcut state trustworthy (82b1759)
- Keep idle tooltips from showing pill body (53fa411)
- Adapt cloud polish timeout for long requests (aae00e2)
- Context aware visual terms (ad8d5b7)
- Add default value for pill_size field (46bcc21)
- Update changelog page layout (928b0da)
- Shortcut not working when no permission (6a97b91)
- Prevent changelog fetch loop and improve UI (79c07fa)
- Return committed transcript from ElevenLabs finish() and add query params (84c3a40)
- Eliminate flash of unstyled content on app startup (c092c9b)
- Embed beep audio files at compile time (e8ceda6)

## v1.0.0 (2026-06-15)

### Features

- Refine transcription polish and UX (af242a1)
- Expose correction dictionaries (8968a88)
- Make settings and local polish easier to use (df5b2bc)
- Make polish model risk diagnosable (17fc83b)
- Support windows (68cc615)
- Verify cloud setup and keep polish output plain text (73a5fb3)
- Customize pill appearance (6a47ab4)
- Support deliberate recording finish triggers (a80c22c)
- Add voice-writing polish templates (9975217)
- Add window context capture via OCR to improve polish accuracy (e749271)
- Optimize UI for better experience (6f4936c)
- Optimize onboarding guide (db9e682)
- Optimize text inject performance (c92c4ce)
- Implement multi-shortcut profiles and update UI (6814403)
- Add changelog viewer in about page (0c72a43)
- Add audio command boundary (b4e7a53)
- Add single-instance plugin and handle silent recordings (a37e33c)
- Add inhouse dev variant with custom icons and hotkey labels (8b43163)
- Improve audio chunking (d9e6cf6)
- Add recording cancellation and enhanced retry experience (e12a97e)
- Add transcription retry functionality (c274588)
- Refactor hotkey (d01a7b7)
- Add model file size validation and metal headers (151a7dd)
- Add custom template management and improve VAD (e4618d9)
- Add Gemma 2B IT local model support (590cc77)
- Add history dashboard, cloud service UI and improve design (9007d47)

### Bug Fixes

- Surface settings nav items that need attention (92134f9)
- Keep local polish bounded and downloads complete (d2b4c59)
- Make Windows shortcuts usable across platforms (4b238f4)
- Keep tray window reachable from dock (b53ced4)
- Prevent shortcut slash leaks (4dea12d)
- Keep recording feedback and shortcut state trustworthy (6d2bdd2)
- Keep idle tooltips from showing pill body (ad32ae3)
- Adapt cloud polish timeout for long requests (fea6258)
- Context aware visual terms (ce46436)
- Add default value for pill_size field (f027349)
- Update changelog page layout (8380247)
- Shortcut not working when no permission (ed322f3)
- Prevent changelog fetch loop and improve UI (254476d)
- Return committed transcript from ElevenLabs finish() and add query params (64ede6b)
- Eliminate flash of unstyled content on app startup (b56424a)
- Embed beep audio files at compile time (a766981)

## v0.6.5 (2026-06-08)

### Features

- Make polish model risk diagnosable (9e54520)

### Bug Fixes

- Keep local polish bounded and downloads complete (4741850)

## v0.6.4 (2026-06-02)

### Bug Fixes

- Make Windows shortcuts usable across platforms (6c8a06a)
- Keep tray window reachable from dock (8b87607)

## v0.6.3 (2026-05-25)

### Features

- Support windows (adb73ec)

## v0.6.2 (2026-05-22)

### Bug Fixes

- Prevent shortcut slash leaks (66d534a)
- Keep recording feedback and shortcut state trustworthy (82b1759)

## v0.6.1 (2026-05-19)

### Bug Fixes

- Keep idle tooltips from showing pill body (53fa411)

## v0.6.0 (2026-05-19)

### Features

- Verify cloud setup and keep polish output plain text (e9a807f)
- Customize pill appearance (a83e1f4)
- Support deliberate recording finish triggers (e9ee0d2)
- Add voice-writing polish templates (acde99b)
- Add window context capture via OCR to improve polish accuracy (ac6a493)
- Optimize UI for better experience (14eaad3)
- Optimize onboarding guide (48c59d5)
- Optimize text inject performance (ee9cb14)
- Implement multi-shortcut profiles and update UI (388d909)
- Add changelog viewer in about page (813614b)
- Add audio command boundary (aa0aada)
- Add single-instance plugin and handle silent recordings (a8144cd)
- Add inhouse dev variant with custom icons and hotkey labels (b970923)
- Improve audio chunking (9e2f54d)
- Add recording cancellation and enhanced retry experience (a4ae864)
- Add transcription retry functionality (ac33fca)
- Refactor hotkey (c2a744d)
- Add model file size validation and metal headers (5c0743d)
- Add custom template management and improve VAD (fdc3940)
- Add Gemma 2B IT local model support (2a3769b)
- Add history dashboard, cloud service UI and improve design (bc1bc8a)

### Bug Fixes

- Adapt cloud polish timeout for long requests (aae00e2)
- Context aware visual terms (ad8d5b7)
- Add default value for pill_size field (46bcc21)
- Update changelog page layout (928b0da)
- Shortcut not working when no permission (6a97b91)
- Prevent changelog fetch loop and improve UI (79c07fa)
- Return committed transcript from ElevenLabs finish() and add query params (84c3a40)
- Eliminate flash of unstyled content on app startup (c092c9b)
- Embed beep audio files at compile time (e8ceda6)

## v0.5.1 (2026-05-07)

### Features

- Add window context capture via OCR to improve polish accuracy (e749271)
- Optimize UI for better experience (6f4936c)
- Optimize onboarding guide (db9e682)
- Optimize text inject performance (c92c4ce)
- Implement multi-shortcut profiles and update UI (6814403)
- Add changelog viewer in about page (0c72a43)
- Add audio command boundary (b4e7a53)
- Add single-instance plugin and handle silent recordings (a37e33c)
- Add inhouse dev variant with custom icons and hotkey labels (8b43163)
- Improve audio chunking (d9e6cf6)
- Add recording cancellation and enhanced retry experience (e12a97e)
- Add transcription retry functionality (c274588)
- Refactor hotkey (d01a7b7)
- Add model file size validation and metal headers (151a7dd)
- Add custom template management and improve VAD (e4618d9)
- Add Gemma 2B IT local model support (590cc77)
- Add history dashboard, cloud service UI and improve design (9007d47)

### Bug Fixes

- Add default value for pill_size field (f027349)
- Update changelog page layout (8380247)
- Shortcut not working when no permission (ed322f3)
- Prevent changelog fetch loop and improve UI (254476d)
- Return committed transcript from ElevenLabs finish() and add query params (64ede6b)
- Eliminate flash of unstyled content on app startup (b56424a)
- Embed beep audio files at compile time (a766981)

## v0.4.0 (2026-04-25)

### Features

- Implement multi-shortcut profiles and update UI (388d909)
- Add changelog viewer in about page (813614b)
- Add audio command boundary (aa0aada)
- Add single-instance plugin and handle silent recordings (a8144cd)
- Add inhouse dev variant with custom icons and hotkey labels (b970923)

### Bug Fixes

- Update changelog page layout (928b0da)
- Shortcut not working when no permission (6a97b91)
- Prevent changelog fetch loop and improve UI (79c07fa)

## v0.3.0 (2026-04-13)

### Features

- Improve audio chunking (9e2f54d)
- Add recording cancellation and enhanced retry experience (a4ae864)
- Add transcription retry functionality (ac33fca)
- Refactor hotkey (c2a744d)

## v0.2.0 (2026-04-11)

### Features

- Add model file size validation and metal headers (5c0743d)

## v0.1.2 (2026-04-08)

### Features

- Add custom template management and improve VAD (fdc3940)

### Bug Fixes

- Return committed transcript from ElevenLabs finish() and add query params (84c3a40)

## v0.1.1 (2026-04-06)

### Features

- Add Gemma 2B IT local model support (2a3769b)
- Add history dashboard, cloud service UI and improve design (bc1bc8a)

### Bug Fixes

- Eliminate flash of unstyled content on app startup (c092c9b)

## v0.1.0-beta.8 (2026-03-09)

### Bug Fixes

- Embed beep audio files at compile time (e8ceda6)
