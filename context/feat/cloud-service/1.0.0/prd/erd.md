# Cloud Service feature specification

## Feature Name
Cloud Service Tab-based Organization

## Version
v1.0.0

## Overview

Cloud Service provides cloud-based speech-to-text (STT) and text polishing (Polish) functionality. This feature refactors the Cloud Service settings page to use a tab-based UI consistent with the Private AI settings page pattern.

## User Experience

### Navigation
- Route: `/cloud`
- Layout: SettingsPageLayout with title and description
- Pattern: Segmented tab navigation matching Private AI (ModelSettings.tsx)

### Tabs
1. **Cloud STT** - Cloud-based speech recognition configuration
2. **Cloud Polish** - Cloud-based text enhancement configuration

### Tab UI Pattern
```tsx
<div className="inline-flex h-10 items-center justify-center rounded-lg bg-secondary p-1 text-muted-foreground">
  <button className={cn(
    "inline-flex items-center justify-center whitespace-nowrap rounded-md px-4 py-1.5 text-sm font-medium transition-all",
    isActive ? "bg-background text-foreground shadow-sm" : "hover:text-foreground"
  )}>
    Tab Label
  </button>
</div>
```

## Components

### CloudSttSection
- Enable/disable Cloud STT toggle
- Provider selection (`volcengine-streaming`, `aliyun-stream`, `elevenlabs`)
- Provider fields come from the backend schema; the frontend does not keep a second provider list
- App ID for Volcengine
- API Key / Access Token
- Secret fields expose a right-side reveal/hide toggle and remain hidden by default
- Base URL
- Model or resource ID when the provider supports it
- Check button for the active provider. The backend performs a lightweight real connection check using the saved active configuration, without sending user audio.
- Volcengine accepts only the `bigmodel_nostream` interface on the official host

### CloudPolishSection
- Enable/disable Cloud Polish toggle
- Provider selection (`anthropic`, `openai`)
- OpenAI-compatible or Anthropic-compatible gateways use the matching shipped provider with a custom Base URL; there is no `custom` provider ID
- API Key
- Secret fields expose a right-side reveal/hide toggle and remain hidden by default
- Base URL
- Model name
- Enable Thinking toggle
- Runtime behavior: Cloud Polish requests use an adaptive bounded timeout that starts at 5 seconds for tiny requests and scales up to 30 seconds as prompt/input size grows across providers
- Check button for the active provider. The backend performs a minimal real model request using the saved active configuration, without exposing API keys through frontend IPC arguments.

## i18n Keys

| Key | Description |
|-----|-------------|
| `cloud.tabs.stt` | Cloud STT tab label |
| `cloud.tabs.polish` | Cloud Polish tab label |

Added to all 10 supported locales: en, zh, de, es, fr, it, ja, ko, pt, ru

## Files Changed

### New Files
- `apps/desktop/src/components/Home/cloud/CloudSttSection.tsx`
- `apps/desktop/src/components/Home/cloud/CloudPolishSection.tsx`
- `context/feat/cloud-service/v1.0.0/prd/erd.md`

### Modified Files
- `apps/desktop/src/components/Home/CloudService.tsx` - Refactored to use tabs
- `apps/desktop/src/i18n/locales/{locale}.json` - Added tab labels to all locales

## Acceptance Criteria

1. **Tab Navigation**: Two tabs (Cloud STT, Cloud Polish) visible on Cloud Service page
2. **Visual Consistency**: Tab UI matches Private AI (ModelSettings.tsx) pattern
3. **Content Switching**: Clicking tab shows corresponding section
4. **i18n**: All tab labels have translations in all 10 locales
5. **Build**: Frontend builds successfully
6. **Tests**: Deterministic provider contract tests and the complete existing suite pass
7. **Timeout Safety**: Cloud Polish requests use a bounded adaptive timeout from 5 to 30 seconds and never hang indefinitely
8. **Configuration Check**: Enabled Cloud STT and Cloud Polish sections expose a small manual Check action that reports success, missing fields, invalid URL, auth failure, model failure, network failure, timeout, or unsupported provider without persisting stale validation state
9. **Secret Reveal**: API Key / Access Token style fields are hidden by default and can be temporarily revealed from a right-side icon button
10. **Provider Alignment**: The UI, backend schema, runtime dispatch, and provider reference expose the same three STT and two Polish IDs
11. **Offline Contract Coverage**: All shipped providers have local mock-server tests that need no credentials or internet access

## Test Coverage

Provider schema, request construction, response parsing, and streaming lifecycles are covered in Rust. Frontend tests verify that the settings page renders the backend-provided schema. Live vendor checks are ignored by default and do not replace local contract tests.

## Verification

```bash
# Frontend build
pnpm --filter @voiceflow/desktop build

# Rust tests
cd apps/desktop/src-tauri && cargo test
```

## Notes

- Feature card (dashed border with Zap, Sparkles, RefreshCw icons) was removed as it's redundant with tabbed navigation
- Routes remain separate (/cloud and /private-ai)
- Settings context via useSettingsContext() pattern maintained
