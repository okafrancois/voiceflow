# Platform Bridge and Quality Specification

## Version

- Feature: `platform-bridge-quality`
- User-visible name: Setup, Developer Bridge, Code Mode, and Quality
- Version: `0.1.0`
- Status: Completed
- Opportunity trace: P10-P13

## Problem Statement

Initial setup checks permissions but does not recommend a configuration from
real hardware or latency evidence. Automation tools have no stable local
contract. Dictation cannot preserve code-shaped tokens intentionally, and the
dashboard emphasizes usage rather than actionable quality failures.

## Goal

Voice Flow diagnoses the local environment, applies an explicit local/cloud processing choice, exposes a secure local developer bridge, preserves code syntax when
requested, and reports correction, latency, transcription, and delivery quality
with enough metadata to drive improvement.

## Non-Goals

1. Do not expose a network-listening unauthenticated API.
2. Do not install third-party editor extensions automatically.
3. Do not turn the quality dashboard into streaks, points, or gamification.

## First-Principles Model

1. Recommendations require measured capability and an explainable rule.
2. Processing choices do not imply an accuracy guarantee and never alter retention or context consent.
3. Local automation uses the same backend command service as the desktop UI.
4. Code-aware mode is a deterministic formatting policy plus structured editor
   context, not a generic creativity prompt.
5. Quality metrics count failures and corrections without storing dictated
   content.

## Information Architecture

- Setup diagnostics: microphone availability/sample, CPU/RAM/architecture,
  local-model compatibility, and a short opt-in latency test.
- Processing: Local only disables cloud STT and polish; Use configured cloud STT validates credentials before enabling cloud transcription.
- Developer bridge: `voiceflow://` URL scheme plus CLI commands using local IPC;
  authenticated loopback TCP with newline-delimited JSON is optional, disabled by default, token-authenticated, and
  binds only to `127.0.0.1`.
- Code-aware mode: language/editor hint, selected file/path/symbol context, and
  casing/path/command preservation.
- Quality dashboard: success/failure/correction counts, p50/p95 STT/polish/total
  latency, injection failures by application, and exportable content-free data.

## Data Contract

```rust
enum SetupPreset { LocalOnly, CloudStt }
struct DiagnosticReport { microphone: Check; hardware: Hardware; models: Vec<ModelFit>; latency: Option<LatencySample> }
enum BridgeCommand { Start, Stop, Cancel, TranscribeFile, Insert, CopyLast, Status }
struct CodeContext { language: Option<String>, file_path: Option<String>, symbol: Option<String>, editor_id: Option<String> }
struct QualityEvent { kind: QualityEventKind, application_id: Option<String>, durations_ms: Durations, created_at: i64 }
```

## Acceptance Criteria

1. P10: diagnostics report microphone and hardware readiness, recommend a local
   model with a reason, and optionally measure end-to-end latency.
2. P10: local-only processing disables both cloud stages atomically. Cloud STT requires configured credentials. Neither choice changes retention or context consent. No processing recommendation is made when the microphone is unavailable.
3. P11: registered `voiceflow://` links and a bundled CLI expose start, stop,
   cancel, file transcription, insertion, last-result copy, and status through
   the same service layer; malformed/unknown commands fail safely.
4. P11: the TCP developer bridge is off by default, loopback-only, token-protected,
   and redacts content from logs.
5. P12: code-aware mode preserves identifiers, casing, paths, flags, commands,
   punctuation, and line breaks; IDE/file/symbol context is optional and typed.
6. P12: a documented stdin/JSON editor bridge works without a proprietary
   extension and extension adapters can call the CLI later.
7. P13: dashboard metrics include failures, correction rate, p50/p95 latency,
   local/cloud split, and application delivery failures without transcript text.
8. P13: users can clear and export quality metrics independently of history.

## BDD Scenarios

### Recommend a private setup

Given compatible hardware, a working microphone, and no cloud credentials
When diagnostics finish
Then local-only processing and a hardware starting-point model are suggested, with no claim of measured accuracy.

### Reject a remote bridge request

Given the optional TCP bridge is enabled
When a non-loopback peer or invalid token sends a command
Then the backend rejects it without executing an action.

### Preserve a shell command

Given code-aware mode with shell context
When speech contains a path and long option
Then the result preserves path separators, casing, dashes, and line breaks.

### Report delivery reliability

Given successful and failed insertions across two applications
When the quality dashboard loads
Then it shows failure counts and latency percentiles per application
And no transcript content is returned.

## Verification

- Recommendation, preset, URL parser, authentication, code-policy, percentile,
  migration, clear, and export unit tests.
- CLI integration tests against a temporary local socket/service.
- Frontend tests for diagnostics, presets, and quality filters.
- Native URL-scheme and CLI smoke tests on the local build.

## Original completion evidence

The [product simplification contract](../../../product-simplification/0.1.0/prd/erd.md) supersedes broad presets, unconditional bridge startup, and primary navigation placement. Diagnostics and code context now live under Advanced. The evidence below predates that reduction.

- Diagnostics, presets, onboarding, authenticated loopback bridge, bundled and
  standalone CLI, `voiceflow://`, code context/formatting, and content-free
  quality metrics are implemented and covered by backend and UI tests.
- The native Tauri smoke test opened Quality and verified Setup diagnostics.
- The release bundle registers `voiceflow://`, its bridge endpoint file was
  verified as user-private (`0600`), and both CLI entry points returned a live
  status response.
- `voiceflow://status`, unknown-command rejection, and stdin code formatting
  were exercised against the running release app; `cargo test --workspace` was
  produced from spoken-form input.
