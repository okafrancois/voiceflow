# Contributing to @voiceflow/shared

## Overview

**Package**: Shared TypeScript utilities for the Voice Flow monorepo.

**Purpose**: Types and constants used across `@voiceflow/desktop` and `@voiceflow/website`.

**Entry Point**: `src/index.ts`

---

## Exports

### Types (`src/types.ts`)

| Interface | Description |
|-----------|-------------|
| `Settings` | Application settings configuration |
| `Model` | STT model metadata |
| `PillPosition` | Floating indicator position options |
| `IndicatorMode` | Indicator visibility behavior |
| `UpdateInfo` | Application update information |

### Constants (`src/constants.ts`)

| Constant | Value | Usage |
|----------|-------|-------|
| `APP_VERSION` | `'1.0.0'` | Version display |
| `APP_NAME` | `'Voice Flow'` | App name display |
| `GITHUB_RELEASES_URL` | `github.com/okafrancois/voiceflow/releases` | GitHub Releases index |
| `DOWNLOAD_URL` | `github.com/okafrancois/voiceflow/releases/latest` | Download fallback page |

---

## Development Setup

```bash
# From repository root
pnpm install

# Type checking (primary validation)
pnpm --filter @voiceflow/shared typecheck

# Or from package directory
cd packages/shared && pnpm typecheck
```

---

## Code Style

- TypeScript strict mode
- ES modules (`type: "module"`)
- No runtime dependencies
- All identifiers and comments in **English**
- Export everything via `src/index.ts`

**TypeScript Config**: Extends `tsconfig.json` in package root

---

## Adding New Types/Constants

1. **Add to appropriate source file**:
   - Types → `src/types.ts`
   - Constants → `src/constants.ts`

2. **Export from index**:
   ```typescript
   // src/index.ts
   export * from './types';
   export * from './constants';
   ```

3. **Run typecheck**:
   ```bash
   pnpm --filter @voiceflow/shared typecheck
   ```

4. **Validate in dependent packages**:
   ```bash
   pnpm --filter @voiceflow/desktop build
   pnpm --filter @voiceflow/website build
   ```

---

## Dependencies

**Runtime**: None (zero dependencies)

**DevDependencies**:
- `typescript@^5.7.3`

---

## Usage in Other Packages

```typescript
// In @voiceflow/desktop or @voiceflow/website
import { Settings, Model, APP_VERSION } from '@voiceflow/shared';

// Type usage
const settings: Settings = {
  autoStart: true,
  recordingSound: true,
  pillPosition: 'top-center',
  indicatorMode: 'always-show',
  selectedModel: 'base',
  language: 'en-US',
};
```

---

## Testing

**No test suite currently** — type checking serves as primary validation.

**Validation Approach**:
- `pnpm typecheck` validates TypeScript correctness
- Changes should be validated in dependent packages
- If tests are added, use Vitest (consistent with desktop package)

---

## Package References

This package is consumed by:

| Package | Import Path |
|---------|-------------|
| `@voiceflow/desktop` | `workspace:*` (monorepo internal) |
| `@voiceflow/website` | `workspace:*` (monorepo internal) |

---

## See Also

- **Root AGENTS.md** — Monorepo guidelines, TDD workflow, coverage gates
- **apps/desktop/CONTRIBUTING.md** — Desktop application development
- **packages/website/CONTRIBUTING.md** — Marketing website development
