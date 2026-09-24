# VoiceFlow Native — macOS rewrite (Swift/SwiftUI)

Native rewrite of VoiceFlow, targeting macOS 26+ (Apple Silicon), Liquid Glass.

- `index.html` — design mockup (simulated macOS desktop): `python3 -m http.server 5849`
- `VoiceFlow/` — SwiftPM package for the application
- `build.sh` — builds and assembles `dist/VoiceFlow.app` (ad hoc signature)

## Phase 1 — end-to-end skeleton (current state)

A single feature, but a real one: **hold ⌥ Space → dictate → release →
the text is inserted into the active app.**

Chain: `HotkeyManager` (CGEventTap, swallows ⌥ Space) → `AudioRecorder`
(AVAudioEngine) → `TranscriptionSession` (SpeechAnalyzer/SpeechTranscriber,
100% on-device) → `TextInjector` (a faithful port of
`apps/desktop/src-tauri/src/text_injector/macos.rs`: AX target captured at
startup, otherwise simulated typing ≤ 400 graphemes, otherwise clipboard + Cmd+V
with full backup/restore).

### Run

```sh
./build.sh && open dist/VoiceFlow.app
```

On first launch:
1. grant **microphone** access (system dialog);
2. grant **accessibility** access (system prompt → Settings > Privacy);
3. relaunch the app after granting accessibility (the CGEventTap is created at launch);
4. the language model downloads in the background (↓ icon in the menu bar).

The ad hoc signature changes with every build: macOS may ask for the
Accessibility checkbox again after a rebuild (uncheck/recheck in Settings).

### To validate (phase goals)

- [ ] **French** quality from SpeechAnalyzer vs. the current Tauri app
      (same dictations, compare). Plan B if disappointing: WhisperKit.
- [ ] End-of-dictation → inserted-text latency.
- [ ] Injection parity: Cursor, Mail, Safari, Terminal, Spotlight field.
      Check AX mode (insertion without reactivation) and clipboard
      restoration.

### Interface

Modeled on the Tauri app, measured from a screenshot of the real app
(`/Applications/Voice Flow.app`) rather than guessed:

- window with no title bar, dark 248 px full-height sidebar,
  traffic lights above the logo;
- logo + "Voice Flow" in italic serif 22, separator line, navigation
  in fully rounded pills (selection = `card` background + border);
- content with 40 px margin, max width 1000: 28 semibold title + gray
  subtitle, status band bordered with 24 px radius, cards at 18 px;
- metric cards: 13 gray label **above**, 36 value below;
- lists in bordered cards with internal dividers, rows expand on click.

Palette from `src/index.css`: `#F9F9F9`/`#FFFFFF`/`#EBEBEB` in light,
`#1B1B1B`/`#212121`/`#343434` in dark.

### Implemented

- **Dictation**: SpeechAnalyzer (Apple), Whisper (tiny to large-v3), SenseVoice
  and Qwen3-ASR; language independent of the system, automatic detection outside
  the Apple engine. Model download visible, with progress, deletion and errors
  displayed. Only one model stays in memory, preloaded at launch and on
  engine change.
- **Dictation cycle**: the microphone starts on press, before the engine
  is ready; audio waits in `EngineFeed`, nothing is lost. Escape
  cancels (during recording: everything is discarded; during processing:
  kept in history, nothing is inserted).
- **Audio input**: microphone selection (CoreAudio), noise reduction via the
  system's voice processing (without lowering other apps' volume), adjustable
  silence trimming.
- **Shortcuts**: customizable dictation shortcut, single key accepted (Fn, F1–F20),
  three modes — hold, toggle, double press. A modifier key used
  alone in a combination (Fn + ↑, right ⌘ + C) triggers
  nothing. Interception on a dedicated thread: the UI can lag without freezing
  the keyboard. Optional second shortcut for **command mode**: a
  spoken instruction ("translate to English") applies to the selected text.
- **Insertion**: origin field remembered at trigger time (can be disabled),
  otherwise simulated typing or clipboard depending on length. Space and
  capitalization connected to the text preceding the cursor. Clipboard restored
  after 700 ms, unless something was copied in the meantime; the dictation is
  marked ephemeral there. Last insertion can be removed from the menu.
- **Polishing**: on-device Apple Intelligence. Six styles, system prompts
  visible and editable, with the option to revert to the original. Long
  dictations are polished in chunks.
- **Per-app rules**: polish style and dictation language.
- **Dictionary**: manual entry, "also heard" variants, CSV import.
  Whole-word replacement only. Terms are also passed to the
  engine (Whisper prompt, SpeechAnalyzer context). Corrections made
  after insertion become suggestions, active once accepted or
  seen three times.
- **Voice commands**: "à la ligne", "nouveau paragraphe",
  "new line"… said alone between two pauses.
- **Snippets**, **SQLite history** (Tauri schema, versioned migrations),
  **statistics** computed in SQL over the whole history, retention
  applied continuously.
- **Import** of history, dictionary and snippets from the Tauri app.
- **Pill**: error messages and live transcript (Apple engine),
  on the screen under the mouse; theme, position, size, color, opacity.
- **Menu bar**: start/stop, cancel, engine, language, polishing.
- **Interface language**: French and English (`Resources/*.lproj`),
  applied immediately. `tools/check-strings.py` checks that no key is
  missing in English.
- **Onboarding** on first launch, **updates** via JSON feed
  (`appcast.json`, published with each release by the
  `release-native.yml` workflow when a `native-v*` tag is pushed).
- **Polish**: launch at login, sounds carried over from the Tauri app, generated
  icon, diagnostic log with rotation.

### Verify

```sh
cd VoiceFlow && swift build && swift test && cd .. && tools/check-strings.py
```

### Deliberate choice: Whisper over MLX

Running the Tauri app's local models (Qwen, Gemma…) via MLX Swift
is impossible **at the same time as WhisperKit**: both depend on
`swift-transformers` in disjoint versions (WhisperKit ≤ 1.2,
mlx-swift-examples ≥ 1.3 on `main`). Whisper is kept for
transcription; polishing stays on the system model. The MLX engine,
written and verified, waits in `attente/`, along with the steps to follow.

Tooling note: compiling MLX requires the Metal toolchain, installed via
(`xcodebuild -downloadComponent MetalToolchain`, 839 MB).

### Still open

- "Vibe coding" editor bridge and cloud services: deliberately left out.
- Audio retention (and therefore playback, re-transcription, translation
  from history): deliberately left out.
- Streaming of the polished text (the result arrives in one block).
- Automatic update installation would require Sparkle and a key
  pair; today the app reports the version and opens the link.
- Per-app engine: dropped, only one model stays loaded and switching
  it per app would cost several seconds per dictation.
