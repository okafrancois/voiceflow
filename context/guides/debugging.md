# Debugging Guide

## When to Read This

- Read [`../../AGENTS.md`](../../AGENTS.md) for execution constraints, recovery protocol, and the default iteration loop
- Read [`../spec/logs.md`](../spec/logs.md) for logging requirements and structured field conventions
- Read this guide for concrete investigation workflow, log locations, and root-cause analysis steps
- Read [`../architecture/data-flow.md`](../architecture/data-flow.md) when debugging depends on pipeline stages, state transitions, or IPC boundaries

## 1. Log Investigation Workflow

When asked to "check and fix" or "investigate":

1. **Check latest logs first**:
   ```bash
   # macOS
   tail -200 ~/Library/Logs/com.voiceflow.voicetotext/voiceflow.log.*
   
   # Or check for crash reports
   ls -la ~/Library/Logs/DiagnosticReports/ | grep -i voiceflow
   ```

2. **Identify failure point** — Look for:
   - ERROR-level log entries
   - Stack traces or panic messages
   - Timestamps near the reported issue
   - Unexpected state transitions

3. **Check crash reports if logs incomplete**:
   ```bash
   head -200 ~/Library/Logs/DiagnosticReports/voiceflow-*.ips
   ```

## 2. Log File Locations

| Platform | Path |
|----------|------|
| macOS (installed app) | `~/Library/Logs/com.voiceflow.voicetotext/` |
| Windows | `%LOCALAPPDATA%\voiceflow\logs\` |

Log files are named `voiceflow.log.YYYY-MM-DD-HH` (hourly rotation).
Local development and E2E bundle identifiers use separate directories.

### Missing dictation endings

Compare raw and final text lengths in the local history database before
attributing a loss to polish. Check capture start/stop, sample rate/channels,
chunk enqueue/send failures, VAD skip counts, and stop-tail flushing. A log
saying every chunk was sent does not prove that preprocessing preserved every
sample or that recognition returned every word.

For replay, verify both the configured `audio_retention` and actual WAV paths
in history. Before the October 23 fix, stop removed the recording state before
the transcription task read it to save audio; even enabled retention therefore
produced no file. The transcription task now owns its audio independently.

Synthetic speech can verify finalization but cannot prove the cause of a lost
phrase without the original recording. Record those limits explicitly.

### Installed version and release source

The default branch of this fork is `master`. The abandoned native prototype
was removed before v1.4.0, but that release was tagged on `native-app`.
On October 23, `master` was fast-forwarded to the exact v1.4.0 commit.

Use `gh ... --repo okafrancois/voiceflow` for this fork: bare `gh` can select
the upstream AriaType repository. Compare the peeled tag commit, the local
branch, remote refs, and the installed app's `CFBundleShortVersionString`.
The release's `targetCommitish` field alone is not evidence of tag ancestry.
Updating the checkout does not replace the running installed binary.

## 3. IPC Log Access

```typescript
// Get last N lines of log content
invoke("get_log_content", { lines: 100 });

// Open log folder in system file manager
invoke("open_log_folder");
```

## 4. Root Cause Analysis Protocol

1. Reproduce the issue deterministically
2. Collect direct evidence: logs, stack traces, diagnostics, failing assertions
3. Trace execution path across caller → callee → state → side effects → boundaries
4. Identify the precise failure point
5. State root cause as evidence-backed conclusion
6. Verify the fix removes root cause, not just symptom

## 5. Failure Recovery Pattern

After 3 consecutive failures:
1. STOP all further edits
2. REVERT to last known working state
3. DOCUMENT what was attempted and what failed
4. ESCALATE to user or consult Oracle with full failure context

## 6. Text Injection Debugging

- On macOS, keyboard simulation handles text up to 400 graphemes without
  newlines, in chunks of up to 100 graphemes.
- Clipboard paste handles longer or multiline text and keyboard fallback.
- A macOS SIGTRAP with `dispatch_sync called on queue already owned by current
  thread` indicates synchronous dispatch back to the caller's main queue.
  Tray menu callbacks run on the main thread; clipboard helpers must execute
  inline there and marshal only background callers. See the native regression
  executable `macos_main_queue_test` in the testing guide.
- A macOS status-icon click must not also activate the main window. Native
  menu tracking and window activation conflict, especially across fullscreen
  Spaces. Only the explicit Show Settings menu action should focus settings.
- On the affected macOS 27.2 host, a background Regular application's menu
  remained invisible over another app's fullscreen Space even with direct
  AppKit status-button handling. Accessory activation policy restored visibility.
  Voice Flow applies it only while its own status menu tracks, then restores
  the preceding policy. Inspect `tray_menu_accessory_policy_applied` and
  `tray_menu_activation_policy_restored` together; do not permanently change
  Dock behavior or remove Tauri's tray overlay without behavioral evidence.
- Check `apps/desktop/src-tauri/context/text_injection_fix.md` for known issues

## 7. Cloud Engine Debugging

- Auth errors (401/403): Check API key, app_id, base_url in settings
- Connection errors: Check network, WebSocket endpoint URL
- Timeout errors: Check provider status, rate limits
- Use mock credentials pattern for local testing
